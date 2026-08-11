//! Модель конфига (YAML) — единственный источник правды для управляемых записей.

use crate::exit::{self, OrCode};
use crate::zones::Origin;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CURRENT_VERSION: u32 = 1;

/// Не трогаем файл, если содержимое не изменилось: иначе каждая команда
/// переписывала бы все зоны и дёргала mtime без причины.
fn write_if_changed(path: &Path, text: &str) -> Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    crate::hostsfile::write_atomic(path, text, 0o644)?;
    crate::paths::chown_to_invoking_user(path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub settings: Settings,
    /// Шаблоны файлов-зон относительно каталога конфига.
    /// Если поля нет — берутся значения по умолчанию (`zones/*.yaml`, `zones/*.hosts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Файл, в который рендерится управляемый блок.
    #[serde(default = "default_target")]
    pub target: PathBuf,
    #[serde(default = "crate::paths::default_backup_dir")]
    pub backup_dir: PathBuf,
    /// Сколько бэкапов хранить; 0 — не чистить.
    #[serde(default = "default_keep_backups")]
    pub keep_backups: usize,
    /// Сбрасывать DNS-кеш после успешной записи.
    #[serde(default = "default_true")]
    pub flush_dns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Удалённый hosts-список. Записи такой группы берутся из кеша, а не из `entries`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
    /// Файл, из которого группа пришла. В сам файл не пишется.
    #[serde(skip)]
    pub origin: Origin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    /// Переписать IP всех записей списка (обычно `0.0.0.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite_ip: Option<String>,
    /// Имена, которые из списка исключаются.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fetch: Option<String>,
}

/// Запись — это связка «адреса → имена», N к M.
///
/// Одному IP можно дать несколько имён, и одно имя может жить на нескольких
/// адресах: в hosts это просто несколько строк, и обрезать их нельзя.
/// В YAML поле называется `ip` и принимает и скаляр, и список.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "ip", with = "ip_field")]
    pub ips: Vec<String>,
    pub hostnames: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_keep_backups() -> usize {
    20
}
fn default_target() -> PathBuf {
    crate::paths::default_target()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            target: default_target(),
            backup_dir: crate::paths::default_backup_dir(),
            keep_backups: default_keep_backups(),
            flush_dns: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            settings: Settings::default(),
            include: Some(crate::zones::DEFAULT_INCLUDE.iter().map(|s| s.to_string()).collect()),
            // Без групп: первая создаётся при `add`, а пустая 'local' в конфиге
            // конфликтовала бы с зоной вроде zones/10-local.hosts.
            groups: vec![],
        }
    }
}

/// `ip: 127.0.0.1` и `ip: [10.0.0.1, 10.0.0.2]` — одно и то же поле.
/// Один адрес пишется обратно скаляром, чтобы конфиг не пух на ровном месте.
mod ip_field {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
        Ok(match OneOrMany::deserialize(d)? {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        })
    }

    #[allow(clippy::ptr_arg)]
    pub fn serialize<S: Serializer>(v: &Vec<String>, s: S) -> Result<S::Ok, S::Error> {
        match v.as_slice() {
            [one] => one.serialize(s),
            many => many.serialize(s),
        }
    }
}

impl Entry {
    pub fn new(ip: impl Into<String>, hostnames: Vec<String>) -> Self {
        Self { ips: vec![ip.into()], hostnames, enabled: true, comment: None }
    }

    /// Совпадает ли набор имён — по нему решаем, доливать IP в существующую
    /// запись или заводить новую.
    pub fn same_hostnames(&self, other: &[String]) -> bool {
        let norm = |v: &[String]| {
            let mut n: Vec<String> =
                v.iter().map(|h| h.trim_end_matches('.').to_lowercase()).collect();
            n.sort();
            n
        };
        norm(&self.hostnames) == norm(other)
    }

    pub fn has_ip(&self, ip: &str) -> bool {
        self.ips.iter().any(|i| i == ip)
    }
}

impl Group {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            description: None,
            source: None,
            entries: vec![],
            origin: Origin::Main,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.source.is_some()
    }
}

impl Config {
    /// Читает основной конфиг без зон — для команд, которым зоны не нужны.
    pub fn load_main(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| {
                format!("cannot read the config {}\n  create one: hostsctl init", path.display())
            })
            .or_code(exit::CONFIG)?;
        let cfg: Config = serde_yaml::from_str(&raw)
            .with_context(|| format!("config {} is not valid", path.display()))
            .or_code(exit::CONFIG)?;
        if cfg.version > CURRENT_VERSION {
            return Err(exit::coded(
                exit::CONFIG,
                anyhow::anyhow!(
                    "config is version {}, this build only understands {}",
                    cfg.version,
                    CURRENT_VERSION
                ),
            ));
        }
        Ok(cfg)
    }

    /// Основной конфиг плюс все подключённые зоны.
    ///
    /// Возвращает ещё и список найденных файлов-зон: при сохранении надо знать,
    /// какие файлы существуют, даже если групп в них не осталось.
    pub fn load(path: &Path) -> Result<(Self, Vec<PathBuf>)> {
        let mut cfg = Self::load_main(path)?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let zone_paths = crate::zones::expand(&cfg.include_patterns(), base)?;

        for zp in &zone_paths {
            let groups = crate::zones::load(zp)?;
            cfg.groups.extend(groups);
        }
        cfg.check_unique_group_names(path)?;
        Ok((cfg, zone_paths))
    }

    pub fn include_patterns(&self) -> Vec<String> {
        self.include.clone().unwrap_or_else(|| {
            crate::zones::DEFAULT_INCLUDE.iter().map(|s| s.to_string()).collect()
        })
    }

    fn check_unique_group_names(&self, main: &Path) -> Result<()> {
        let mut seen: std::collections::HashMap<String, &Group> = std::collections::HashMap::new();
        for g in &self.groups {
            if let Some(prev) = seen.insert(g.name.to_lowercase(), g) {
                return Err(exit::coded(
                    exit::CONFIG,
                    anyhow::anyhow!(
                        "group '{}' is declared twice: {} and {}",
                        g.name,
                        prev.origin.label(main),
                        g.origin.label(main)
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Пишет основной конфиг и все зоны, тронув только реально изменившиеся файлы.
    pub fn save(&self, path: &Path, known_zones: &[PathBuf]) -> Result<()> {
        self.check_unique_group_names(path)?;
        if let Some(dir) = path.parent() {
            crate::paths::ensure_dir_owned(dir)?;
        }

        let main_only = Config {
            version: self.version,
            settings: self.settings.clone(),
            include: self.include.clone(),
            groups: self.groups.iter().filter(|g| g.origin == Origin::Main).cloned().collect(),
        };
        let body = serde_yaml::to_string(&main_only).context("cannot serialize the config")?;
        write_if_changed(path, &format!("# hostsctl config — see hostsctl --help\n{body}"))?;

        // Файл-зона перезаписывается целиком, включая опустевшие.
        let mut targets: Vec<PathBuf> = known_zones.to_vec();
        for g in &self.groups {
            if let Some(p) = g.origin.path()
                && !targets.contains(&p.to_path_buf())
            {
                targets.push(p.to_path_buf());
            }
        }
        for zone in targets {
            let groups: Vec<Group> = self
                .groups
                .iter()
                .filter(|g| g.origin.path() == Some(zone.as_path()))
                .cloned()
                .collect();
            let text = crate::zones::render(&groups, crate::zones::kind_of(&zone), &zone)?;
            if let Some(dir) = zone.parent() {
                crate::paths::ensure_dir_owned(dir)?;
            }
            write_if_changed(&zone, &text)?;
        }
        Ok(())
    }

    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name.eq_ignore_ascii_case(name))
    }

    pub fn group_mut(&mut self, name: &str) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.name.eq_ignore_ascii_case(name))
    }

    pub fn require_group_mut(&mut self, name: &str) -> Result<&mut Group> {
        if self.group(name).is_none() {
            bail!("no such group '{name}'\n  list them: hostsctl group list");
        }
        Ok(self.group_mut(name).expect("checked above"))
    }

    /// Все локальные записи с именем группы: (группа, индекс, запись).
    pub fn iter_entries(&self) -> impl Iterator<Item = (&Group, usize, &Entry)> {
        self.groups.iter().flat_map(|g| g.entries.iter().enumerate().map(move |(i, e)| (g, i, e)))
    }

    /// Ищет запись по имени хоста. Возвращает (индекс группы, индекс записи).
    pub fn find_hostname(&self, host: &str) -> Vec<(usize, usize)> {
        let needle = host.trim_end_matches('.').to_lowercase();
        let mut out = vec![];
        for (gi, g) in self.groups.iter().enumerate() {
            for (ei, e) in g.entries.iter().enumerate() {
                if e.hostnames.iter().any(|h| h.trim_end_matches('.').to_lowercase() == needle) {
                    out.push((gi, ei));
                }
            }
        }
        out
    }
}
