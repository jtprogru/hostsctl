//! apply / diff / off / status — всё, что трогает целевой файл.

use crate::cli::{ApplyArgs, YesArgs};
use crate::ctx::{Ctx, now_human, now_stamp};
use crate::exit;
use crate::hostsfile::HostsFile;
use crate::{backup, diff, hostsfile, render, ui, validate};
use anyhow::{Result, anyhow, bail};

/// Что даст apply: новое содержимое файла и всё, о чём стоит предупредить.
pub struct Plan {
    pub file: HostsFile,
    pub new_content: String,
    pub warnings: Vec<String>,
    pub hosts_count: usize,
    pub groups_used: usize,
    pub changed: bool,
}

pub fn plan(ctx: &Ctx, drop_legacy: bool, block: bool) -> Result<Plan> {
    let file = HostsFile::load(&ctx.target)?;
    let mut warnings = vec![];

    // Ошибки конфига блокируют запись — незачем рендерить заведомо мусор.
    // Печатает их вызывающий (в конечном счёте main), поэтому здесь они просто
    // складываются в текст ошибки: иначе `check` выводил бы их дважды.
    let issues = validate::check_config(&ctx.cfg);
    let errors: Vec<_> = issues.iter().filter(|i| i.level == validate::Level::Error).collect();
    if !errors.is_empty() {
        let listed: Vec<String> =
            errors.iter().map(|e| format!("\n  {}: {}", e.where_, e.message)).collect();
        return Err(exit::coded(
            exit::CONFIG,
            anyhow!(
                "the config has {} — fix them (hostsctl check){}",
                ui::plural(errors.len(), "error"),
                listed.concat()
            ),
        ));
    }
    warnings.extend(
        issues
            .iter()
            .filter(|i| i.level == validate::Level::Warn)
            .map(|i| format!("{}: {}", i.where_, i.message)),
    );

    let (new_content, hosts_count, groups_used) = if block {
        let r = render::render(&ctx.cfg, &ctx.cache_dir, &now_human(), &ctx.config_path)?;
        warnings.extend(r.warnings.clone());
        warnings.extend(render::conflicts_with_unmanaged(&file, &r.block));
        (
            render::preserve_timestamp(&file.raw, file.compose(Some(&r.block), drop_legacy)),
            r.hosts_count,
            r.groups_used,
        )
    } else {
        (file.compose(None, drop_legacy), 0, 0)
    };

    if file.legacy().is_some() && !drop_legacy {
        warnings.push(format!(
            "{} still holds a legacy hosts-sync block — its entries can win over ours; \
             move them over with: hostsctl migrate",
            ctx.target.display()
        ));
    }

    hostsfile::sanity_check(&new_content)?;
    let changed = new_content != file.raw;

    Ok(Plan { file, new_content, warnings, hosts_count, groups_used, changed })
}

pub fn run_apply(ctx: &Ctx, args: &ApplyArgs) -> Result<()> {
    let p = plan(ctx, args.drop_legacy, true)?;
    finish(ctx, p, args.yes, args.no_flush, "apply")
}

pub fn run_off(ctx: &Ctx, args: &YesArgs) -> Result<()> {
    let p = plan(ctx, false, false)?;
    if p.file.managed().is_none() {
        ui::info(&format!("{} has no managed block", ctx.target.display()));
        return Ok(());
    }
    finish(ctx, p, args.yes, false, "off")
}

pub fn run_diff(ctx: &Ctx) -> Result<()> {
    let p = plan(ctx, false, true)?;
    for w in ui::dedup_counted(&p.warnings) {
        ui::warn(&w);
    }
    if !p.changed {
        ui::info("no changes");
        return Ok(());
    }
    print!("{}", diff::render(&p.file.raw, &p.new_content, &ctx.target.display().to_string()));
    let (add, del) = diff::summary(&p.file.raw, &p.new_content);
    println!("{}", ui::dim(&format!("+{add} / -{del} lines")));
    Ok(())
}

fn finish(ctx: &Ctx, p: Plan, yes: bool, no_flush: bool, what: &str) -> Result<()> {
    for w in ui::dedup_counted(&p.warnings) {
        ui::warn(&w);
    }
    if !p.changed {
        ui::info(&format!("{} is already in the desired state", ctx.target.display()));
        return Ok(());
    }

    print!("{}", diff::render(&p.file.raw, &p.new_content, &ctx.target.display().to_string()));
    let (add, del) = diff::summary(&p.file.raw, &p.new_content);
    println!("{}", ui::dim(&format!("+{add} / -{del} lines")));

    if ctx.dry_run {
        ui::info("dry-run: nothing was written");
        return Ok(());
    }

    crate::paths::ensure_writable(&ctx.target, what)?;
    crate::paths::ensure_dir_writable(&ctx.backup_dir(), what)?;
    if !yes && !ui::confirm(&format!("Write {}?", ctx.target.display())) {
        bail!("cancelled");
    }

    let dir = ctx.backup_dir();
    let bak = backup::create(&ctx.target, &dir, &now_stamp())?;
    hostsfile::write_atomic(&ctx.target, &p.new_content, 0o644)?;
    let pruned = backup::prune(&dir, ctx.cfg.settings.keep_backups);

    ui::ok(&format!(
        "{} updated: {} entries from {} groups",
        ctx.target.display(),
        p.hosts_count,
        p.groups_used
    ));
    println!("   backup: {}", bak.display());
    if pruned > 0 {
        println!("   {}", ui::dim(&format!("old backups deleted: {pruned}")));
    }

    if !no_flush && ctx.cfg.settings.flush_dns {
        let notes = hostsfile::flush_dns(&ctx.target);
        for note in &notes {
            ui::warn(note);
        }
        if notes.is_empty() && ctx.target == std::path::Path::new("/etc/hosts") {
            println!("   DNS cache flushed");
        }
    }
    Ok(())
}

pub fn run_status(ctx: &Ctx) -> Result<()> {
    let file = HostsFile::load(&ctx.target)?;
    println!("{}", ui::bold("config"));
    println!("  file:    {}", ctx.config_path.display());
    println!("  groups:  {}", ctx.cfg.groups.len());
    println!("  zones:   {} ({})", ctx.zone_paths.len(), ctx.cfg.include_patterns().join(", "));
    let local: usize = ctx
        .cfg
        .groups
        .iter()
        .filter(|g| !g.is_remote())
        .flat_map(|g| g.entries.iter().map(|e| e.ips.len()))
        .sum();
    println!("  lines:   {local} from local groups");

    println!("{}", ui::bold("\ntarget"));
    println!("  file:  {}", ctx.target.display());
    match file.managed() {
        Some(b) => println!("  block: present, lines {}–{}", b.start + 1, b.end + 1),
        None => println!("  block: {}", ui::yellow("missing")),
    }
    if let Some(b) = file.legacy() {
        println!(
            "  {}",
            ui::yellow(&format!(
                "hosts-sync block: lines {}–{} (hostsctl leaves it alone)",
                b.start + 1,
                b.end + 1
            ))
        );
    }

    match plan(ctx, false, true) {
        Ok(p) if p.changed => {
            let (add, del) = diff::summary(&file.raw, &p.new_content);
            println!("  drift: {}", ui::yellow(&format!("+{add} / -{del} lines — apply needed")));
        }
        Ok(_) => println!("  drift: {}", ui::green("none, the file matches the config")),
        Err(e) => println!("  drift: {}", ui::red(&format!("cannot compute: {e}"))),
    }

    println!("{}", ui::bold("\ngroups"));
    for g in &ctx.cfg.groups {
        let state = if g.enabled { ui::green("on ") } else { ui::dim("off") };
        let count = if let Some(src) = &g.source {
            match crate::remote::meta_for(&g.name, &ctx.cache_dir) {
                Some(m) => format!("{} entries, updated {}", m.lines, m.fetched),
                None => ui::yellow(&format!("not cached ({})", src.url)),
            }
        } else {
            format!("{} entries", g.entries.len())
        };
        println!("  {state} {:<16} {count}", g.name);
    }

    let backups = backup::list(&ctx.backup_dir());
    println!("{}", ui::bold("\nbackups"));
    println!("  directory: {}", ctx.backup_dir().display());
    match backups.first() {
        Some(b) => println!("  snapshots: {} (latest {})", backups.len(), b.id),
        None => println!("  snapshots: none"),
    }
    Ok(())
}
