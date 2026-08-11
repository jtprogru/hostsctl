---
title: Zone files
description: Splitting the config across files, in YAML or in plain hosts syntax.
---

Keeping everything in one `config.yaml` is optional. `include` attaches zone files next to
the config, each holding its own groups:

```
~/.config/hostsctl/
├── config.yaml          settings, include, shared groups
└── zones/
    ├── 10-local.hosts   plain hosts syntax
    ├── 20-work.yaml     groups in YAML
    └── 30-ads.yaml      a remote blocklist
```

Patterns are expanded relative to the config directory. Their order sets the order of
groups in `/etc/hosts`, and there the first matching line wins — so ordering is not
cosmetic. Within one pattern, files are sorted by name, which is why the `10-`, `20-`
prefixes do what you expect.

## Two formats, chosen by extension

### `.hosts` — plain hosts syntax, one group per file

The group name comes from the file name (`10-local.hosts` → `local`), the header line
becomes the description, a comment above a line or at its end becomes that entry's
comment, and a commented-out line becomes a disabled entry:

```
# Local development

127.0.0.1   k8s.orb.local
10.30.13.37 sre-mcp.local  # stand
# 127.0.0.1 old.local
```

The header is exactly one comment line: the comments after it, up to the first entry,
belong to that entry.

A `.hosts` zone holds exactly one group and cannot hold a `source` — a remote list needs
`.yaml`.

### `.yaml` — the same groups as in the main config

Three shapes are accepted: `groups: [...]`, a bare list of groups, and a single group with
no `name` (taken from the file name).

```yaml
# zones/20-work.yaml
description: Work stands
entries:
  - ip: 10.0.0.7
    hostnames: [stand.local]
```

## Managing the include list

```bash
hostsctl zone list                          # patterns, files, what is in them
hostsctl zone add 'legacy/*.hosts'          # attach existing files
hostsctl zone rm  'legacy/*.hosts'          # detach; the files stay on disk
```

`zone add` loads the matched files right away, so a group-name collision surfaces at that
moment rather than on the next run.

## Putting a group in a file

```bash
hostsctl group add work --file zones/20-work.yaml
hostsctl add 10.0.0.7 stand.local --group work
hostsctl group move blocklist --file zones/30-ads.yaml
hostsctl group move blocklist --file main
hostsctl edit work                          # opens that group's file
```

If the path given to `--file` does not match any include pattern, it is appended to
`include` — otherwise the group would be lost on the next read.

## Edits go back where they came from

Every group remembers the file it was loaded from. A change made through the CLI is
written back to that file, in that file's own format, and a file whose content did not
change is not rewritten at all — so `mtime` stays meaningful and `git diff` stays quiet.

## Attaching files you already have

Existing `*.hosts` files anywhere on disk can be attached as they are, with no conversion:

```bash
hostsctl zone add '/Users/me/Work/hosts/*.hosts'
```

Be aware that the first edit through the CLI rewrites such a file in hostsctl's own
formatting: columns get aligned and the header moves to the top. The content survives; the
layout does not.

## Limits

- Group names are unique across all files. A duplicate is an error naming both files.
- A `.hosts` zone holds one group, and that group cannot have a `source`.
- A zone file that produces no groups is simply skipped.
