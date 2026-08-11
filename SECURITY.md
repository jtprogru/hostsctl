# Security policy

## Supported versions

The latest release is supported. Fixes land in a new patch release rather than in patches
to older tags.

## Reporting a vulnerability

Please do not open a public issue.

Use [GitHub's private vulnerability reporting](https://github.com/jtprogru/hostsctl/security/advisories/new)
or email <jtprogru@gmail.com>. Include what you did, what happened, and what you expected —
a reproduction against a temporary target file (`hostsctl --target /tmp/hosts …`) is ideal.

Expect an acknowledgement within a few days. This is a personal project maintained in spare
time, so please be patient with the fix timeline; a coordinated disclosure date can be
agreed on request.

## Threat model

hostsctl writes to `/etc/hosts` and is therefore often run under `sudo`. Reports involving
any of the following are especially welcome:

- A way to make the tool write outside its target and backup directory, or to follow a
  symlink into somewhere unexpected during the atomic write.
- A path where files created while running as root are not handed back to the invoking
  user, or where a root-owned file lands in the user's home directory.
- Content in a downloaded blocklist, a config file or a zone file that causes hostsctl to
  emit lines outside its own block, or to remove lines that are not its own.
- A way to defeat the sanity check that refuses a result without `127.0.0.1 localhost`.

Out of scope: the contents of third-party blocklists, and the general fact that a user with
root can edit `/etc/hosts` by other means.

## Supply chain

Release archives carry a `sha256` in `checksums.txt`, a keyless
[cosign](https://docs.sigstore.dev/) signature (`.bundle`), and a SLSA build-provenance
attestation produced by the release workflow. Verification commands are documented in the
[installation guide](https://jtprogru.github.io/hostsctl/install/).

GitHub Actions used by this repository are pinned by commit SHA and updated through
Dependabot. Dependencies are checked weekly with `cargo audit` and `cargo deny`.
