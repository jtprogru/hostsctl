//! Управление списком подключённых файлов-зон (`include` в конфиге).

use crate::cli::ZoneCmd;
use crate::ctx::Ctx;
use crate::ui;
use crate::zones;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub fn run(ctx: &mut Ctx, cmd: &ZoneCmd) -> Result<()> {
    let base = ctx.config_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();

    match cmd {
        ZoneCmd::List => {
            println!("{}", ui::bold("include"));
            for p in ctx.cfg.include_patterns() {
                println!("  {p}");
            }
            println!("{}", ui::bold("\nfiles"));
            if ctx.zone_paths.is_empty() {
                println!("  {}", ui::dim("no pattern matched anything"));
            }
            for zp in &ctx.zone_paths {
                let groups: Vec<&crate::config::Group> = ctx
                    .cfg
                    .groups
                    .iter()
                    .filter(|g| g.origin.path() == Some(zp.as_path()))
                    .collect();
                let entries: usize =
                    groups.iter().flat_map(|g| g.entries.iter().map(|e| e.ips.len())).sum();
                let kind = match zones::kind_of(zp) {
                    zones::Kind::Yaml => "yaml ",
                    zones::Kind::Hosts => "hosts",
                };
                let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
                println!(
                    "  {kind} {:<40} {:>4} entries  {}",
                    display_rel(zp, &base),
                    entries,
                    ui::dim(&names.join(", "))
                );
            }
            let main_groups =
                ctx.cfg.groups.iter().filter(|g| g.origin == zones::Origin::Main).count();
            println!(
                "\n  {}",
                ui::dim(&format!(
                    "groups in {}: {main_groups}",
                    ctx.config_path.file_name().unwrap_or_default().to_string_lossy(),
                ))
            );
            Ok(())
        }

        ZoneCmd::Add { pattern } => {
            let mut patterns = ctx.cfg.include_patterns();
            if patterns.iter().any(|p| p == pattern) {
                bail!("pattern '{pattern}' is already in include");
            }
            let before = zones::expand(&patterns, &base)?;
            patterns.push(pattern.clone());
            let after = zones::expand(&patterns, &base)?;
            let new_files: Vec<PathBuf> =
                after.into_iter().filter(|p| !before.contains(p)).collect();

            // Конфликт имён лучше поймать сейчас, а не при следующем запуске.
            let mut added_groups = 0;
            for f in &new_files {
                for g in zones::load(f)? {
                    if let Some(prev) = ctx.cfg.group(&g.name) {
                        bail!(
                            "group '{}' from {} already exists in {}",
                            g.name,
                            display_rel(f, &base),
                            prev.origin.label(&ctx.config_path)
                        );
                    }
                    added_groups += 1;
                    ctx.cfg.groups.push(g);
                }
            }

            ctx.cfg.include = Some(patterns);
            ctx.zone_paths.extend(new_files.iter().cloned());
            ctx.save()?;
            ui::ok(&format!(
                "'{pattern}' attached: {} files, {added_groups} groups",
                new_files.len()
            ));
            for f in &new_files {
                println!("   {}", display_rel(f, &base));
            }
            if added_groups > 0 {
                ui::info("apply it with: sudo hostsctl apply");
            }
            Ok(())
        }

        ZoneCmd::Rm { pattern } => {
            let patterns = ctx.cfg.include_patterns();
            if !patterns.iter().any(|p| p == pattern) {
                bail!("pattern '{pattern}' is not in include\n  list them: hostsctl zone list");
            }
            let kept: Vec<String> = patterns.into_iter().filter(|p| p != pattern).collect();
            let still = zones::expand(&kept, &base)?;
            let dropped: Vec<PathBuf> =
                ctx.zone_paths.iter().filter(|p| !still.contains(p)).cloned().collect();

            // Группы отключаемых файлов из памяти убираем, иначе save запишет
            // их обратно и заодно потащит в /etc/hosts.
            ctx.cfg
                .groups
                .retain(|g| g.origin.path().is_none_or(|p| !dropped.iter().any(|d| d == p)));
            ctx.cfg.include = Some(kept);
            ctx.zone_paths.retain(|p| still.contains(p));
            ctx.save()?;

            ui::ok(&format!("'{pattern}' detached"));
            for f in &dropped {
                println!("   {} {}", display_rel(f, &base), ui::dim("(the file stays on disk)"));
            }
            ui::info("apply it with: sudo hostsctl apply");
            Ok(())
        }
    }
}

fn display_rel(path: &Path, base: &Path) -> String {
    path.strip_prefix(base).unwrap_or(path).to_string_lossy().to_string()
}
