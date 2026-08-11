//! Линтер конфига: ловит то, что hosts молча проглотит и не применит.

use crate::config::Config;
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub level: Level,
    pub where_: String,
    pub message: String,
}

impl Issue {
    fn err(where_: impl Into<String>, message: impl Into<String>) -> Self {
        Self { level: Level::Error, where_: where_.into(), message: message.into() }
    }
    fn warn(where_: impl Into<String>, message: impl Into<String>) -> Self {
        Self { level: Level::Warn, where_: where_.into(), message: message.into() }
    }
}

pub fn check_config(cfg: &Config) -> Vec<Issue> {
    let mut out = vec![];
    for g in &cfg.groups {
        if g.name.trim().is_empty() {
            out.push(Issue::err("(group)", "empty group name"));
        }
        if g.is_remote() && !g.entries.is_empty() {
            out.push(Issue::warn(
                &g.name,
                "the group has both source and entries — entries are ignored when rendering",
            ));
        }
        if let Some(src) = &g.source {
            if !src.url.starts_with("http://") && !src.url.starts_with("https://") {
                out.push(Issue::err(&g.name, format!("source.url is not http(s): {}", src.url)));
            }
            if src.url.starts_with("http://") {
                out.push(Issue::warn(
                    &g.name,
                    "source.url has no TLS — the list can be tampered with",
                ));
            }
            if let Some(ip) = &src.rewrite_ip
                && ip.parse::<IpAddr>().is_err()
            {
                out.push(Issue::err(&g.name, format!("rewrite_ip is not an IP address: {ip}")));
            }
        }
        for e in &g.entries {
            let at = format!("{}: {}", g.name, e.hostnames.join(" "));
            let mut issues = vec![];
            if e.ips.is_empty() {
                issues.push(Issue::err(&at, "entry without a single IP address"));
            }
            for ip in &e.ips {
                if ip.parse::<IpAddr>().is_err() {
                    issues.push(Issue::err(&at, format!("'{ip}' is not an IP address")));
                }
            }
            if e.hostnames.is_empty() {
                issues.push(Issue::err(
                    &g.name,
                    format!("entry {} has no hostnames", e.ips.join(", ")),
                ));
            }
            // Один и тот же адрес дважды в одной записи — опечатка.
            let mut uniq = std::collections::HashSet::new();
            for ip in &e.ips {
                if !uniq.insert(ip) {
                    issues.push(Issue::warn(&at, format!("address {ip} is repeated in the entry")));
                }
            }
            for h in &e.hostnames {
                issues.extend(check_hostname(&at, h));
            }
            // Выключенная запись в hosts не попадает, ломать apply ей незачем.
            let active = g.enabled && e.enabled;
            if !active {
                for i in issues.iter_mut() {
                    if i.level == Level::Error {
                        i.level = Level::Warn;
                        i.message = format!("{} (entry is disabled)", i.message);
                    }
                }
            }
            out.extend(issues);
        }
    }
    out
}

fn check_hostname(at: &str, host: &str) -> Vec<Issue> {
    let mut out = vec![];
    if host.contains('*') {
        out.push(Issue::err(
            at,
            format!("'{host}': hosts does not support wildcards — use dnsmasq or /etc/resolver/"),
        ));
        return out;
    }
    // Завершающая точка (`example.com.`) намеренно не считается проблемой:
    // проверено на macOS — запрос без точки такую строку находит.
    if host.contains(':') || host.contains('/') {
        out.push(Issue::err(
            at,
            format!("'{host}': a port or a path does not work in hosts — hostname only"),
        ));
        return out;
    }
    let bare = host.trim_end_matches('.');
    if bare.is_empty() {
        out.push(Issue::err(at, "empty hostname"));
        return out;
    }
    if bare.len() > 253 {
        out.push(Issue::err(at, format!("'{host}': longer than 253 characters")));
    }
    for label in bare.split('.') {
        if label.is_empty() {
            out.push(Issue::err(at, format!("'{host}': empty label (two dots in a row)")));
            break;
        }
        if label.len() > 63 {
            out.push(Issue::err(
                at,
                format!("'{host}': label '{label}' is longer than 63 characters"),
            ));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            out.push(Issue::err(at, format!("'{host}': invalid characters in label '{label}'")));
        }
        if label.starts_with('-') || label.ends_with('-') {
            out.push(Issue::warn(
                at,
                format!("'{host}': label '{label}' starts or ends with a hyphen"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Entry, Group};

    fn cfg(entries: Vec<Entry>) -> Config {
        let mut g = Group::new("g");
        g.entries = entries;
        Config { version: 1, settings: Default::default(), include: None, groups: vec![g] }
    }

    #[test]
    fn rejects_wildcards_and_bad_ip() {
        let issues = check_config(&cfg(vec![
            Entry::new("127.0.0.1", vec!["*.k8s.orb.local".into()]),
            Entry::new("not-an-ip", vec!["ok.local".into()]),
        ]));
        assert_eq!(issues.iter().filter(|i| i.level == Level::Error).count(), 2);
    }

    #[test]
    fn trailing_dot_is_fine() {
        // Проверено на macOS: запрос без точки находит строку с точкой.
        let issues =
            check_config(&cfg(vec![Entry::new("0.0.0.0", vec!["analytics.google.com.".into()])]));
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn checks_every_ip_of_an_entry() {
        let mut e = Entry::new("10.0.0.1", vec!["multi.local".into()]);
        e.ips.push("not-an-ip".into());
        e.ips.push("10.0.0.1".into());
        let issues = check_config(&cfg(vec![e]));
        assert_eq!(issues.iter().filter(|i| i.level == Level::Error).count(), 1);
        assert!(issues.iter().any(|i| i.level == Level::Warn && i.message.contains("is repeated")));
    }

    #[test]
    fn accepts_normal_entry() {
        let issues =
            check_config(&cfg(vec![Entry::new("10.30.13.37", vec!["sre-mcp.local".into()])]));
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn rejects_port_in_hostname() {
        let issues =
            check_config(&cfg(vec![Entry::new("127.0.0.1", vec!["api.local:8080".into()])]));
        assert!(issues.iter().any(|i| i.level == Level::Error));
    }
}
