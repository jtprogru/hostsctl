---
title: Installation
description: Homebrew, crates.io, an install script, a release archive, or from source.
sidebar:
  order: 2
---

hostsctl is a single static binary with no runtime dependencies. Pick whichever of these
fits your machine.

## Homebrew

```bash
brew install jtprogru/tap/hostsctl
```

Covers macOS on Apple silicon and Intel, and Linux on `x86_64` and `arm64`. The man page
and shell completions are installed with the formula.

## crates.io

```bash
cargo install hostsctl
```

Builds from source, so it needs a Rust toolchain at or above the MSRV (see
`rust-version` in `Cargo.toml`). Completions and the man page are not installed this way —
generate them yourself if you want them:

```bash
hostsctl completions zsh > ~/.zsh/completions/_hostsctl
hostsctl man > /usr/local/share/man/man1/hostsctl.1
```

## Install script

```bash
curl -fsSL https://raw.githubusercontent.com/jtprogru/hostsctl/main/scripts/install.sh | sh
```

POSIX `sh`, so it also runs in an Alpine container whose only shell is `ash`. It detects
the OS and architecture, picks the musl build when there is no glibc loader, verifies the
archive against the release's `checksums.txt` **before** unpacking anything, and installs
into `/usr/local/bin`.

```bash
# pin a version and install somewhere else
curl -fsSL .../install.sh | sh -s -- --version v0.1.0 --bin-dir ~/.local/bin
```

## Release archive

Every release publishes a `.tar.gz` per target on the
[releases page](https://github.com/jtprogru/hostsctl/releases):

| Target | Use it on |
| --- | --- |
| `aarch64-apple-darwin` | macOS, Apple silicon |
| `x86_64-apple-darwin` | macOS, Intel |
| `x86_64-unknown-linux-gnu` | Linux, glibc, x86_64 |
| `aarch64-unknown-linux-gnu` | Linux, glibc, arm64 |
| `x86_64-unknown-linux-musl` | Alpine and other musl systems, x86_64 |
| `aarch64-unknown-linux-musl` | Alpine and other musl systems, arm64 |

Each archive holds the binary, `completions/`, `man/`, the README and the licence.

## Verifying a download

Every archive is listed in `checksums.txt`, signed with a keyless
[cosign](https://docs.sigstore.dev/) signature (`.bundle` next to it), and carries a SLSA
build-provenance attestation.

```bash
# checksum
sha256sum -c checksums.txt --ignore-missing

# signature
cosign verify-blob \
  --bundle hostsctl-aarch64-apple-darwin.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/jtprogru/hostsctl/.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  hostsctl-aarch64-apple-darwin.tar.gz

# provenance
gh attestation verify hostsctl-aarch64-apple-darwin.tar.gz --repo jtprogru/hostsctl
```

## From source

```bash
git clone https://github.com/jtprogru/hostsctl
cd hostsctl
make install            # builds release and installs into /usr/local/bin
```

`make install PREFIX=~/.local` installs elsewhere. `make uninstall` removes it.

## Shell completions

```bash
hostsctl completions bash
hostsctl completions zsh
hostsctl completions fish
hostsctl completions elvish
hostsctl completions powershell
```

Write the output wherever your shell looks — for zsh, a directory on `$fpath`, with the
file named `_hostsctl`.
