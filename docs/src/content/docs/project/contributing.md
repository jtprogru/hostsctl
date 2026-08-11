---
title: Contributing
description: How to build, test and propose a change.
---

Bug reports, feature requests, documentation fixes and code are all welcome. The full
version of this page lives in
[CONTRIBUTING.md](https://github.com/jtprogru/hostsctl/blob/main/CONTRIBUTING.md).

## Setup

You need a Rust toolchain at or above the MSRV declared as `rust-version` in `Cargo.toml`.

```bash
git clone https://github.com/jtprogru/hostsctl
cd hostsctl
make            # lists every target
make build
make test
```

For the linters and the release helpers:

```bash
make install-tools    # shellcheck, shfmt, actionlint, cargo-deny, cargo-audit
```

## Before opening a pull request

```bash
make ci     # fmt-check, clippy, shellcheck, actionlint, tests, gen-check, msrv
```

That is the same set CI runs, in the same order. Two of those deserve a note:

- `gen-check` reassembles the generated reference pages from the binary and fails if the
  committed copy differs. If you touched the CLI definition or the exit codes, run
  `make gen` and commit the result.
- `msrv` builds against the minimum supported Rust version, which is usually older than
  your `stable`.

## Working on the docs site

```bash
make docs-install
make docs-dev        # http://localhost:4321/hostsctl/
make docs-build
```

English is the primary language and lives in `docs/src/content/docs/`; the Russian locale
mirrors it under `docs/src/content/docs/ru/`. A missing Russian page falls back to its
English original rather than 404ing, so a partial translation is fine — an English page
with no Russian counterpart is not a broken build.

Two pages per locale are assembled rather than written:

```
docs/src/parts/<locale>/reference-cli.head.md        prose, frontmatter
+ hostsctl docs cli                                  the command tree
+ docs/src/parts/<locale>/reference-cli.tail.md      prose
= docs/src/content/docs/[ru/]reference/cli.md        never edit this file
```

`reference/exit-codes.md` is built the same way. Edit the parts and run `make gen`; the
assembled file is committed so that the site builds without a Rust toolchain, and CI fails
if it drifts from a fresh generation.

## Conventions

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(zones): ...`, `fix: ...`, `docs: ...`.
- Branches: `feature/<short-desc>`, `fix/<short-desc>`, `docs/<short-desc>`.
- User-facing strings and public documentation are in English. Internal comments in the
  Rust sources are in Russian; match the file you are editing rather than converting it.
- One logical change per commit. Refactoring and behaviour changes go in separate commits.

## Tests

Integration tests drive the real binary against a copy of `/etc/hosts` in a temporary
directory through `--target`. They never touch the system file, and they should not need
root. If a change makes a test require root, that is a signal the change is wrong.

## Releasing

Maintainers only:

```bash
make release-prep VERSION=0.2.0     # stamps Cargo.toml and refreshes the lockfile
# write the CHANGELOG.md section for 0.2.0, commit
make version-check TAG=v0.2.0       # what CI will check
git tag -a v0.2.0 -m "v0.2.0" && git push origin v0.2.0
```

The tag triggers the release workflow: cross-builds for six targets, checksums, keyless
cosign signatures, a SLSA provenance attestation, the GitHub release with notes taken from
the changelog, the crates.io publish, and the Homebrew formula update. A tag containing a
hyphen (`v0.2.0-rc1`) is published as a pre-release and does not update the tap.
