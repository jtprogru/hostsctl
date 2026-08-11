# AGENTS.md

Context for AI coding agents working in this repository. Humans want
[CONTRIBUTING.md](CONTRIBUTING.md).

## What this is

`hostsctl` — a Rust CLI that manages `/etc/hosts` from a YAML config. Binary crate, no
library target. Edition 2024; the MSRV is `rust-version` in `Cargo.toml`.

## The invariant that matters

Everything outside the `# >>> hostsctl begin >>>` / `# <<< hostsctl end <<<` markers is
copied into the new file unchanged. If a change can affect that, it needs a test that
proves it still holds. `tests/cli.rs` runs the real binary against a copy of a realistic
`/etc/hosts` in a temp dir via `--target`, which is how every behavioural test should work —
never against the system file, never needing root.

Alongside it: the write is atomic (temp file, `fsync`, `rename`) and preserves the target's
mode and owner; a result without `127.0.0.1 localhost` is refused; the legacy `hosts-sync`
block is never removed without `--drop-legacy`.

## Layout

| Path | What |
| --- | --- |
| `src/cli.rs` | The clap definition. Doc comments here are user-facing help text. |
| `src/commands/` | One module per command family; `docs.rs` generates the reference. |
| `src/config.rs` | The YAML model and load/save, including per-file writeback. |
| `src/zones.rs` | Zone files: `.yaml` and plain `.hosts`, parse and render. |
| `src/hostsfile.rs` | Reading the target, finding the block, composing, atomic write. |
| `src/render.rs` | Building the managed block from the config. |
| `src/remote.rs` | Downloading and caching remote blocklists. |
| `src/exit.rs` | Exit codes and the `Coded` error wrapper. |
| `src/paths.rs` | Config/cache paths and everything euid/sudo related. |
| `docs/` | Astro + Starlight site, English with a `ru/` locale. |
| `docs/src/parts/<locale>/` | Prose around the generated reference: frontmatter, intro, tail. |
| `docs/src/content/docs/**/reference/{cli,exit-codes}.md` | **Generated.** Assembled by `make gen` from the parts plus the binary's output; never edit by hand. |

## Commands

```bash
make ci          # what CI runs, in order
make test
make gen         # after any change to src/cli.rs or src/exit.rs
make docs-build
```

`make gen-check` fails the build when the assembled reference pages differ from a fresh
generation. If you change the CLI surface and do not run `make gen`, CI will fail.

## Conventions

- **Language.** User-facing strings — help, errors, printed output, docs — are in English.
  Internal comments in the Rust sources are in Russian. Do not convert a file from one to
  the other as a side effect of another change.
- **Comments explain why, not what.** The existing sources are sparse and load-bearing;
  match that density rather than annotating every line.
- **Errors.** `anyhow` throughout. When a failure has a meaningful exit code, wrap it with
  `exit::coded(...)` or `.or_code(...)` — see `src/exit.rs` for the table.
- **Commits** follow Conventional Commits. Do not add authorship trailers or
  generated-by markers.
- **Formatting** is `cargo fmt` with the committed `rustfmt.toml` (100 columns).

## Things not to do

- Do not add a Windows target. The tool is built on `/etc/hosts`, `libc` and the platform
  DNS flush.
- Do not make `apply` reach the network. It reads the blocklist cache only, on purpose.
- Do not add a dependency without checking `deny.toml` allows its licence.
- Do not edit an assembled reference page (`reference/cli.md`, `reference/exit-codes.md`),
  `Cargo.lock` by hand, or the version in `Cargo.toml` outside `make release-prep`. Edit
  `docs/src/parts/` and run `make gen` instead.
