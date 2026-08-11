//! Общая обвязка интеграционных тестов: песочница с временным «/etc/hosts».

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

pub const SYSTEM_HEAD: &str = "\
##
# Host Database
##
127.0.0.1\tlocalhost
255.255.255.255\tbroadcasthost
::1             localhost

# a hand edit that must not be lost
10.0.0.5 nas.home
";

pub struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hosts"), SYSTEM_HEAD).unwrap();
        Self { dir }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub fn target(&self) -> PathBuf {
        self.path("hosts")
    }

    pub fn hosts(&self) -> String {
        std::fs::read_to_string(self.target()).unwrap()
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(bin())
            .args(args)
            .arg("--config")
            .arg(self.path("config.yaml"))
            .arg("--target")
            .arg(self.target())
            .env("HOSTSCTL_CACHE", self.path("cache"))
            .env("NO_COLOR", "1")
            .output()
            .expect("the binary started")
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?} failed:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    pub fn fails(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(!out.status.success(), "{args:?} should have failed");
        String::from_utf8_lossy(&out.stderr).to_string()
    }

    /// init + backup_dir внутри песочницы, чтобы не лезть в /var.
    pub fn init(&self) {
        self.ok(&["init"]);
        let cfg = self.path("config.yaml");
        let text = std::fs::read_to_string(&cfg).unwrap();
        let patched = text.replace(
            "backup_dir: /var/db/hostsctl/backups",
            &format!("backup_dir: {}", self.path("backups").display()),
        );
        let patched = if patched == text {
            // на linux путь другой
            text.lines()
                .map(|l| {
                    if l.trim_start().starts_with("backup_dir:") {
                        format!("  backup_dir: {}", self.path("backups").display())
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            patched
        };
        std::fs::write(&cfg, patched).unwrap();
    }
}

pub fn bin() -> PathBuf {
    // target/debug/deps/cli-<hash> → target/debug/hostsctl
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join(format!("hostsctl{}", std::env::consts::EXE_SUFFIX))
}

pub fn assert_system_lines_intact(hosts: &str) {
    for line in SYSTEM_HEAD.lines().filter(|l| !l.trim().is_empty()) {
        assert!(hosts.contains(line), "lost line: {line:?}\n---\n{hosts}");
    }
}
