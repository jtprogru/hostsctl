//! init / import / migrate / check / edit / completions.

use crate::cli::{Cli, CompletionsArgs, ImportArgs, InitArgs, MigrateArgs, Shell};
use crate::config::{Config, Entry, Group};
use crate::ctx::Ctx;
use crate::{exit, hostsfile, ui, validate};
use anyhow::{Context, Result, anyhow, bail};
use clap::CommandFactory;
use std::path::{Path, PathBuf};

pub fn init(cli: &Cli, args: &InitArgs) -> Result<()> {
    let path = match &cli.config {
        Some(p) => p.clone(),
        None => crate::paths::config_path()?,
    };
    if path.exists() && !args.force {
        bail!("config {} already exists\n  overwrite it: hostsctl init --force", path.display());
    }

    let mut cfg = Config::default();
    if let Some(dir) = &args.from {
        let groups = collect_from(std::slice::from_ref(dir), None)?;
        if !groups.is_empty() {
            cfg.groups = groups;
        }
    }
    if let Some(t) = &cli.target {
        cfg.settings.target = t.clone();
    }

    if cli.dry_run {
        println!("{}", serde_yaml::to_string(&cfg)?);
        ui::info(&format!("dry-run: config {} was not created", path.display()));
        return Ok(());
    }
    cfg.save(&path, &[])?;
    ui::ok(&format!("created {}", path.display()));
    ui::info("next: hostsctl add 127.0.0.1 my.local && sudo hostsctl apply");
    Ok(())
}

pub fn import(ctx: &mut Ctx, args: &ImportArgs) -> Result<()> {
    let imported = collect_from(&args.paths, args.group.as_deref())?;
    if imported.is_empty() {
        bail!("nothing to import: no *.hosts found");
    }

    let mut added_entries = 0;
    let mut added_groups = 0;
    for g in imported {
        if ctx.cfg.group(&g.name).is_none() {
            added_groups += 1;
            added_entries += g.entries.len();
            ctx.cfg.groups.push(g);
            continue;
        }
        // Группа уже есть — доливаем только новые имена.
        let existing: Vec<String> = ctx
            .cfg
            .group(&g.name)
            .expect("checked above")
            .entries
            .iter()
            .flat_map(|e| e.hostnames.iter().map(|h| h.to_lowercase()))
            .collect();
        let target = ctx.cfg.require_group_mut(&g.name)?;
        for e in g.entries {
            if e.hostnames.iter().any(|h| existing.contains(&h.to_lowercase())) {
                continue;
            }
            added_entries += 1;
            target.entries.push(e);
        }
    }

    ctx.save()?;
    ui::ok(&format!("imported {added_entries} entries, {added_groups} new groups"));
    report_issues(&ctx.cfg);
    ui::info("apply it with: sudo hostsctl apply");
    Ok(())
}

pub fn migrate(ctx: &mut Ctx, args: &MigrateArgs) -> Result<()> {
    let file = hostsfile::HostsFile::load(&ctx.target)?;
    let has_legacy = file.legacy().is_some();

    import(ctx, &ImportArgs { paths: vec![args.from.clone()], group: None })?;

    if !has_legacy {
        ui::info(&format!("{} has no hosts-sync block — the import is done", ctx.target.display()));
        return Ok(());
    }
    ui::info("the hosts-sync block will now be dropped, its entries are already in the config");
    super::apply::run_apply(
        ctx,
        &crate::cli::ApplyArgs { yes: args.yes, no_flush: false, drop_legacy: true },
    )
}

pub fn check(ctx: &Ctx) -> Result<()> {
    let issues = validate::check_config(&ctx.cfg);
    let errors: Vec<String> = issues
        .iter()
        .filter(|i| i.level == validate::Level::Error)
        .map(|i| format!("{}: {}", i.where_, i.message))
        .collect();

    // plan даёт те же предупреждения плюс конфликты с содержимым hosts;
    // если он не собрался (есть ошибки), берём предупреждения напрямую.
    let warnings = match super::apply::plan(ctx, false, true) {
        Ok(p) => p.warnings,
        Err(_) => issues
            .iter()
            .filter(|i| i.level == validate::Level::Warn)
            .map(|i| format!("{}: {}", i.where_, i.message))
            .collect(),
    };

    for e in ui::dedup_counted(&errors) {
        println!("{} {e}", ui::red("error"));
    }
    let warnings = ui::dedup_counted(&warnings);
    for w in &warnings {
        println!("{} {w}", ui::yellow("warn "));
    }

    if errors.is_empty() && warnings.is_empty() {
        ui::ok("the config is clean");
        return Ok(());
    }
    // Тот же код, что и у apply на сломанном конфиге: скрипт различает
    // «конфиг чинит человек» (3) и «не хватило прав» (4), не читая текст.
    if !errors.is_empty() {
        return Err(exit::coded(
            exit::CONFIG,
            anyhow!(
                "{}, {}",
                ui::plural(errors.len(), "error"),
                ui::plural(warnings.len(), "warning")
            ),
        ));
    }
    ui::info(&format!("{}, no errors", ui::plural(warnings.len(), "warning")));
    Ok(())
}

pub fn edit(ctx: &Ctx, args: &crate::cli::EditArgs) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    if !ctx.config_path.exists() {
        bail!("config {} does not exist — run hostsctl init", ctx.config_path.display());
    }
    // Без аргумента правим основной конфиг, с именем группы — её файл.
    let path = match &args.group {
        None => ctx.config_path.clone(),
        Some(name) => {
            let g = ctx.cfg.group(name).ok_or_else(|| anyhow::anyhow!("no such group '{name}'"))?;
            g.origin.path().unwrap_or(&ctx.config_path).to_path_buf()
        }
    };
    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("cannot start the editor '{editor}'"))?;
    if !status.success() {
        bail!("{editor} exited with an error");
    }
    // После правки конфиг обязан читаться — иначе о поломке узнаешь при apply.
    let (cfg, zone_paths) = Config::load(&ctx.config_path)?;
    let fresh = Ctx {
        config_path: ctx.config_path.clone(),
        cfg,
        zone_paths,
        target: ctx.target.clone(),
        cache_dir: ctx.cache_dir.clone(),
        dry_run: ctx.dry_run,
    };
    check(&fresh)
}

pub fn completions(args: &CompletionsArgs) {
    use clap_complete::{Shell as CS, generate};
    let shell = match args.shell {
        Shell::Bash => CS::Bash,
        Shell::Zsh => CS::Zsh,
        Shell::Fish => CS::Fish,
        Shell::Elvish => CS::Elvish,
        Shell::PowerShell => CS::PowerShell,
    };
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "hostsctl", &mut std::io::stdout());
}

fn report_issues(cfg: &Config) {
    let issues = validate::check_config(cfg);
    let errors: Vec<String> = issues
        .iter()
        .filter(|i| i.level == validate::Level::Error)
        .map(|i| format!("{}: {}", i.where_, i.message))
        .collect();
    let warns: Vec<String> = issues
        .iter()
        .filter(|i| i.level == validate::Level::Warn)
        .map(|i| format!("{}: {}", i.where_, i.message))
        .collect();
    for e in ui::dedup_counted(&errors) {
        eprintln!("  {} {e}", ui::red("error"));
    }
    for w in ui::dedup_counted(&warns) {
        ui::warn(&w);
    }
}

/// Читает *.hosts из файлов и каталогов в группы.
fn collect_from(paths: &[PathBuf], single_group: Option<&str>) -> Result<Vec<Group>> {
    let mut files = vec![];
    for p in paths {
        if p.is_dir() {
            let mut in_dir: Vec<PathBuf> = std::fs::read_dir(p)
                .with_context(|| format!("cannot read directory {}", p.display()))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "hosts"))
                .collect();
            in_dir.sort();
            files.extend(in_dir);
        } else if p.is_file() {
            files.push(p.clone());
        } else {
            bail!("no such path: {}", p.display());
        }
    }

    let mut groups: Vec<Group> = vec![];
    for f in files {
        let name = single_group.map(str::to_string).unwrap_or_else(|| group_name_from(&f));
        let (header, entries) = parse_hosts_file(&f)?;
        if entries.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|g| g.name == name) {
            Some(g) => g.entries.extend(entries),
            None => {
                let mut g = Group::new(name);
                // Шапка файла — это описание группы, а не комментарий к первой записи.
                g.description = header.or_else(|| Some(format!("imported from {}", f.display())));
                g.entries = entries;
                groups.push(g);
            }
        }
    }
    Ok(groups)
}

/// `10-local.hosts` → `local`.
fn group_name_from(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".into());
    let trimmed = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
    if trimmed.is_empty() { stem } else { trimmed.to_string() }
}

/// Комментарий над строкой или в конце строки становится comment записи.
/// Первый комментарий файла возвращается отдельно — это шапка.
fn parse_hosts_file(path: &Path) -> Result<(Option<String>, Vec<Entry>)> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut out = vec![];
    let mut pending: Option<String> = None;
    let mut header: Option<String> = None;
    let mut first_line = true;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            pending = None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            // Закомментированная запись остаётся записью, только выключенной.
            if let Some(h) = hostsfile::parse_line(rest) {
                let mut e = Entry::new(h.ip, h.hostnames);
                e.enabled = false;
                e.comment = pending.take();
                out.push(e);
            } else if first_line && out.is_empty() {
                header = Some(rest.trim().to_string());
                first_line = false;
            } else {
                pending = Some(rest.trim().to_string());
            }
            continue;
        }
        first_line = false;
        let Some(h) = hostsfile::parse_line(trimmed) else {
            pending = None;
            continue;
        };
        let inline = trimmed.split_once('#').map(|(_, c)| c.trim().to_string());
        let mut e = Entry::new(h.ip, h.hostnames);
        e.comment = inline.or_else(|| pending.take());
        out.push(e);
    }
    Ok((header, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_name_drops_numeric_prefix() {
        assert_eq!(group_name_from(Path::new("10-local.hosts")), "local");
        assert_eq!(group_name_from(Path::new("20-blocklist.hosts")), "blocklist");
        assert_eq!(group_name_from(Path::new("dev.hosts")), "dev");
    }

    #[test]
    fn parses_comments_and_disabled_entries() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("10-local.hosts");
        std::fs::write(
            &f,
            "# Local development\n127.0.0.1 a.local\n10.0.0.1 b.local # a note\n# 127.0.0.1 old.local\n",
        )
        .unwrap();

        let (header, entries) = parse_hosts_file(&f).unwrap();
        assert_eq!(header.as_deref(), Some("Local development"));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].comment, None, "the file header is not an entry comment");
        assert_eq!(entries[1].comment.as_deref(), Some("a note"));
        assert!(!entries[2].enabled);
        assert_eq!(entries[2].hostnames, vec!["old.local"]);
    }

    #[test]
    fn collects_dir_into_groups_by_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("10-local.hosts"), "127.0.0.1 a\n").unwrap();
        std::fs::write(dir.path().join("20-block.hosts"), "0.0.0.0 b\n").unwrap();
        let groups = collect_from(&[dir.path().to_path_buf()], None).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "local");
        assert_eq!(groups[1].name, "block");
    }
}
