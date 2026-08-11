---
title: Config reference
description: Every key of config.yaml and of a zone file.
---

## Top level

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `version` | integer | — | Config format version. Currently `1`. A config declaring a higher version is refused rather than misread. |
| `settings` | map | see below | Where to write, where to back up, what to flush. |
| `include` | list of strings | `["zones/*.yaml", "zones/*.hosts"]` | Glob patterns for zone files, relative to the config directory. Omit the key for the defaults; an empty list attaches nothing. |
| `groups` | list of groups | `[]` | Groups held in the main config. Groups from zone files are not written here. |

## `settings`

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `target` | path | `/etc/hosts`, or `$HOSTSCTL_TARGET` when the key is absent | The file the managed block is rendered into. `--target` overrides it for one run. `$HOSTSCTL_TARGET` only fills the default, so it has no effect once the key is written out — and `hostsctl init` does write it out. |
| `backup_dir` | path | `/var/db/hostsctl/backups` (macOS), `/var/lib/hostsctl/backups` (Linux) | Where snapshots are written. |
| `keep_backups` | integer | `20` | How many snapshots to keep. `0` disables pruning. |
| `flush_dns` | boolean | `true` | Flush the DNS cache after a successful write. Only ever attempted when the target is the real `/etc/hosts`. |

## A group

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `name` | string | — | Unique across every file, compared case-insensitively. |
| `enabled` | boolean | `true` | A disabled group keeps its entries and is not rendered. |
| `description` | string | — | Rendered as the group's header comment inside the block. |
| `entries` | list of entries | `[]` | Local entries. Mutually exclusive with `source` in practice. |
| `source` | map | — | A remote blocklist; see below. Its entries come from the cache, not from `entries`. |

## An entry

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `ip` | string or list of strings | — | One or more addresses. A single address is written back as a scalar. |
| `hostnames` | list of strings | — | One or more names. Every address gets a line carrying all of them. |
| `enabled` | boolean | `true` | A disabled entry stays in the config and is not rendered. |
| `comment` | string | — | Appended to the rendered line after `#`. |

```yaml
- ip: 10.0.0.7
  hostnames: [api.local, web.local]
  enabled: true
  comment: staging box
```

## A source

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `url` | string | — | `http://` or `https://`. Plain HTTP is accepted but warned about: the list can be tampered with in transit. |
| `rewrite_ip` | string | — | Replaces the address of every entry in the list. Usually `0.0.0.0`. |
| `allow` | list of strings | `[]` | Names dropped from the downloaded list. |
| `last_fetch` | string | — | Written by hostsctl after a successful update. Informational. |

## Zone files

A `.yaml` zone accepts three shapes:

```yaml
# 1. wrapped
groups:
  - name: work
    entries: []
```

```yaml
# 2. a bare list
- name: work
  entries: []
```

```yaml
# 3. a single group, name taken from the file name
description: Work stands
entries:
  - ip: 10.0.0.7
    hostnames: [stand.local]
```

An unknown key in shape 3 is an error rather than a silently empty file — `group:` instead
of `groups:` is a typo worth catching.

A `.hosts` zone is plain hosts syntax and carries exactly one group. Its name comes from
the file name with any leading digits, dashes and underscores stripped (`10-local.hosts` →
`local`). A `# hostsctl: disabled` line in the header marks the group disabled, which is
the one thing the format cannot otherwise express.

## Environment variables

| Variable | Effect |
| --- | --- |
| `HOSTSCTL_CONFIG` | Path to the config; beats `$XDG_CONFIG_HOME` and `~/.config`. |
| `HOSTSCTL_CACHE` | Directory for cached blocklists. |
| `HOSTSCTL_TARGET` | Target file for a config that does not name one. A config written by `hostsctl init` does, so this is mostly useful for a hand-trimmed config. |
| `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` | Honoured, except under `sudo` where they point at root's environment. |
| `VISUAL`, `EDITOR` | Used by `hostsctl edit`, in that order, falling back to `vi`. |
| `NO_COLOR`, `TERM=dumb` | Disable coloured output. Colour is also off when stdout is not a tty. |
