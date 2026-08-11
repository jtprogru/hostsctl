//! Общий контекст команд: где конфиг, куда пишем, какое сейчас время.

use crate::cli::Cli;
use crate::config::Config;
use anyhow::Result;
use std::path::PathBuf;
use time::OffsetDateTime;
use time::macros::format_description;

pub struct Ctx {
    pub config_path: PathBuf,
    pub cfg: Config,
    /// Файлы-зоны, найденные при загрузке: их надо переписать даже пустыми.
    pub zone_paths: Vec<PathBuf>,
    pub target: PathBuf,
    pub cache_dir: PathBuf,
    pub dry_run: bool,
}

impl Ctx {
    pub fn load(cli: &Cli) -> Result<Self> {
        let config_path = match &cli.config {
            Some(p) => p.clone(),
            None => crate::paths::config_path()?,
        };
        let (cfg, zone_paths) = Config::load(&config_path)?;
        let target = cli.target.clone().unwrap_or_else(|| cfg.settings.target.clone());
        Ok(Self {
            config_path,
            cfg,
            zone_paths,
            target,
            cache_dir: crate::paths::cache_dir()?,
            dry_run: cli.dry_run,
        })
    }

    /// Сохраняет конфиг; в dry-run только сообщает, что сохранил бы.
    pub fn save(&self) -> Result<()> {
        if self.dry_run {
            crate::ui::info(&format!(
                "dry-run: config {} left unchanged",
                self.config_path.display()
            ));
            return Ok(());
        }
        self.cfg.save(&self.config_path, &self.zone_paths)
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.cfg.settings.backup_dir.clone()
    }
}

pub fn now_human() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
    now().format(&fmt).unwrap_or_else(|_| "unknown".into())
}

pub fn now_stamp() -> String {
    let fmt = format_description!("[year][month][day]-[hour][minute][second]");
    now().format(&fmt).unwrap_or_else(|_| "unknown".into())
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}
