---
title: Migrating from hosts-sync
description: Moving an older shell-script setup into hostsctl, either by importing it or by attaching the files as they are.
---

The predecessor was a bash script called `hosts-sync` plus a directory of `*.hosts` files
and a block in `/etc/hosts` between `# >>> hosts-sync begin >>>` markers. There are two
ways across, and they differ in whether the old files keep existing.

## Option 1 — import everything into the config

```bash
sudo hostsctl migrate --from /path/to/old/hosts-dir
```

This imports every `*.hosts` file into the config (one group per file, comments preserved,
commented-out lines becoming disabled entries), then removes the `hosts-sync` block from
`/etc/hosts` and renders its own. After that the old script and its files can be deleted.

To import without touching `/etc/hosts` yet:

```bash
hostsctl import /path/to/old/hosts-dir
hostsctl list --all
hostsctl diff
sudo hostsctl apply --drop-legacy
```

## Option 2 — keep the files, manage them in place

If you would rather keep the `*.hosts` files where they are, attach them as
[zone files](/hostsctl/guides/zones/) — nothing is copied into the config at all:

```bash
hostsctl zone add '/Users/you/Work/hosts/*.hosts'
sudo hostsctl apply --drop-legacy
```

The files stay the source of truth and stay hand-editable. The one caveat: the first edit
made through the CLI rewrites the file in hostsctl's formatting — columns aligned, header
moved to the top. Content survives, layout does not.

## What happens to the old block

Until you pass `--drop-legacy` (or run `migrate`), the `hosts-sync` block stays in
`/etc/hosts` and hostsctl only warns about it. That warning matters: if both blocks define
the same name, the one that appears first in the file wins, so leaving the old block in
place can make the new one look broken.

```bash
hostsctl status      # shows both blocks and their line ranges
```

## Coming from a hand-maintained /etc/hosts

If there was never a `hosts-sync`, but `/etc/hosts` has accumulated entries you want
managed:

```bash
hostsctl init
# copy the lines you want managed into the config, e.g.
hostsctl add 10.0.0.7 stand.local --group work
hostsctl diff
```

hostsctl deliberately does not slurp the whole existing file into the config: the lines it
does not manage keep working exactly as before, and there is no reason to take
responsibility for them. Move over what you actually want to manage and leave the rest
alone.
