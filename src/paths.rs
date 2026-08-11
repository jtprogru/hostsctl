//! Пути к конфигу, кешу и бэкапам + всё, что связано с euid/sudo.
//!
//! Главная тонкость: пишущие команды запускаются через `sudo`, а конфиг и кеш
//! лежат в домашнем каталоге обычного пользователя. Поэтому HOME под sudo
//! доверять нельзя — реальный домашний каталог берём из passwd по `SUDO_USER`,
//! а созданные под root файлы возвращаем владельцу.

use crate::exit;
use anyhow::{Context, Result, anyhow, bail};
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};

pub fn euid() -> u32 {
    unsafe { libc::geteuid() }
}

pub fn is_root() -> bool {
    euid() == 0
}

pub struct SudoUser {
    pub uid: u32,
    pub gid: u32,
    pub name: String,
}

/// Пользователь, который вызвал sudo. None, если запущено без sudo.
pub fn sudo_user() -> Option<SudoUser> {
    let name = std::env::var("SUDO_USER").ok().filter(|s| !s.is_empty())?;
    let uid = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    let gid = std::env::var("SUDO_GID").ok()?.parse().ok()?;
    Some(SudoUser { uid, gid, name })
}

fn passwd_home(user: &str) -> Option<PathBuf> {
    let cname = CString::new(user).ok()?;
    // SAFETY: getpwnam возвращает указатель на статический буфер libc; читаем
    // его сразу и копируем строку, наружу указатель не отдаём.
    unsafe {
        let pw = libc::getpwnam(cname.as_ptr());
        if pw.is_null() {
            return None;
        }
        let dir = (*pw).pw_dir;
        if dir.is_null() {
            return None;
        }
        let s = CStr::from_ptr(dir).to_str().ok()?;
        if s.is_empty() { None } else { Some(PathBuf::from(s)) }
    }
}

/// Домашний каталог того, кто на самом деле запустил утилиту.
pub fn home_dir() -> Result<PathBuf> {
    if let Some(su) = sudo_user()
        && let Some(home) = passwd_home(&su.name)
    {
        return Ok(home);
    }
    if let Ok(h) = std::env::var("HOME")
        && !h.is_empty()
    {
        return Ok(PathBuf::from(h));
    }
    bail!("cannot determine the home directory: $HOME is empty and SUDO_USER is unset")
}

/// `$HOSTSCTL_CONFIG` → `$XDG_CONFIG_HOME/hostsctl/config.yaml` → `~/.config/hostsctl/config.yaml`.
pub fn config_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HOSTSCTL_CONFIG")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    // Под sudo XDG_* указывает на окружение root — игнорируем.
    if sudo_user().is_none()
        && let Ok(x) = std::env::var("XDG_CONFIG_HOME")
        && !x.is_empty()
    {
        return Ok(PathBuf::from(x).join("hostsctl/config.yaml"));
    }
    Ok(home_dir()?.join(".config/hostsctl/config.yaml"))
}

pub fn cache_dir() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HOSTSCTL_CACHE")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    if sudo_user().is_none()
        && let Ok(x) = std::env::var("XDG_CACHE_HOME")
        && !x.is_empty()
    {
        return Ok(PathBuf::from(x).join("hostsctl"));
    }
    Ok(home_dir()?.join(".cache/hostsctl"))
}

pub fn default_backup_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/var/db/hostsctl/backups")
    } else {
        PathBuf::from("/var/lib/hostsctl/backups")
    }
}

pub fn default_target() -> PathBuf {
    std::env::var("HOSTSCTL_TARGET")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/hosts"))
}

/// Возвращает файл, созданный под root, реальному пользователю.
pub fn chown_to_invoking_user(path: &Path) -> Result<()> {
    let Some(su) = sudo_user() else { return Ok(()) };
    if !is_root() {
        return Ok(());
    }
    let cpath = CString::new(path.as_os_str().as_encoded_bytes())
        .with_context(|| format!("invalid path: {}", path.display()))?;
    // SAFETY: путь валиден и NUL-терминирован, uid/gid из окружения sudo.
    let rc = unsafe { libc::chown(cpath.as_ptr(), su.uid, su.gid) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("cannot hand {} back to its owner", path.display()));
    }
    Ok(())
}

/// Создаёт каталог и возвращает его пользователю, если работаем из-под sudo.
pub fn ensure_dir_owned(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)
        .with_context(|| format!("cannot create directory {}", dir.display()))?;
    // Владельца возвращаем всей цепочке от домашнего каталога вниз.
    if let Ok(home) = home_dir() {
        let mut cur = dir.to_path_buf();
        while cur.starts_with(&home) && cur != home {
            chown_to_invoking_user(&cur)?;
            let Some(parent) = cur.parent() else { break };
            cur = parent.to_path_buf();
        }
    }
    Ok(())
}

fn writable(path: &Path) -> bool {
    let Ok(cpath) = CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: путь валиден и NUL-терминирован, access ничего не меняет.
    unsafe { libc::access(cpath.as_ptr(), libc::W_OK) == 0 }
}

/// Сможем ли мы писать в каталог (возможно, ещё не созданный).
pub fn ensure_dir_writable(dir: &Path, action: &str) -> Result<()> {
    if is_root() {
        return Ok(());
    }
    let mut p = dir;
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent,
            None => break,
        }
    }
    if writable(p) {
        return Ok(());
    }
    let cmd: Vec<String> = std::env::args().collect();
    Err(exit::coded(
        exit::PERMISSION,
        anyhow!(
            "{action}: no write access to the backup directory {}\n  run: sudo {}",
            dir.display(),
            cmd.join(" ")
        ),
    ))
}

/// Можем ли мы заменить файл: нужна запись и в сам файл, и в его каталог
/// (атомарная запись делает rename поверх).
pub fn ensure_writable(path: &Path, action: &str) -> Result<()> {
    if is_root() {
        return Ok(());
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_ok = !path.exists() || writable(path);
    let dir_ok = writable(dir);
    if file_ok && dir_ok {
        return Ok(());
    }
    let cmd: Vec<String> = std::env::args().collect();
    Err(exit::coded(
        exit::PERMISSION,
        anyhow!("{action}: no write access to {}\n  run: sudo {}", path.display(), cmd.join(" ")),
    ))
}
