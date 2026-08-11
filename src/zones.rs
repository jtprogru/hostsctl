//! Зоны — файлы с группами рядом с `config.yaml`.
//!
//! Зона бывает двух форматов: YAML (те же группы, что в основном конфиге) и
//! обычный hosts-синтаксис. Какой именно — определяется расширением. Каждая
//! группа помнит, из какого файла пришла, и правки уходят обратно туда же.

use crate::config::{Entry, Group};
use crate::exit::{self, OrCode};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Откуда пришла группа. По умолчанию — основной конфиг.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Origin {
    #[default]
    Main,
    Zone(PathBuf),
}

impl Origin {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Origin::Main => None,
            Origin::Zone(p) => Some(p),
        }
    }

    pub fn label(&self, main: &Path) -> String {
        match self {
            Origin::Main => main
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| main.display().to_string()),
            Origin::Zone(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.display().to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Yaml,
    Hosts,
}

/// Что подключается, если `include` в конфиге не задан.
pub const DEFAULT_INCLUDE: [&str; 2] = ["zones/*.yaml", "zones/*.hosts"];

/// Маркер в шапке hosts-зоны: в этом формате `enabled: false` иначе не выразить.
const DISABLED_MARK: &str = "hostsctl: disabled";

pub fn kind_of(path: &Path) -> Kind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("hosts") => Kind::Hosts,
        _ => Kind::Yaml,
    }
}

/// Разворачивает шаблоны `include` относительно каталога конфига.
///
/// Порядок важен: он задаёт порядок групп, а в hosts выигрывает первая строка.
/// Внутри одного шаблона пути сортируются, между шаблонами — как записано.
pub fn expand(patterns: &[String], base: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = vec![];
    for pat in patterns {
        let full = if Path::new(pat).is_absolute() {
            pat.clone()
        } else {
            base.join(pat).to_string_lossy().to_string()
        };
        let mut matched: Vec<PathBuf> = glob::glob(&full)
            .with_context(|| format!("invalid include pattern: {pat}"))?
            .filter_map(|r| r.ok())
            .filter(|p| p.is_file())
            .collect();
        matched.sort();
        for p in matched {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// Покрыт ли путь шаблонами include.
pub fn is_covered(path: &Path, patterns: &[String], base: &Path) -> bool {
    expand(patterns, base).is_ok_and(|found| found.iter().any(|p| p == path))
}

// --- чтение -----------------------------------------------------------------

/// YAML-зона принимается в трёх видах: `groups:`, голый список групп и
/// одна группа без имени (имя берётся из имени файла).
#[derive(Deserialize)]
#[serde(untagged)]
enum ZoneDoc {
    Wrapped { groups: Vec<Group> },
    List(Vec<Group>),
    Single(SingleGroup),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SingleGroup {
    #[serde(default)]
    name: Option<String>,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    source: Option<crate::config::Source>,
    #[serde(default)]
    entries: Vec<Entry>,
}

fn yes() -> bool {
    true
}

pub fn load(path: &Path) -> Result<Vec<Group>> {
    let mut groups = match kind_of(path) {
        Kind::Yaml => load_yaml(path)?,
        Kind::Hosts => load_hosts(path)?,
    };
    for g in groups.iter_mut() {
        g.origin = Origin::Zone(path.to_path_buf());
    }
    Ok(groups)
}

fn load_yaml(path: &Path) -> Result<Vec<Group>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read zone {}", path.display()))
        .or_code(exit::CONFIG)?;
    if raw.trim().is_empty() {
        return Ok(vec![]);
    }
    let doc: ZoneDoc = serde_yaml::from_str(&raw)
        .with_context(|| format!("zone {} is not valid", path.display()))
        .or_code(exit::CONFIG)?;
    Ok(match doc {
        ZoneDoc::Wrapped { groups } => groups,
        ZoneDoc::List(groups) => groups,
        ZoneDoc::Single(s) => vec![Group {
            name: s.name.unwrap_or_else(|| name_from_path(path)),
            enabled: s.enabled,
            description: s.description,
            source: s.source,
            entries: s.entries,
            origin: Origin::Main,
        }],
    })
}

fn load_hosts(path: &Path) -> Result<Vec<Group>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read zone {}", path.display()))
        .or_code(exit::CONFIG)?;
    let parsed = parse_hosts(&raw);
    if parsed.entries.is_empty() && parsed.description.is_none() {
        return Ok(vec![]);
    }
    Ok(vec![Group {
        name: name_from_path(path),
        enabled: parsed.enabled,
        description: parsed.description,
        source: None,
        entries: parsed.entries,
        origin: Origin::Main,
    }])
}

/// `10-local.hosts` → `local`, `work.yaml` → `work`.
pub fn name_from_path(path: &Path) -> String {
    let stem =
        path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "zone".into());
    let trimmed = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
    if trimmed.is_empty() { stem } else { trimmed.to_string() }
}

pub struct ParsedHosts {
    pub description: Option<String>,
    pub enabled: bool,
    pub entries: Vec<Entry>,
}

/// Разбор hosts-синтаксиса в записи.
///
/// Шапка файла (комментарии до первой записи) становится описанием группы,
/// комментарий над записью или в конце строки — её комментарием,
/// закомментированная запись — выключенной записью.
pub fn parse_hosts(raw: &str) -> ParsedHosts {
    let mut entries = vec![];
    let mut pending: Option<String> = None;
    let mut description = None;
    let mut enabled = true;
    let mut in_header = true;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            pending = None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            let rest = rest.trim();
            if let Some(h) = crate::hostsfile::parse_line(rest) {
                in_header = false;
                let inline = rest.split_once('#').map(|(_, c)| c.trim().to_string());
                entries.push(Entry {
                    ips: vec![h.ip],
                    hostnames: h.hostnames,
                    enabled: false,
                    comment: inline.or_else(|| pending.take()),
                });
            } else if rest == DISABLED_MARK {
                enabled = false;
            } else if in_header && description.is_none() {
                description = Some(rest.to_string());
            } else {
                pending = Some(rest.to_string());
            }
            continue;
        }
        let Some(h) = crate::hostsfile::parse_line(trimmed) else {
            pending = None;
            continue;
        };
        in_header = false;
        let inline = trimmed.split_once('#').map(|(_, c)| c.trim().to_string());
        entries.push(Entry {
            ips: vec![h.ip],
            hostnames: h.hostnames,
            enabled: true,
            comment: inline.or_else(|| pending.take()),
        });
    }

    ParsedHosts { description, enabled, entries: merge_by_hostnames(entries) }
}

/// Несколько строк с одинаковым набором имён — это одно имя на нескольких
/// адресах. Складываем их в одну запись, чтобы конфиг читался как задумано.
fn merge_by_hostnames(entries: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::with_capacity(entries.len());
    for e in entries {
        let twin = out.iter_mut().find(|o| {
            o.enabled == e.enabled && o.comment == e.comment && o.same_hostnames(&e.hostnames)
        });
        match twin {
            Some(o) => o.ips.extend(e.ips),
            None => out.push(e),
        }
    }
    out
}

// --- запись -----------------------------------------------------------------

/// Текст зоны для набора групп. Для hosts-формата групп должно быть не больше одной.
pub fn render(groups: &[Group], kind: Kind, path: &Path) -> Result<String> {
    match kind {
        Kind::Yaml => {
            let doc = serde_yaml::to_string(&YamlZone { groups })
                .with_context(|| format!("cannot serialize zone {}", path.display()))?;
            Ok(format!("# hostsctl zone — attached through include in config.yaml\n{doc}"))
        }
        Kind::Hosts => {
            if groups.len() > 1 {
                bail!(
                    "a hosts zone holds exactly one group, {} was given {} — use .yaml instead",
                    path.display(),
                    groups.len()
                );
            }
            let Some(g) = groups.first() else {
                return Ok(String::new());
            };
            if let Some(src) = &g.source {
                bail!(
                    "group '{}' is a remote list ({}), which a hosts zone cannot express; \
                     move it to .yaml",
                    g.name,
                    src.url
                );
            }
            Ok(render_hosts(g))
        }
    }
}

#[derive(serde::Serialize)]
struct YamlZone<'a> {
    groups: &'a [Group],
}

fn render_hosts(g: &Group) -> String {
    let mut out = String::new();
    if !g.enabled {
        out.push_str(&format!("# {DISABLED_MARK}\n"));
    }
    if let Some(d) = &g.description {
        out.push_str(&format!("# {d}\n"));
    }
    if !out.is_empty() {
        out.push('\n');
    }

    let width = g
        .entries
        .iter()
        .flat_map(|e| e.ips.iter().map(|i| i.len()))
        .max()
        .unwrap_or(0)
        .clamp(7, 15);
    // Строка на каждый адрес: hosts другого способа записать несколько A нет.
    for e in &g.entries {
        for ip in &e.ips {
            let prefix = if e.enabled { "" } else { "# " };
            let pad = width.saturating_sub(ip.len()) + 1;
            out.push_str(prefix);
            out.push_str(ip);
            out.push_str(&" ".repeat(pad));
            out.push_str(&e.hostnames.join(" "));
            if let Some(c) = &e.comment {
                out.push_str(&format!("  # {c}"));
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(name: &str) -> Group {
        Group {
            name: name.into(),
            enabled: true,
            description: Some("a description".into()),
            source: None,
            entries: vec![
                Entry {
                    ips: vec!["127.0.0.1".into()],
                    hostnames: vec!["a.local".into()],
                    enabled: true,
                    comment: Some("a note".into()),
                },
                Entry {
                    ips: vec!["10.0.0.1".into(), "10.0.0.2".into()],
                    hostnames: vec!["b.local".into(), "b2.local".into()],
                    enabled: false,
                    comment: None,
                },
            ],
            origin: Origin::Main,
        }
    }

    #[test]
    fn hosts_zone_roundtrips() {
        let g = group("work");
        let text = render_hosts(&g);
        let parsed = parse_hosts(&text);
        assert_eq!(parsed.description.as_deref(), Some("a description"));
        assert!(parsed.enabled);
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].comment.as_deref(), Some("a note"));
        assert!(!parsed.entries[1].enabled);
        assert_eq!(parsed.entries[1].hostnames, vec!["b.local", "b2.local"]);
        assert_eq!(
            parsed.entries[1].ips,
            vec!["10.0.0.1", "10.0.0.2"],
            "several addresses for one name must survive the round-trip"
        );
    }

    #[test]
    fn disabled_group_survives_hosts_roundtrip() {
        let mut g = group("work");
        g.enabled = false;
        let parsed = parse_hosts(&render_hosts(&g));
        assert!(!parsed.enabled);
        assert_eq!(parsed.entries.len(), 2);
    }

    #[test]
    fn yaml_zone_accepts_three_shapes() {
        let dir = tempfile::tempdir().unwrap();

        let wrapped = dir.path().join("a.yaml");
        std::fs::write(&wrapped, "groups:\n  - name: one\n    entries: []\n").unwrap();
        assert_eq!(load(&wrapped).unwrap()[0].name, "one");

        let list = dir.path().join("b.yaml");
        std::fs::write(&list, "- name: two\n  entries: []\n").unwrap();
        assert_eq!(load(&list).unwrap()[0].name, "two");

        let single = dir.path().join("10-three.yaml");
        std::fs::write(&single, "description: nameless\nentries: []\n").unwrap();
        let g = load(&single).unwrap();
        assert_eq!(g[0].name, "three", "the name comes from the file name");
        assert_eq!(g[0].origin, Origin::Zone(single));
    }

    #[test]
    fn yaml_zone_typo_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("z.yaml");
        // 'group' вместо 'groups' — молча считать файл пустым нельзя
        std::fs::write(&f, "group:\n  - name: one\n").unwrap();
        assert!(load(&f).is_err());
    }

    #[test]
    fn hosts_zone_rejects_remote_group() {
        let mut g = group("ads");
        g.source = Some(crate::config::Source {
            url: "https://example.invalid/hosts".into(),
            rewrite_ip: None,
            allow: vec![],
            last_fetch: None,
        });
        let err = render(&[g], Kind::Hosts, Path::new("ads.hosts")).unwrap_err();
        assert!(err.to_string().contains("is a remote list"));
    }

    #[test]
    fn include_order_follows_patterns_then_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("zones")).unwrap();
        for n in ["20-b.yaml", "10-a.yaml"] {
            std::fs::write(dir.path().join("zones").join(n), "groups: []\n").unwrap();
        }
        std::fs::write(dir.path().join("zones/c.hosts"), "").unwrap();

        let pats: Vec<String> = DEFAULT_INCLUDE.iter().map(|s| s.to_string()).collect();
        let found = expand(&pats, dir.path()).unwrap();
        let names: Vec<String> =
            found.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["10-a.yaml", "20-b.yaml", "c.hosts"]);
    }

    #[test]
    fn name_from_path_strips_prefix() {
        assert_eq!(name_from_path(Path::new("/x/10-local.hosts")), "local");
        assert_eq!(name_from_path(Path::new("/x/work.yaml")), "work");
    }
}
