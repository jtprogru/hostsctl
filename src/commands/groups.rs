//! Команды группы: list / add / rm / enable / disable.

use crate::cli::GroupCmd;
use crate::config::Group;
use crate::ctx::Ctx;
use crate::ui;
use crate::zones::{self, Origin};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Куда положить новую группу: в основной конфиг или в файл-зону.
///
/// Путь без каталога считается относительным каталогу конфига. Если файл не
/// попадает под include, шаблон дописывается — иначе группа потерялась бы
/// при следующем чтении.
pub fn resolve_origin(ctx: &mut Ctx, file: Option<&Path>) -> Result<Origin> {
    let Some(file) = file else {
        return Ok(Origin::Main);
    };
    let base = ctx.config_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let path = if file.is_absolute() { file.to_path_buf() } else { base.join(file) };
    if path == ctx.config_path {
        return Ok(Origin::Main);
    }
    if path.extension().is_none() {
        bail!("zone {} has no extension — it needs .yaml or .hosts", path.display());
    }

    let patterns = ctx.cfg.include_patterns();
    // Файла может ещё не быть, поэтому проверяем шаблоном по пути, а не поиском.
    if !covered(&path, &patterns, &base) {
        let rel = path
            .strip_prefix(&base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        let mut include = patterns;
        include.push(rel.clone());
        ctx.cfg.include = Some(include);
        ui::info(&format!("'{rel}' was added to include"));
    }
    Ok(Origin::Zone(path))
}

fn covered(path: &Path, patterns: &[String], base: &Path) -> bool {
    if zones::is_covered(path, patterns, base) {
        return true;
    }
    // Файла ещё нет на диске — сверяем сам шаблон.
    patterns.iter().any(|pat| {
        let full = if Path::new(pat).is_absolute() { PathBuf::from(pat) } else { base.join(pat) };
        glob::Pattern::new(&full.to_string_lossy()).is_ok_and(|p| p.matches_path(path))
    })
}

pub fn run(ctx: &mut Ctx, cmd: &GroupCmd) -> Result<()> {
    match cmd {
        GroupCmd::List => {
            if ctx.cfg.groups.is_empty() {
                ui::info("no groups");
            } else {
                println!(
                    "{}",
                    ui::dim(&format!(
                        "    {:<6} {:<16} {:>7}  {:<22} description",
                        "type", "name", "entries", "file"
                    ))
                );
            }
            for g in &ctx.cfg.groups {
                let state = if g.enabled { ui::green("on ") } else { ui::dim("off") };
                let kind = if g.is_remote() { "remote" } else { "local " };
                let count = if g.is_remote() {
                    crate::remote::meta_for(&g.name, &ctx.cache_dir).map(|m| m.lines).unwrap_or(0)
                } else {
                    g.entries.iter().map(|e| e.ips.len()).sum()
                };
                let desc = g.description.clone().unwrap_or_default();
                println!(
                    "{state} {kind} {:<16} {count:>7}  {:<22} {}",
                    g.name,
                    ui::dim(&g.origin.label(&ctx.config_path)),
                    ui::dim(&desc)
                );
            }
            Ok(())
        }

        GroupCmd::Move { name, file } => {
            let target = match file.as_deref() {
                Some(f) if f == Path::new("main") => Origin::Main,
                other => resolve_origin(ctx, other)?,
            };
            let main = ctx.config_path.clone();
            let g = ctx.cfg.require_group_mut(name)?;
            if g.origin == target {
                ui::info(&format!("group '{name}' is already in {}", target.label(&main)));
                return Ok(());
            }
            let from = g.origin.label(&main);
            g.origin = target.clone();
            ctx.save()?;
            ui::ok(&format!("group '{name}': {from} → {}", target.label(&main)));
            Ok(())
        }
        GroupCmd::Add { name, description, file, disabled } => {
            if ctx.cfg.group(name).is_some() {
                bail!("group '{name}' already exists");
            }
            let mut g = Group::new(name.clone());
            g.enabled = !disabled;
            g.description = description.clone();
            g.origin = resolve_origin(ctx, file.as_deref())?;
            let where_ = g.origin.label(&ctx.config_path);
            ctx.cfg.groups.push(g);
            ctx.save()?;
            ui::ok(&format!("group '{name}' created in {where_}"));
            Ok(())
        }
        GroupCmd::Rm { name, yes } => {
            let g = ctx.cfg.group(name).ok_or_else(|| anyhow::anyhow!("no such group '{name}'"))?;
            let n = g.entries.len();
            let remote = g.is_remote();
            if !yes && !ui::confirm(&format!("Delete group '{name}' and its {n} entries?")) {
                bail!("cancelled");
            }
            ctx.cfg.groups.retain(|g| !g.name.eq_ignore_ascii_case(name));
            ctx.save()?;
            if remote && !ctx.dry_run {
                crate::remote::drop_cache(name, &ctx.cache_dir);
            }
            ui::ok(&format!("group '{name}' deleted ({n} entries)"));
            ui::info("apply it with: hostsctl apply");
            Ok(())
        }
        GroupCmd::Enable { name, apply } => set_enabled(ctx, name, true, *apply),
        GroupCmd::Disable { name, apply } => set_enabled(ctx, name, false, *apply),
    }
}

fn set_enabled(ctx: &mut Ctx, name: &str, enabled: bool, apply: bool) -> Result<()> {
    let g = ctx.cfg.require_group_mut(name)?;
    if g.enabled == enabled {
        ui::info(&format!(
            "group '{name}' is already {}",
            if enabled { "enabled" } else { "disabled" }
        ));
        return Ok(());
    }
    g.enabled = enabled;
    ctx.save()?;
    ui::ok(&format!("group '{name}' {}", if enabled { "enabled" } else { "disabled" }));
    super::entries::maybe_apply(ctx, apply)
}
