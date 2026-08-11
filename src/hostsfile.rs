//! Чтение `/etc/hosts`, поиск управляемого блока и атомарная запись.
//!
//! Инвариант: всё, что лежит вне маркеров `hostsctl`, переносится в новый файл
//! байт в байт. Утилита не трогает ни системные строки, ни ручные правки.

use crate::exit::{self, OrCode};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const BEGIN: &str = "# >>> hostsctl begin >>>";
pub const END: &str = "# <<< hostsctl end <<<";
/// Блок старого bash-скрипта: мы его видим, но по своей воле не удаляем.
pub const LEGACY_BEGIN: &str = "# >>> hosts-sync begin >>>";
pub const LEGACY_END: &str = "# <<< hosts-sync end <<<";

/// Диапазон строк блока, включая обе строки-маркера.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct HostsFile {
    pub path: PathBuf,
    pub raw: String,
    pub lines: Vec<String>,
}

/// Одна разобранная строка hosts: IP + имена.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostLine {
    pub ip: String,
    pub hostnames: Vec<String>,
}

/// Разбирает строку hosts. Первое поле обязано быть IP — иначе это не запись,
/// а обычный текст (например, комментарий из нескольких слов).
pub fn parse_line(line: &str) -> Option<HostLine> {
    let body = line.split('#').next().unwrap_or("").trim();
    if body.is_empty() {
        return None;
    }
    let mut it = body.split_whitespace();
    let ip = it.next()?;
    // fe80::1%lo0 — zone id к IP не относится, но в hosts встречается.
    let bare = ip.split('%').next().unwrap_or(ip);
    bare.parse::<std::net::IpAddr>().ok()?;
    let hostnames: Vec<String> = it.map(str::to_string).collect();
    if hostnames.is_empty() {
        return None;
    }
    Some(HostLine { ip: ip.to_string(), hostnames })
}

impl HostsFile {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))
            .or_code(exit::IO)?;
        let lines = raw.lines().map(str::to_string).collect();
        Ok(Self { path: path.to_path_buf(), raw, lines })
    }

    fn find_block(&self, begin: &str, end: &str) -> Option<Block> {
        let start = self.lines.iter().position(|l| l.trim() == begin)?;
        let end_idx = self.lines[start..].iter().position(|l| l.trim() == end).map(|i| i + start);
        match end_idx {
            Some(e) => Some(Block { start, end: e }),
            // Маркер начала без конца — считаем блоком до конца файла,
            // иначе повторный apply будет плодить дубликаты.
            None => Some(Block { start, end: self.lines.len().saturating_sub(1) }),
        }
    }

    pub fn managed(&self) -> Option<Block> {
        self.find_block(BEGIN, END)
    }

    pub fn legacy(&self) -> Option<Block> {
        self.find_block(LEGACY_BEGIN, LEGACY_END)
    }

    /// Записи вне управляемого блока — они выигрывают у наших, если стоят выше.
    pub fn unmanaged_lines(&self) -> Vec<(usize, HostLine)> {
        let managed = self.managed();
        self.lines
            .iter()
            .enumerate()
            .filter(|(i, _)| managed.is_none_or(|b| *i < b.start || *i > b.end))
            .filter_map(|(i, l)| parse_line(l).map(|h| (i + 1, h)))
            .collect()
    }

    /// Собирает новое содержимое файла.
    ///
    /// * `block: Some(..)` — вставить/заменить управляемый блок;
    /// * `block: None` — убрать управляемый блок совсем;
    /// * `drop_legacy` — заодно вырезать блок старого hosts-sync.
    pub fn compose(&self, block: Option<&[String]>, drop_legacy: bool) -> String {
        let managed = self.managed();
        let legacy = if drop_legacy { self.legacy() } else { None };

        let mut out: Vec<String> = Vec::with_capacity(self.lines.len() + 32);
        let mut inserted = false;
        let mut i = 0;
        while i < self.lines.len() {
            if let Some(b) = managed
                && i == b.start
            {
                if let Some(new_block) = block {
                    out.extend(new_block.iter().cloned());
                    inserted = true;
                } else {
                    // блок убираем — не оставляем после него висящую пустую строку
                    while out.last().is_some_and(|l| l.trim().is_empty()) {
                        out.pop();
                    }
                }
                i = b.end + 1;
                continue;
            }
            if let Some(b) = legacy
                && i == b.start
            {
                while out.last().is_some_and(|l| l.trim().is_empty()) {
                    out.pop();
                }
                i = b.end + 1;
                continue;
            }
            out.push(self.lines[i].clone());
            i += 1;
        }

        if let Some(new_block) = block
            && !inserted
        {
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                out.pop();
            }
            if !out.is_empty() {
                out.push(String::new());
            }
            out.extend(new_block.iter().cloned());
        }

        while out.last().is_some_and(|l| l.trim().is_empty()) {
            out.pop();
        }
        let mut text = out.join("\n");
        text.push('\n');
        text
    }
}

/// Проверка перед записью: системные строки должны уцелеть.
///
/// Ловит и баг в самой утилите, и попытку конфига переопределить localhost.
pub fn sanity_check(new_content: &str) -> Result<()> {
    let has_localhost = new_content.lines().filter_map(parse_line).any(|h| {
        h.ip == "127.0.0.1" && h.hostnames.iter().any(|n| n.eq_ignore_ascii_case("localhost"))
    });
    if !has_localhost {
        bail!(
            "refusing to write: the result has no '127.0.0.1 localhost' line.\n  \
             that would break the system — check the config and the current /etc/hosts"
        );
    }
    Ok(())
}

/// Атомарная запись: временный файл рядом с целью, затем rename.
///
/// Права и владелец существующего файла сохраняются — /etc/hosts остаётся
/// root:wheel 0644 даже если umask другой.
pub fn write_atomic(path: &Path, content: &str, fallback_mode: u32) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let meta = std::fs::metadata(path).ok();

    let mut tmp = tempfile::Builder::new()
        .prefix(".hostsctl-")
        .tempfile_in(dir)
        .with_context(|| format!("cannot create a temporary file in {}", dir.display()))?;
    tmp.write_all(content.as_bytes()).context("cannot write the temporary file")?;
    tmp.flush().context("cannot flush the buffer")?;
    tmp.as_file().sync_all().context("fsync failed")?;

    let mode = meta.as_ref().map(|m| m.permissions().mode() & 0o7777).unwrap_or(fallback_mode);
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode))
        .context("cannot set the mode of the temporary file")?;

    #[cfg(unix)]
    if let Some(m) = &meta {
        use std::os::unix::fs::MetadataExt;
        let cpath = std::ffi::CString::new(tmp.path().as_os_str().as_encoded_bytes())
            .context("invalid temporary file path")?;
        // SAFETY: путь валиден; uid/gid взяты у существующего целевого файла.
        let rc = unsafe { libc::chown(cpath.as_ptr(), m.uid(), m.gid()) };
        if rc != 0 && crate::paths::is_root() {
            return Err(std::io::Error::last_os_error()).context("cannot set the owner");
        }
    }

    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("cannot replace {}", path.display()))
        .or_code(exit::IO)?;
    Ok(())
}

/// Сброс DNS-кеша. Имеет смысл только для настоящего системного файла:
/// правка копии в /tmp резолвер не волнует.
pub fn flush_dns(target: &Path) -> Vec<String> {
    if target != Path::new("/etc/hosts") {
        return vec![];
    }
    let quiet = |bin: &str, args: &[&str]| -> std::io::Result<std::process::ExitStatus> {
        std::process::Command::new(bin)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    };

    let mut notes = vec![];
    if cfg!(target_os = "macos") {
        if !quiet("dscacheutil", &["-flushcache"]).is_ok_and(|s| s.success()) {
            notes.push("dscacheutil -flushcache did not succeed".into());
        }
        // mDNSResponder может быть не запущен — это не ошибка.
        let _ = quiet("killall", &["-HUP", "mDNSResponder"]);
    } else {
        let ok = ["resolvectl", "systemd-resolve"]
            .iter()
            .any(|bin| quiet(bin, &["--flush-caches"]).is_ok_and(|s| s.success()));
        if !ok {
            notes.push("found no way to flush the DNS cache — probably none is needed".into());
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hf(text: &str) -> HostsFile {
        HostsFile {
            path: PathBuf::from("/tmp/hosts"),
            raw: text.to_string(),
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    #[test]
    fn parses_line_with_comment() {
        let l = parse_line("127.0.0.1  foo.local bar # a comment").unwrap();
        assert_eq!(l.ip, "127.0.0.1");
        assert_eq!(l.hostnames, vec!["foo.local", "bar"]);
        assert!(parse_line("   # comment only").is_none());
        assert!(parse_line("127.0.0.1").is_none());
    }

    #[test]
    fn text_is_not_a_host_line() {
        // Иначе комментарий из двух слов превращается в «запись».
        assert!(parse_line("Local development").is_none());
        assert!(parse_line("nameserver 8.8.8.8").is_none());
        assert!(parse_line("fe80::1%lo0 router").is_some());
        assert!(parse_line("::1 localhost").is_some());
    }

    #[test]
    fn appends_block_when_absent() {
        let f = hf("127.0.0.1\tlocalhost\n");
        let out = f.compose(Some(&[BEGIN.into(), "1.2.3.4 x".into(), END.into()]), false);
        assert_eq!(
            out,
            "127.0.0.1\tlocalhost\n\n# >>> hostsctl begin >>>\n1.2.3.4 x\n# <<< hostsctl end <<<\n"
        );
    }

    #[test]
    fn replaces_block_in_place_and_keeps_tail() {
        let f = hf("a\n\n# >>> hostsctl begin >>>\nold\n# <<< hostsctl end <<<\n\nb\n");
        let out = f.compose(Some(&[BEGIN.into(), "new".into(), END.into()]), false);
        assert_eq!(out, "a\n\n# >>> hostsctl begin >>>\nnew\n# <<< hostsctl end <<<\n\nb\n");
    }

    #[test]
    fn removes_block_without_leaving_blank() {
        let f = hf("a\n\n# >>> hostsctl begin >>>\nold\n# <<< hostsctl end <<<\n");
        assert_eq!(f.compose(None, false), "a\n");
    }

    #[test]
    fn keeps_unmanaged_content_verbatim() {
        let src = "##\n# Host Database\n##\n127.0.0.1\tlocalhost\n::1  localhost\n\n# hand edit\n10.0.0.1 nas\n";
        let f = hf(src);
        let out = f.compose(Some(&[BEGIN.into(), "1.1.1.1 one".into(), END.into()]), false);
        for line in src.lines() {
            assert!(out.contains(line), "lost line: {line}");
        }
    }

    #[test]
    fn legacy_block_survives_unless_asked() {
        let src = "127.0.0.1 localhost\n\n# >>> hosts-sync begin >>>\n1.1.1.1 old\n# <<< hosts-sync end <<<\n";
        let f = hf(src);
        let keep = f.compose(Some(&[BEGIN.into(), "2.2.2.2 new".into(), END.into()]), false);
        assert!(keep.contains("hosts-sync begin"));
        assert!(keep.contains("1.1.1.1 old"));
        let dropped = f.compose(Some(&[BEGIN.into(), "2.2.2.2 new".into(), END.into()]), true);
        assert!(!dropped.contains("hosts-sync"));
        assert!(dropped.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn unterminated_marker_is_not_duplicated() {
        let f = hf("127.0.0.1 localhost\n# >>> hostsctl begin >>>\nbroken\n");
        let out = f.compose(Some(&[BEGIN.into(), "ok".into(), END.into()]), false);
        assert_eq!(out.matches(BEGIN).count(), 1);
        assert!(!out.contains("broken"));
    }

    #[test]
    fn sanity_check_guards_localhost() {
        assert!(sanity_check("127.0.0.1 localhost\n").is_ok());
        assert!(sanity_check("10.0.0.1 nas\n").is_err());
    }
}
