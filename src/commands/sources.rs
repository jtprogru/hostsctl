//! Удалённые блоклисты.

use crate::cli::SourceCmd;
use crate::config::{Group, Source};
use crate::ctx::{Ctx, now_human};
use crate::{remote, ui};
use anyhow::{Result, bail};

pub fn run(ctx: &mut Ctx, cmd: &SourceCmd) -> Result<()> {
    match cmd {
        SourceCmd::List => {
            let remotes: Vec<_> = ctx.cfg.groups.iter().filter(|g| g.is_remote()).collect();
            if remotes.is_empty() {
                ui::info("no remote sources");
                ui::info("add one: hostsctl source add <url> --group ads --rewrite-ip 0.0.0.0");
            }
            for g in remotes {
                let src = g.source.as_ref().expect("is_remote");
                let state = if g.enabled { ui::green("on ") } else { ui::dim("off") };
                println!("{state} {}", ui::bold(&g.name));
                println!("     url:   {}", src.url);
                if let Some(ip) = &src.rewrite_ip {
                    println!("     ip:    {ip}");
                }
                if !src.allow.is_empty() {
                    println!("     allow: {}", src.allow.join(", "));
                }
                match remote::meta_for(&g.name, &ctx.cache_dir) {
                    Some(m) => {
                        println!("     cache: {} entries, {}", m.lines, m.fetched);
                        if m.url != src.url {
                            println!(
                                "     {}",
                                ui::yellow(&format!(
                                    "the cache came from a different url ({}) — run hostsctl source update",
                                    m.url
                                ))
                            );
                        }
                    }
                    None => {
                        println!("     cache: {}", ui::yellow("empty — run hostsctl source update"))
                    }
                }
            }
            Ok(())
        }

        SourceCmd::Add { url, group, rewrite_ip, allow, file, update } => {
            if ctx.cfg.group(group).is_some() {
                bail!("group '{group}' already exists — pick another name or delete it");
            }
            if !url.starts_with("http://") && !url.starts_with("https://") {
                bail!("the url has to start with http:// or https://");
            }
            if let Some(ip) = rewrite_ip
                && ip.parse::<std::net::IpAddr>().is_err()
            {
                bail!("--rewrite-ip '{ip}' is not an IP address");
            }
            let mut g = Group::new(group.clone());
            g.source = Some(Source {
                url: url.clone(),
                rewrite_ip: rewrite_ip.clone(),
                allow: allow.clone(),
                last_fetch: None,
            });
            g.origin = super::groups::resolve_origin(ctx, file.as_deref())?;
            ctx.cfg.groups.push(g);
            ctx.save()?;
            ui::ok(&format!("source '{group}' added"));
            if *update {
                update_groups(ctx, Some(group), false)?;
            } else {
                ui::info(&format!("download the list with: hostsctl source update {group}"));
            }
            Ok(())
        }

        SourceCmd::Rm { group, yes } => {
            let g =
                ctx.cfg.group(group).ok_or_else(|| anyhow::anyhow!("no such group '{group}'"))?;
            if !g.is_remote() {
                bail!("'{group}' is a plain group — remove it with hostsctl group rm");
            }
            if !yes && !ui::confirm(&format!("Delete source '{group}' and its cache?")) {
                bail!("cancelled");
            }
            ctx.cfg.groups.retain(|g| !g.name.eq_ignore_ascii_case(group));
            ctx.save()?;
            if !ctx.dry_run {
                remote::drop_cache(group, &ctx.cache_dir);
            }
            ui::ok(&format!("source '{group}' deleted"));
            ui::info("apply it with: hostsctl apply");
            Ok(())
        }

        SourceCmd::Update { group, force, apply } => {
            update_groups(ctx, group.as_deref(), *force)?;
            super::entries::maybe_apply(ctx, *apply)
        }
    }
}

fn update_groups(ctx: &mut Ctx, only: Option<&str>, force: bool) -> Result<()> {
    let targets: Vec<(String, Source)> = ctx
        .cfg
        .groups
        .iter()
        .filter(|g| g.is_remote())
        .filter(|g| only.is_none_or(|n| g.name.eq_ignore_ascii_case(n)))
        .map(|g| (g.name.clone(), g.source.clone().expect("is_remote")))
        .collect();

    if targets.is_empty() {
        match only {
            Some(n) => bail!("no remote source '{n}'"),
            None => {
                ui::info("no remote sources");
                return Ok(());
            }
        }
    }
    if ctx.dry_run {
        for (name, src) in &targets {
            ui::info(&format!("dry-run: would download {name} ← {}", src.url));
        }
        return Ok(());
    }

    let now = now_human();
    let mut failures = 0;
    for (name, src) in &targets {
        print!("{} {name} … ", ui::cyan("::"));
        use std::io::Write;
        let _ = std::io::stdout().flush();
        match remote::fetch(name, src, &ctx.cache_dir, force, &now) {
            Ok(o) if o.changed => {
                println!("{} entries ({} KiB)", o.entries, o.bytes / 1024);
                if let Some(g) = ctx.cfg.group_mut(name)
                    && let Some(s) = g.source.as_mut()
                {
                    s.last_fetch = Some(now.clone());
                }
            }
            Ok(o) => println!("{}", ui::dim(&format!("unchanged ({} entries)", o.entries))),
            Err(e) => {
                failures += 1;
                println!("{}", ui::red(&format!("error: {e:#}")));
            }
        }
    }
    ctx.save()?;
    if failures > 0 {
        bail!("{failures} sources failed to update");
    }
    ui::info("apply it with: hostsctl apply");
    Ok(())
}
