---
title: Groups
description: Sets of entries that switch on and off as a whole.
---

A group is a set of entries with a name, and the unit that gets enabled or disabled. Every
entry belongs to exactly one group; `hostsctl add` without `--group` puts it in `local`,
creating that group on first use.

```bash
hostsctl group list
```

```
    type   name             entries  file                   description
on  local  local                  3  config.yaml            Local development
on  local  work                   2  20-work.yaml           Work stands
off remote blocklist         102843  30-ads.yaml            https://raw.githubusercontent.com/…
```

## Creating and filling

```bash
hostsctl group add work --description "Work stands"
hostsctl add 10.0.0.7 stand.local --group work
```

`--group` on `add` also creates the group when it does not exist, so the explicit
`group add` is only needed when you want a description or a specific file up front.

## Switching a whole group

```bash
hostsctl group disable blocklist --apply
hostsctl group enable  blocklist --apply
```

A disabled group keeps all of its entries and simply is not rendered. This is the reason
groups exist: turning off a hundred thousand blocked domains for one afternoon should not
mean deleting them.

`--apply` on any mutating command re-renders `/etc/hosts` immediately, which needs root.
Without it, the change stays in the config until the next `apply`.

## Description becomes a comment

The description is rendered as the group's header inside the managed block:

```
# --- work — Work stands ---
10.0.0.7       stand.local
```

For a remote group with no description, the source URL is used instead — so it is always
obvious where a block of lines came from.

## Deleting

```bash
hostsctl group rm work          # asks first; -y skips the question
```

This deletes the group together with its entries. For a remote group it also drops the
cached list. If you want the entries back later, disable rather than delete.

## Moving between files

Groups can live in the main config or in a [zone file](/hostsctl/guides/zones/):

```bash
hostsctl group move work --file zones/20-work.yaml
hostsctl group move work --file main            # back into config.yaml
```

Group names are unique across every file. If the same name shows up twice, hostsctl
refuses to run and names both files — silently picking one would mean silently losing the
other.

## Local and remote

A group has either `entries` (local) or a `source` (a remote blocklist), never both in a
meaningful way. Adding a manual entry to a remote group is rejected rather than quietly
ignored; `check` warns if a config somehow ends up with both.
