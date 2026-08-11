---
title: Backups
description: A snapshot before every write, and how to roll one back.
---

hostsctl takes a snapshot of the target file before every write — including before a
restore, so a rollback is itself reversible.

```bash
hostsctl backup list
```

```
/var/db/hostsctl/backups
  20260811-171531          412 B ← latest
  20260811-165502          389 B
  20260810-224417          389 B
```

Snapshot IDs are `YYYYMMDD-HHMMSS`, which sorts lexicographically and chronologically at
the same time. Two writes within the same second get `-1`, `-2` suffixes rather than
overwriting each other.

## Restoring

```bash
sudo hostsctl backup restore                    # to the latest snapshot
sudo hostsctl backup restore 20260810-171531    # to a specific one
```

Before writing, hostsctl shows the diff between the current file and the snapshot, asks
for confirmation (`-y` skips it), and takes a snapshot of the current state. A snapshot
that is empty, or that would leave the system without `127.0.0.1 localhost`, is refused.

## Pruning

```bash
hostsctl backup prune
```

Deletes everything beyond `settings.keep_backups` (default 20). `apply` prunes
automatically after a successful write, so this is only needed after lowering the setting.
Setting `keep_backups: 0` disables pruning entirely.

## Where they live

`settings.backup_dir` — `/var/db/hostsctl/backups` on macOS, `/var/lib/hostsctl/backups`
elsewhere. Both are root-owned, which is why commands that write need root even when the
target itself would be writable.

To keep snapshots somewhere else, point the setting at a directory you own:

```yaml
settings:
  backup_dir: /Users/you/.local/state/hostsctl/backups
```

## What a snapshot is not

A snapshot is a copy of the *target file*, not of the config. It restores `/etc/hosts` to
a previous state, but the config still says what it said — so the next `apply` will undo
the restore. If you want to undo a config change, edit the config; if you want to undo a
write, restore the backup and then fix the config.
