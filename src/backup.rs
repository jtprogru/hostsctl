//! Бэкапы целевого файла: снимок перед каждой записью + откат.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Backup {
    pub id: String,
    pub path: PathBuf,
    pub size: u64,
}

const PREFIX: &str = "hosts-";

pub fn create(target: &Path, dir: &Path, stamp: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let content = std::fs::read_to_string(target)
        .with_context(|| format!("cannot read {} to back it up", target.display()))?;
    let mut path = dir.join(format!("{PREFIX}{stamp}"));
    // Два apply в одну секунду не должны затирать друг друга.
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{PREFIX}{stamp}-{n}"));
        n += 1;
    }
    crate::hostsfile::write_atomic(&path, &content, 0o644)?;
    Ok(path)
}

pub fn list(dir: &Path) -> Vec<Backup> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<Backup> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_prefix(PREFIX)?.to_string();
            let size = e.metadata().ok()?.len();
            Some(Backup { id, path: e.path(), size })
        })
        .collect();
    // Имена сортируются лексикографически = хронологически (YYYYmmdd-HHMMSS).
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

pub fn find(dir: &Path, id: &str) -> Result<Backup> {
    list(dir)
        .into_iter()
        .find(|b| b.id == id)
        .with_context(|| format!("no backup '{id}'\n  list them: hostsctl backup list"))
}

pub fn latest(dir: &Path) -> Result<Backup> {
    list(dir).into_iter().next().with_context(|| format!("no backups in {}", dir.display()))
}

pub fn prune(dir: &Path, keep: usize) -> usize {
    if keep == 0 {
        return 0;
    }
    let all = list(dir);
    let mut removed = 0;
    for b in all.into_iter().skip(keep) {
        if std::fs::remove_file(&b.path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub fn restore(backup: &Backup, target: &Path) -> Result<()> {
    let content = std::fs::read_to_string(&backup.path)
        .with_context(|| format!("cannot read {}", backup.path.display()))?;
    crate::hostsfile::sanity_check(&content)
        .with_context(|| format!("backup {} looks broken", backup.id))?;
    if content.trim().is_empty() {
        bail!("backup {} is empty — restore cancelled", backup.id);
    }
    crate::hostsfile::write_atomic(target, &content, 0o644)
}
