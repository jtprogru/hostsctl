# Contributing to hostsctl

Thanks for taking the time. Bug reports, feature requests, documentation fixes and code are
all welcome.

## Code of conduct

Be respectful and constructive. Assume good intent, keep the discussion on the work.

## What hostsctl touches

hostsctl writes to `/etc/hosts`. That is a file the system will not boot happily without,
so the bar for changes to the write path is higher than usual:

- Everything outside the block markers must survive. Any change to `hostsctl.rs` or
  `render.rs` needs a test that proves it.
- The write stays atomic and keeps the target's mode and owner.
- The sanity check that refuses a result without `127.0.0.1 localhost` stays.
- No command silently gains the need for root, and no command tries to work around missing
  permissions.

## Getting started

You need a Rust toolchain at or above the MSRV declared as `rust-version` in `Cargo.toml`.
Install it via [rustup](https://rustup.rs/) with the `rustfmt` and `clippy` components.

```bash
git clone https://github.com/jtprogru/hostsctl
cd hostsctl
make            # lists every target
make build
make test
```

For the linters and release helpers:

```bash
make install-tools    # shellcheck, shfmt, actionlint, cargo-deny, cargo-audit
```

## Before opening a pull request

```bash
make ci
```

That runs, in order: `fmt-check`, `clippy -D warnings`, `shellcheck`, `actionlint`, the
test suite, `gen-check` and the MSRV build — the same set CI runs.

Two of them are easy to trip over:

- **`gen-check`** reassembles the generated reference pages from the binary and fails when
  the committed copy differs. If you touched `src/cli.rs` or `src/exit.rs`, run `make gen`
  and commit the result. The command reference and the exit-code table are generated
  precisely so they cannot silently fall behind the code.
- **`msrv`** builds against the minimum supported Rust version, which is usually older than
  your `stable`. Raising the MSRV is allowed but it is a `MINOR` bump and belongs in the
  changelog.

## Tests

Integration tests drive the real binary against a copy of `/etc/hosts` in a temporary
directory through `--target`. They never touch the system file and they must not need root.
If a change makes a test require root, that is a signal the change is wrong.

```bash
cargo test                       # everything
cargo test --test cli            # the end-to-end suite
cargo test --test zones          # zone files
```

## Documentation

The site is Astro + Starlight under `docs/`.

```bash
make docs-install
make docs-dev        # http://localhost:4321/hostsctl/
make docs-build
```

English is the primary language and lives in `docs/src/content/docs/`; the Russian locale
mirrors it under `docs/src/content/docs/ru/`. A missing Russian page falls back to its
English original rather than 404ing, so a partial translation is fine. If you add an
English page, adding the Russian one is appreciated but not required.

Two pages per locale are assembled instead of written: `reference/cli.md` and
`reference/exit-codes.md`. Each is the prose from `docs/src/parts/<locale>/`, wrapped
around the markdown the binary prints. Edit the parts, run `make gen`, and commit the
assembled file — never edit the assembled file itself.

## Conventions

- **Commits** follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(zones): support absolute include patterns`, `fix: keep the comment on merge`,
  `docs(readme): ...`. Subject in English, lower case, no trailing dot, ≤ 72 characters.
- **Branches**: `feature/<short-desc>`, `fix/<short-desc>`, `docs/<short-desc>`.
- **Atomic commits.** One logical change each; refactoring and behaviour changes separately.
- **Language.** User-facing strings — help text, errors, output — and public documentation
  are English. Internal comments in the Rust sources are Russian. Match the file you are
  editing instead of converting it.
- **Formatting** is whatever `cargo fmt` produces with the committed `rustfmt.toml`.

## Pull requests

Describe why the change is needed, what changed, and how to check it. Link the issue it
closes. If it changes behaviour, say so explicitly and add a `CHANGELOG.md` entry under
`## [Unreleased]`.

## Releasing

Maintainers only.

```bash
make release-prep VERSION=0.2.0     # stamps Cargo.toml, refreshes the lockfile
# write the CHANGELOG.md section for 0.2.0, commit both
make version-check TAG=v0.2.0       # exactly what the release workflow checks first
git tag -a v0.2.0 -m "v0.2.0"
git push origin main --follow-tags
```

The tag triggers `.github/workflows/release.yml`: cross-builds for six targets, checksums,
keyless cosign signatures, a SLSA build-provenance attestation, a GitHub release whose
notes come from the changelog section, the crates.io publish, and the Homebrew formula
update in `jtprogru/homebrew-tap`.

A tag containing a hyphen (`v0.2.0-rc1`) is published as a pre-release and does not update
the tap.

Versioning is [semver](https://semver.org/). Before `1.0.0` a breaking change bumps
`MINOR`.

## Security

Do not open a public issue for a security problem — see [SECURITY.md](SECURITY.md).
