//! Бэкапы и откат.

use crate::cli::BackupCmd;
use crate::ctx::{Ctx, now_stamp};
use crate::{backup, diff, ui};
use anyhow::{Result, bail};

pub fn run(ctx: &Ctx, cmd: &BackupCmd) -> Result<()> {
    let dir = ctx.backup_dir();
    match cmd {
        BackupCmd::List => {
            let all = backup::list(&dir);
            if all.is_empty() {
                ui::info(&format!("no snapshots in {}", dir.display()));
                return Ok(());
            }
            println!("{}", ui::dim(&format!("{}", dir.display())));
            for (i, b) in all.iter().enumerate() {
                let tag = if i == 0 { ui::green(" ← latest") } else { String::new() };
                println!("  {:<20} {:>7} B{tag}", b.id, b.size);
            }
            Ok(())
        }

        BackupCmd::Prune => {
            if ctx.dry_run {
                let extra = backup::list(&dir).len().saturating_sub(ctx.cfg.settings.keep_backups);
                ui::info(&format!("dry-run: would delete {extra} snapshots"));
                return Ok(());
            }
            // Каталог бэкапов обычно root-овый, а remove_file молча возвращает
            // ошибку — без этой проверки prune «удалял» бы ноль снимков и
            // рапортовал об успехе.
            crate::paths::ensure_dir_writable(&dir, "backup prune")?;
            let n = backup::prune(&dir, ctx.cfg.settings.keep_backups);
            ui::ok(&format!("deleted {}", ui::plural(n, "snapshot")));
            Ok(())
        }

        BackupCmd::Restore { id, yes } => {
            let b = match id {
                Some(i) => backup::find(&dir, i)?,
                None => backup::latest(&dir)?,
            };
            let current = std::fs::read_to_string(&ctx.target)?;
            let restored = std::fs::read_to_string(&b.path)?;
            if current == restored {
                ui::info(&format!("{} already matches snapshot {}", ctx.target.display(), b.id));
                return Ok(());
            }
            print!("{}", diff::render(&current, &restored, &ctx.target.display().to_string()));

            if ctx.dry_run {
                ui::info("dry-run: nothing was restored");
                return Ok(());
            }
            crate::paths::ensure_writable(&ctx.target, "backup restore")?;
            crate::paths::ensure_dir_writable(&dir, "backup restore")?;
            if !yes
                && !ui::confirm(&format!(
                    "Roll {} back to snapshot {}?",
                    ctx.target.display(),
                    b.id
                ))
            {
                bail!("cancelled");
            }
            // Перед откатом снимаем текущее состояние — чтобы откат откатывался.
            let safety = backup::create(&ctx.target, &dir, &now_stamp())?;
            backup::restore(&b, &ctx.target)?;
            ui::ok(&format!("{} restored from {}", ctx.target.display(), b.id));
            println!("   snapshot taken before the restore: {}", safety.display());
            if ctx.cfg.settings.flush_dns {
                for note in crate::hostsfile::flush_dns(&ctx.target) {
                    ui::warn(&note);
                }
            }
            Ok(())
        }
    }
}
