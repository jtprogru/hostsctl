# Changelog

All notable changes are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/). Before `1.0.0`, a breaking change bumps the MINOR version.

## [Unreleased]

### Added

- The generated command reference opens with an index of every top-level command.
- `hostsctl man` is no longer hidden from `--help`. It was documented on the site while being invisible in the CLI.

### Changed

- `hostsctl check` exits with `3` when the config has errors, the same code `apply` already used. It used to exit with the generic `1`, which contradicted the documented contract that `3` means "a human has to fix the config".

### Fixed

- The generated command reference no longer contains clap's automatic `help` subcommand tree — roughly half the page was entries like `hostsctl group help rm`, which document nothing.
- A config error was printed twice by `hostsctl check`: once by the planner, once by the command itself.
- `hostsctl backup prune` without write access to the backup directory reported that it had deleted nothing instead of asking for `sudo` and exiting with `4`.
- `hostsctl init --from` promised to import the managed block as well as the `*.hosts` files of a directory; it only ever imported the files. The help text now says so.
- Counts in messages are no longer pluralised as `1 errors`.

## [0.1.0] — 2026-08-11

First release. `hostsctl` keeps `/etc/hosts` entries in a YAML config and renders them into a block between markers; everything outside those markers is carried over byte for byte.

### Added

- A YAML config at `~/.config/hostsctl/config.yaml` (overridable with `--config` or `$HOSTSCTL_CONFIG`) holding groups of entries. A group is enabled or disabled as a whole, and an entry maps N addresses to M hostnames, so one name on several addresses stays several lines in `/etc/hosts`.
- Zone files: `include` attaches files next to the config, in either plain hosts syntax (`.hosts`, one group per file, comments preserved) or YAML (`.yaml`). Edits made through the CLI go back to the file the group came from, in that file's own format, and a file that did not change is not rewritten.
- Remote blocklists: `source add` attaches a hosts list as a group, with `rewrite_ip` and an allowlist. Lists are cached under `~/.cache/hostsctl/sources/` and refreshed with `source update`, which honours `ETag`. `apply` reads only the cache and never touches the network, so its result does not depend on connectivity.
- Backups: a snapshot of the target is taken before every write, including before a restore. `backup list`, `backup restore [ID]` and `backup prune` manage them, and `settings.keep_backups` bounds how many are kept.
- `check`, a linter for what `/etc/hosts` would silently ignore: wildcards, ports and paths in a hostname, invalid addresses, an address repeated inside one entry, and a name already defined outside the managed block.
- Safety guarantees: writes are atomic (temporary file, `fsync`, `rename`) and preserve the target's mode and owner; the result is refused unless it still contains `127.0.0.1 localhost`; a legacy `hosts-sync` block is visible to the tool but is never removed without `--drop-legacy` or `migrate`.
- Under `sudo`, the config and cache are still read from the invoking user's home directory (resolved through `SUDO_USER` in passwd), and files created as root are handed back to that user.
- `migrate` and `import` to move an existing `hosts-sync` setup or loose `*.hosts` files into the config.
- Shell completions for bash, zsh, fish, elvish and PowerShell, plus a man page.
- A documented exit-code contract: `0` ok, `1` generic failure, `2` usage, `3` config error, `4` permission, `5` I/O, `6` network.
- A documentation site at <https://jtprogru.github.io/hostsctl/>, English with a Russian locale. The command reference and the exit-code table are generated from the code, and CI fails when the committed copies drift.
- Release archives for Linux (`x86_64`/`aarch64`, glibc and musl) and macOS (Apple silicon and Intel), each with a keyless cosign signature, a SLSA build-provenance attestation and an entry in `checksums.txt`. Distributed through Homebrew, crates.io, `scripts/install.sh` and the archives themselves.

[Unreleased]: https://github.com/jtprogru/hostsctl/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jtprogru/hostsctl/releases/tag/v0.1.0
