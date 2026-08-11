mod backup;
mod cli;
mod commands;
mod config;
mod ctx;
mod diff;
mod exit;
mod hostsfile;
mod paths;
mod remote;
mod render;
mod ui;
mod validate;
mod zones;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use ctx::Ctx;

fn main() {
    // Rust глушит SIGPIPE, из-за чего `hostsctl list | head` падает с паникой
    // вместо тихого выхода. Возвращаем поведение обычной unix-утилиты.
    // SAFETY: вызов до старта любых потоков, обработчик — стандартный SIG_DFL.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = run() {
        eprintln!("{} {e:#}", ui::red("error:"));
        std::process::exit(exit::code_of(&e));
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Команды, которым конфиг не нужен (или которые его создают).
    match &cli.command {
        Command::Init(args) => return commands::setup::init(&cli, args),
        Command::Completions(args) => {
            commands::setup::completions(args);
            return Ok(());
        }
        Command::Man => {
            commands::docs::man().map_err(|e| exit::coded(exit::IO, e))?;
            return Ok(());
        }
        Command::Docs(cmd) => {
            commands::docs::run(cmd);
            return Ok(());
        }
        Command::ConfigPath(args) => {
            let p = match &cli.config {
                Some(p) => p.clone(),
                None => paths::config_path()?,
            };
            println!("{}", p.display());
            if args.all {
                let cfg = config::Config::load_main(&p)?;
                let base = p.parent().unwrap_or_else(|| std::path::Path::new("."));
                for z in zones::expand(&cfg.include_patterns(), base)? {
                    println!("{}", z.display());
                }
            }
            return Ok(());
        }
        _ => {}
    }

    let mut ctx = Ctx::load(&cli)?;

    match &cli.command {
        Command::Apply(args) => commands::apply::run_apply(&ctx, args),
        Command::Diff => commands::apply::run_diff(&ctx),
        Command::Status => commands::apply::run_status(&ctx),
        Command::Off(args) => commands::apply::run_off(&ctx, args),
        Command::List(args) => commands::entries::list(&ctx, args),
        Command::Search(args) => commands::entries::search(&ctx, args),
        Command::Add(args) => commands::entries::add(&mut ctx, args),
        Command::Rm(args) => commands::entries::rm(&mut ctx, args),
        Command::Enable(args) => commands::entries::toggle(&mut ctx, args, true),
        Command::Disable(args) => commands::entries::toggle(&mut ctx, args, false),
        Command::Group(cmd) => commands::groups::run(&mut ctx, cmd),
        Command::Zone(cmd) => commands::zonecmd::run(&mut ctx, cmd),
        Command::Source(cmd) => commands::sources::run(&mut ctx, cmd),
        Command::Backup(cmd) => commands::backups::run(&ctx, cmd),
        Command::Import(args) => commands::setup::import(&mut ctx, args),
        Command::Migrate(args) => commands::setup::migrate(&mut ctx, args),
        Command::Check => commands::setup::check(&ctx),
        Command::Edit(args) => commands::setup::edit(&ctx, args),
        Command::Init(_)
        | Command::Completions(_)
        | Command::ConfigPath(_)
        | Command::Man
        | Command::Docs(_) => unreachable!(),
    }
}
