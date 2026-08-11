//! add / rm / enable / disable / list / search — работа с записями конфига.

use crate::cli::{AddArgs, ApplyArgs, ListArgs, RmArgs, SearchArgs, ToggleArgs};
use crate::config::{Entry, Group};
use crate::ctx::Ctx;
use crate::{ui, validate};
use anyhow::{Result, bail};

pub fn add(ctx: &mut Ctx, args: &AddArgs) -> Result<()> {
    let normalized: Vec<String> = args.hostnames.iter().map(|h| h.trim().to_string()).collect();

    // Точный повтор пары «адрес + имя» бессмыслен, а вот то же имя на другом
    // адресе — законная вторая A-запись, её пропускаем дальше.
    for h in &normalized {
        for (gi, ei) in ctx.cfg.find_hostname(h) {
            let g = &ctx.cfg.groups[gi];
            let e = &g.entries[ei];
            if e.has_ip(&args.ip) {
                bail!("'{} {h}' is already in group '{}'", args.ip, g.name);
            }
        }
    }

    let mut entry = Entry::new(args.ip.clone(), normalized.clone());
    entry.enabled = !args.disabled;
    entry.comment = args.comment.clone();

    let mut probe_group = Group::new(args.group.clone());
    probe_group.entries = vec![entry.clone()];
    let probe = crate::config::Config {
        version: crate::config::CURRENT_VERSION,
        settings: ctx.cfg.settings.clone(),
        include: None,
        groups: vec![probe_group],
    };
    let issues = validate::check_config(&probe);
    for i in &issues {
        match i.level {
            validate::Level::Error => eprintln!("  {} {}", ui::red("error"), i.message),
            validate::Level::Warn => ui::warn(&i.message),
        }
    }
    if issues.iter().any(|i| i.level == validate::Level::Error) {
        bail!("the entry was not added");
    }

    if ctx.cfg.group(&args.group).is_none() {
        let mut g = Group::new(args.group.clone());
        g.origin = super::groups::resolve_origin(ctx, args.file.as_deref())?;
        let where_ = g.origin.label(&ctx.config_path);
        ctx.cfg.groups.push(g);
        ui::info(&format!("created group '{}' in {where_}", args.group));
    }
    let g = ctx.cfg.require_group_mut(&args.group)?;
    if g.is_remote() {
        bail!("group '{}' is a remote list, manual entries do not go into it", args.group);
    }
    // Тот же набор имён — доливаем адрес в существующую запись, чтобы в конфиге
    // не плодились три записи с одинаковыми hostnames.
    let merged =
        g.entries.iter_mut().find(|e| e.same_hostnames(&normalized) && e.enabled == entry.enabled);
    match merged {
        Some(e) => {
            e.ips.push(args.ip.clone());
            if e.comment.is_none() {
                e.comment = entry.comment.clone();
            }
        }
        None => g.entries.push(entry),
    }

    ctx.save()?;
    let total = ctx
        .cfg
        .group(&args.group)
        .and_then(|g| g.entries.iter().find(|e| e.same_hostnames(&normalized)))
        .map(|e| e.ips.len())
        .unwrap_or(1);
    ui::ok(&format!(
        "{} → {} (group {}{})",
        normalized.join(" "),
        args.ip,
        args.group,
        if total > 1 { format!(", addresses for this name: {total}") } else { String::new() }
    ));
    maybe_apply(ctx, args.apply)
}

pub fn rm(ctx: &mut Ctx, args: &RmArgs) -> Result<()> {
    let mut removed = vec![];
    for target in &args.targets {
        let needle = target.trim_end_matches('.').to_lowercase();
        for g in ctx.cfg.groups.iter_mut() {
            if let Some(filter) = &args.group
                && !g.name.eq_ignore_ascii_case(filter)
            {
                continue;
            }
            if g.is_remote() {
                continue;
            }
            for e in g.entries.iter_mut() {
                // Удаление по IP снимает только этот адрес: у имени могут быть
                // и другие, и терять их нельзя.
                if e.ips.iter().any(|i| i.to_lowercase() == needle) {
                    removed.push(format!("{needle} {}", e.hostnames.join(" ")));
                    e.ips.retain(|i| i.to_lowercase() != needle);
                    continue;
                }
                let before = e.hostnames.len();
                e.hostnames.retain(|h| h.trim_end_matches('.').to_lowercase() != needle);
                if e.hostnames.len() != before {
                    removed.push(format!("{} {target}", e.ips.join(" ")));
                }
            }
            g.entries.retain(|e| !e.hostnames.is_empty() && !e.ips.is_empty());
        }
    }

    if removed.is_empty() {
        bail!("nothing matched: {}", args.targets.join(", "));
    }
    ctx.save()?;
    for r in &removed {
        ui::ok(&format!("removed: {r}"));
    }
    maybe_apply(ctx, args.apply)
}

pub fn toggle(ctx: &mut Ctx, args: &ToggleArgs, enable: bool) -> Result<()> {
    let mut touched = 0;
    for h in &args.hostnames {
        let hits = ctx.cfg.find_hostname(h);
        if hits.is_empty() {
            ui::warn(&format!("'{h}' is not in the config"));
            continue;
        }
        for (gi, ei) in hits {
            ctx.cfg.groups[gi].entries[ei].enabled = enable;
            touched += 1;
        }
    }
    if touched == 0 {
        bail!("nothing to change");
    }
    ctx.save()?;
    ui::ok(&format!("{touched} entries {}", if enable { "enabled" } else { "disabled" }));
    maybe_apply(ctx, args.apply)
}

pub fn list(ctx: &Ctx, args: &ListArgs) -> Result<()> {
    let mut shown = 0;
    for g in &ctx.cfg.groups {
        if let Some(f) = &args.group
            && !g.name.eq_ignore_ascii_case(f)
        {
            continue;
        }
        let state = if g.enabled { "" } else { " (disabled)" };
        let head = match &g.description {
            Some(d) => format!("{}{state} — {d}", g.name),
            None => format!("{}{state}", g.name),
        };
        println!("{}", ui::bold(&head));

        if let Some(src) = &g.source {
            let meta = crate::remote::meta_for(&g.name, &ctx.cache_dir);
            match meta {
                Some(m) => println!(
                    "  {}",
                    ui::dim(&format!("{} — {} entries, {}", src.url, m.lines, m.fetched))
                ),
                None => println!("  {}", ui::yellow(&format!("{} — not cached", src.url))),
            }
            continue;
        }

        if g.entries.is_empty() {
            println!("  {}", ui::dim("empty"));
        }
        for e in &g.entries {
            if !e.enabled && !args.all {
                continue;
            }
            shown += 1;
            let mark = if e.enabled { " " } else { "#" };
            let comment =
                e.comment.as_ref().map(|c| ui::dim(&format!("  # {c}"))).unwrap_or_default();
            // Строка на адрес — ровно то, что уйдёт в hosts.
            for (i, ip) in e.ips.iter().enumerate() {
                let line = format!("{mark} {:<15} {}", ip, e.hostnames.join(" "));
                let tail = if i == 0 { comment.clone() } else { String::new() };
                if e.enabled {
                    println!("{line}{tail}");
                } else {
                    println!("{}{tail}", ui::dim(&line));
                }
            }
        }
        println!();
    }
    if shown == 0 && args.group.is_none() && !args.all {
        ui::info("no enabled entries (see them all with: hostsctl list --all)");
    }
    Ok(())
}

pub fn search(ctx: &Ctx, args: &SearchArgs) -> Result<()> {
    let needle = args.pattern.to_lowercase();
    let mut found = 0;

    for (g, _, e) in ctx.cfg.iter_entries() {
        let hit = e.ips.iter().any(|i| i.to_lowercase().contains(&needle))
            || e.hostnames.iter().any(|h| h.to_lowercase().contains(&needle))
            || e.comment.as_ref().is_some_and(|c| c.to_lowercase().contains(&needle));
        if hit {
            found += 1;
            let mark = if e.enabled { " " } else { "#" };
            println!(
                "{mark} {:<15} {:<40} {}",
                e.ips.join(", "),
                e.hostnames.join(" "),
                ui::dim(&g.name)
            );
        }
    }

    // По удалённым спискам ищем в кеше — там основной объём.
    for g in ctx.cfg.groups.iter().filter(|g| g.is_remote()) {
        let Some(src) = &g.source else { continue };
        let Ok(Some(entries)) = crate::remote::cached_entries(&g.name, src, &ctx.cache_dir) else {
            continue;
        };
        for (ip, host) in entries.iter().filter(|(_, h)| h.to_lowercase().contains(&needle)) {
            found += 1;
            println!("  {ip:<15} {host:<40} {}", ui::dim(&g.name));
        }
    }

    if found == 0 {
        ui::info(&format!("nothing matches '{}'", args.pattern));
    }
    Ok(())
}

/// Общий хвост для команд с флагом `--apply`.
pub fn maybe_apply(ctx: &Ctx, apply: bool) -> Result<()> {
    if !apply {
        return Ok(());
    }
    if ctx.dry_run {
        ui::info("dry-run: apply skipped");
        return Ok(());
    }
    super::apply::run_apply(ctx, &ApplyArgs { yes: true, no_flush: false, drop_legacy: false })
}
