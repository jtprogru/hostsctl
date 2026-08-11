//! Определение CLI.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "hostsctl",
    version,
    about = "Manage /etc/hosts from a YAML config",
    long_about = "hostsctl keeps entries in a YAML config and renders them into a managed block \
inside /etc/hosts. Everything outside the block markers — system lines and hand edits — is \
carried over untouched. Commands that write to /etc/hosts need root."
)]
pub struct Cli {
    /// Path to the config (default: ~/.config/hostsctl/config.yaml)
    #[arg(long, short, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// File to manage (default: /etc/hosts)
    #[arg(long, global = true, value_name = "FILE")]
    pub target: Option<PathBuf>,

    /// Write nothing, only show what would happen
    #[arg(long, short = 'n', global = true)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render the config into /etc/hosts (needs root)
    Apply(ApplyArgs),
    /// Show the diff between the current /etc/hosts and what apply would write
    Diff,
    /// Report on the config, the managed block, groups and backups
    Status,
    /// Remove the managed block from /etc/hosts, leaving the config alone (needs root)
    Off(YesArgs),
    /// List the entries in the config
    List(ListArgs),
    /// Find entries by a substring of a hostname or an IP
    Search(SearchArgs),
    /// Add an entry
    Add(AddArgs),
    /// Remove entries by hostname or IP
    #[command(visible_alias = "remove")]
    Rm(RmArgs),
    /// Enable entries by hostname
    Enable(ToggleArgs),
    /// Disable entries by hostname (they stay in the config)
    Disable(ToggleArgs),
    /// Groups of entries
    #[command(subcommand)]
    Group(GroupCmd),
    /// Zone files next to the config
    #[command(subcommand)]
    Zone(ZoneCmd),
    /// Remote blocklists
    #[command(subcommand)]
    Source(SourceCmd),
    /// Backups of /etc/hosts
    #[command(subcommand)]
    Backup(BackupCmd),
    /// Import existing *.hosts files into the config
    Import(ImportArgs),
    /// Move a legacy hosts-sync setup into the config and drop its block
    Migrate(MigrateArgs),
    /// Check the config for things /etc/hosts would silently ignore
    Check,
    /// Open the config (or a group's file) in $EDITOR and check it afterwards
    Edit(EditArgs),
    /// Create a config from scratch
    Init(InitArgs),
    /// Print the path to the config and to every attached zone
    ConfigPath(ConfigPathArgs),
    /// Print a shell completion script
    Completions(CompletionsArgs),
    /// Print the man page in roff format
    #[command(hide = true)]
    Man,
    /// Print documentation fragments generated from the code
    #[command(hide = true, subcommand)]
    Docs(DocsCmd),
}

#[derive(Args, Debug)]
pub struct ApplyArgs {
    /// Do not ask for confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// Do not flush the DNS cache
    #[arg(long)]
    pub no_flush: bool,
    /// Also drop the block left behind by the legacy hosts-sync
    #[arg(long)]
    pub drop_legacy: bool,
}

#[derive(Args, Debug)]
pub struct YesArgs {
    /// Do not ask for confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Only this group
    #[arg(long, short)]
    pub group: Option<String>,
    /// Show disabled entries too
    #[arg(long, short = 'a')]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Substring to look for in hostnames, addresses and comments
    pub pattern: String,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Address the hostnames resolve to
    pub ip: String,
    /// One or more hostnames
    #[arg(required = true)]
    pub hostnames: Vec<String>,
    /// Group (created when it does not exist)
    #[arg(long, short, default_value = "local")]
    pub group: String,
    /// Zone file for a new group (for example zones/work.yaml)
    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,
    /// Comment kept next to the entry
    #[arg(long)]
    pub comment: Option<String>,
    /// Add it disabled
    #[arg(long)]
    pub disabled: bool,
    /// Apply to /etc/hosts right away (needs root)
    #[arg(long)]
    pub apply: bool,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    /// Hostnames or IPs
    #[arg(required = true)]
    pub targets: Vec<String>,
    /// Look only in this group
    #[arg(long, short)]
    pub group: Option<String>,
    /// Apply to /etc/hosts right away (needs root)
    #[arg(long)]
    pub apply: bool,
}

#[derive(Args, Debug)]
pub struct ToggleArgs {
    /// One or more hostnames
    #[arg(required = true)]
    pub hostnames: Vec<String>,
    /// Apply to /etc/hosts right away (needs root)
    #[arg(long)]
    pub apply: bool,
}

#[derive(Subcommand, Debug)]
pub enum GroupCmd {
    /// List the groups
    List,
    /// Create a group
    Add {
        /// Group name
        name: String,
        /// Human-readable description, rendered as a comment in /etc/hosts
        #[arg(long, short)]
        description: Option<String>,
        /// Put the group in a zone file (for example zones/work.hosts)
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Create it disabled
        #[arg(long)]
        disabled: bool,
    },
    /// Move a group to another file ('main' means the main config)
    Move {
        /// Group name
        name: String,
        /// Destination zone file, or 'main' for the main config
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
    },
    /// Delete a group together with its entries
    Rm {
        /// Group name
        name: String,
        /// Do not ask for confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Enable a group
    Enable {
        /// Group name
        name: String,
        /// Apply to /etc/hosts right away (needs root)
        #[arg(long)]
        apply: bool,
    },
    /// Disable a group
    Disable {
        /// Group name
        name: String,
        /// Apply to /etc/hosts right away (needs root)
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ZoneCmd {
    /// Show the include patterns and the files they match
    List,
    /// Attach a file or a pattern (for example 'zones/*.hosts')
    Add {
        /// Path or glob, relative to the config directory
        pattern: String,
    },
    /// Detach a pattern; the files stay on disk
    Rm {
        /// Pattern exactly as it appears in include
        pattern: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SourceCmd {
    /// List the remote sources and the state of their cache
    List,
    /// Attach a remote hosts list as a group
    Add {
        /// http(s) address of the hosts list
        url: String,
        /// Group name
        #[arg(long, short)]
        group: String,
        /// Rewrite the IP of every entry (usually 0.0.0.0)
        #[arg(long)]
        rewrite_ip: Option<String>,
        /// Hostnames to keep out of the list
        #[arg(long)]
        allow: Vec<String>,
        /// Zone file for the group (.yaml only)
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Download it right away
        #[arg(long)]
        update: bool,
    },
    /// Detach a source (drops the group and its cache)
    Rm {
        /// Group the source belongs to
        group: String,
        /// Do not ask for confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Download the lists again
    Update {
        /// A single group; all of them when omitted
        group: Option<String>,
        /// Ignore the cached ETag
        #[arg(long)]
        force: bool,
        /// Apply to /etc/hosts right away (needs root)
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BackupCmd {
    /// List the snapshots
    List,
    /// Roll /etc/hosts back to a snapshot (needs root)
    Restore {
        /// Snapshot ID; the latest one when omitted
        id: Option<String>,
        /// Do not ask for confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Delete snapshots beyond settings.keep_backups
    Prune,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// *.hosts files, or directories holding them
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// Put everything in one group instead of a group per file
    #[arg(long, short)]
    pub group: Option<String>,
}

#[derive(Args, Debug)]
pub struct MigrateArgs {
    /// Directory holding the *.hosts files of the legacy hosts-sync
    #[arg(long, default_value = ".")]
    pub from: PathBuf,
    /// Do not ask for confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Overwrite an existing config
    #[arg(long)]
    pub force: bool,
    /// Import the current managed block and the *.hosts files in this directory
    #[arg(long)]
    pub from: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ConfigPathArgs {
    /// Print the zone files as well
    #[arg(long, short = 'a')]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// Open the file this group lives in
    pub group: Option<String>,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate the script for
    pub shell: Shell,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    PowerShell,
}

/// Фрагменты документации, которые генерируются из самого кода: `make gen`
/// кладёт их в docs/src/generated, а `make gen-check` роняет CI при расхождении.
#[derive(Subcommand, Debug)]
pub enum DocsCmd {
    /// Full command reference as markdown
    Cli,
    /// Exit code table as markdown
    ExitCodes,
}
