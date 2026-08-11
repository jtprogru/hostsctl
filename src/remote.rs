//! Удалённые hosts-списки: скачивание, кеш, разбор.
//!
//! `apply` работает только с кешем и никогда не ходит в сеть — иначе результат
//! зависел бы от того, доступен ли сейчас интернет.

use crate::config::Source;
use crate::exit::{self, OrCode};
use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Meta {
    pub url: String,
    pub etag: Option<String>,
    pub fetched: String,
    pub lines: usize,
}

pub struct FetchOutcome {
    pub changed: bool,
    pub entries: usize,
    pub bytes: usize,
}

fn safe_name(group: &str) -> String {
    group
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn paths(group: &str, cache_dir: &Path) -> (PathBuf, PathBuf) {
    let base = cache_dir.join("sources");
    let n = safe_name(group);
    (base.join(format!("{n}.hosts")), base.join(format!("{n}.meta")))
}

pub fn fetch(
    group: &str,
    src: &Source,
    cache_dir: &Path,
    force: bool,
    now: &str,
) -> Result<FetchOutcome> {
    let (data_path, meta_path) = paths(group, cache_dir);
    crate::paths::ensure_dir_owned(data_path.parent().expect("has a parent"))?;

    let prev = read_meta(&meta_path);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(concat!("hostsctl/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();

    let mut req = agent.get(&src.url);
    if !force
        && let Some(m) = &prev
        && let Some(etag) = &m.etag
        && data_path.exists()
    {
        req = req.header("If-None-Match", etag);
    }

    let mut resp = req
        .call()
        .with_context(|| format!("cannot download {}", src.url))
        .or_code(exit::NETWORK)?;
    let status = resp.status().as_u16();
    if status == 304 {
        return Ok(FetchOutcome {
            changed: false,
            entries: prev.map(|m| m.lines).unwrap_or(0),
            bytes: 0,
        });
    }
    if !(200..300).contains(&status) {
        return Err(exit::coded(exit::NETWORK, anyhow!("{} answered {status}", src.url)));
    }
    let etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = resp
        .body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .read_to_string()
        .with_context(|| format!("cannot read the response from {}", src.url))
        .or_code(exit::NETWORK)?;

    let entries = parse_list(&body, src);
    let rendered: String = entries.iter().map(|(ip, host)| format!("{ip} {host}\n")).collect();
    crate::hostsfile::write_atomic(&data_path, &rendered, 0o644)?;
    crate::paths::chown_to_invoking_user(&data_path)?;

    let meta = format!(
        "url: {}\netag: {}\nfetched: {now}\nlines: {}\n",
        src.url,
        etag.as_deref().unwrap_or(""),
        entries.len()
    );
    crate::hostsfile::write_atomic(&meta_path, &meta, 0o644)?;
    crate::paths::chown_to_invoking_user(&meta_path)?;

    Ok(FetchOutcome { changed: true, entries: entries.len(), bytes: body.len() })
}

/// Записи из кеша. `None` — кеша нет, группу надо пропустить.
pub fn cached_entries(
    group: &str,
    src: &Source,
    cache_dir: &Path,
) -> Result<Option<Vec<(String, String)>>> {
    let (data_path, _) = paths(group, cache_dir);
    if !data_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&data_path)
        .with_context(|| format!("cannot read the cache {}", data_path.display()))?;
    // Кеш уже нормализован при скачивании, но allow/rewrite_ip могли поменяться
    // в конфиге после этого — применяем их ещё раз.
    Ok(Some(parse_list(&raw, src)))
}

pub fn read_meta(meta_path: &Path) -> Option<Meta> {
    let raw = std::fs::read_to_string(meta_path).ok()?;
    let mut url = String::new();
    let mut etag = None;
    let mut fetched = String::new();
    let mut lines = 0usize;
    for l in raw.lines() {
        let Some((k, v)) = l.split_once(": ").or_else(|| l.split_once(':')) else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "url" => url = v.to_string(),
            "etag" if !v.is_empty() => etag = Some(v.to_string()),
            "fetched" => fetched = v.to_string(),
            "lines" => lines = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    Some(Meta { url, etag, fetched, lines })
}

pub fn meta_for(group: &str, cache_dir: &Path) -> Option<Meta> {
    let (_, meta_path) = paths(group, cache_dir);
    read_meta(&meta_path)
}

pub fn drop_cache(group: &str, cache_dir: &Path) {
    let (d, m) = paths(group, cache_dir);
    let _ = std::fs::remove_file(d);
    let _ = std::fs::remove_file(m);
}

/// Разбор скачанного hosts-списка: комментарии прочь, localhost прочь,
/// дубли прочь, IP при необходимости переписан.
pub fn parse_list(body: &str, src: &Source) -> Vec<(String, String)> {
    let allow: HashSet<String> =
        src.allow.iter().map(|s| s.trim_end_matches('.').to_lowercase()).collect();
    let mut seen = HashSet::new();
    let mut out = vec![];

    for line in body.lines() {
        let Some(parsed) = crate::hostsfile::parse_line(line) else {
            continue;
        };
        for host in parsed.hostnames {
            let key = host.trim_end_matches('.').to_lowercase();
            if key == "localhost"
                || key == "localhost.localdomain"
                || key == "broadcasthost"
                || key == "local"
                || key.is_empty()
            {
                continue;
            }
            if allow.contains(&key) || !seen.insert(key.clone()) {
                continue;
            }
            let ip = src.rewrite_ip.clone().unwrap_or_else(|| parsed.ip.clone());
            out.push((ip, host));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(rewrite: Option<&str>, allow: &[&str]) -> Source {
        Source {
            url: "https://example.invalid/hosts".into(),
            rewrite_ip: rewrite.map(str::to_string),
            allow: allow.iter().map(|s| s.to_string()).collect(),
            last_fetch: None,
        }
    }

    #[test]
    fn strips_localhost_comments_and_dupes() {
        let body = "# header\n127.0.0.1 localhost\n0.0.0.0 ads.example\n0.0.0.0 ads.example\n0.0.0.0 track.example # inline\n";
        let out = parse_list(body, &src(None, &[]));
        assert_eq!(
            out,
            vec![
                ("0.0.0.0".to_string(), "ads.example".to_string()),
                ("0.0.0.0".to_string(), "track.example".to_string())
            ]
        );
    }

    #[test]
    fn rewrite_ip_and_allowlist() {
        let body = "127.0.0.1 ads.example\n127.0.0.1 needed.example\n";
        let out = parse_list(body, &src(Some("0.0.0.0"), &["needed.example"]));
        assert_eq!(out, vec![("0.0.0.0".to_string(), "ads.example".to_string())]);
    }

    #[test]
    fn multiple_hostnames_per_line_expand() {
        let out = parse_list("0.0.0.0 a.example b.example\n", &src(None, &[]));
        assert_eq!(out.len(), 2);
    }
}
