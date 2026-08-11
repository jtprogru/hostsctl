---
title: Permissions and sudo
description: Which commands need root, and why sudo does not leave root-owned files in your home directory.
---

## What needs root

| Runs as you | Needs root |
| --- | --- |
| `list`, `search`, `diff`, `status`, `check`, `config-path` | `apply` |
| `add`, `rm`, `enable`, `disable` (without `--apply`) | `off` |
| `group *`, `zone *` (without `--apply`) | `backup restore` |
| `source add/update/list/rm` | any command with `--apply` |
| `init`, `import`, `edit`, `completions` | |

The split follows one rule: writing to `/etc/hosts` or to the backup directory needs root,
and nothing else does.

When permission is missing, hostsctl does not try to work around it. It prints the exact
command to re-run and exits with code `4`:

```
error: apply: no write access to /etc/hosts
  run: sudo hostsctl apply
```

## What sudo does not break

The config and the cache live in your home directory, not root's. Under `sudo`, `$HOME`
points at the invoking user only some of the time and `$XDG_CONFIG_HOME` points at root's
environment, so hostsctl does not trust either: the real home directory is resolved from
`SUDO_USER` through passwd.

Files and directories created while running as root are handed back to the invoking user,
walking up the chain from the created directory to the home directory. The result is that
`sudo hostsctl apply` never leaves root-owned files in `~/.config` or `~/.cache` — the
classic way a tool makes its own config unreadable for the person who owns it.

## Dry runs need nothing

```bash
hostsctl apply -n           # renders, diffs, writes nothing
hostsctl diff               # the same picture, without the apply ceremony
```

`--dry-run` is global, so it works on every command. On a mutating command it reports what
would have been written to the config as well.

## The target file's own permissions

The write is atomic: a temporary file next to the target, `fsync`, then `rename`. The mode
and owner of the existing target are read first and applied to the temporary file, so
`/etc/hosts` stays `root:wheel 0644` regardless of your umask, and the file it is replaced
by is a regular file with the same identity.

If the resulting content would not contain `127.0.0.1 localhost`, the write is refused
before anything is renamed. That guard catches both a bad config and a bug in hostsctl
itself.

## Using a different target

```bash
hostsctl --target /tmp/hosts apply -y
```

`--target` (or `settings.target`, or `$HOSTSCTL_TARGET`) points hostsctl at another file.
The DNS cache flush is skipped unless the target really is `/etc/hosts` — editing a copy in
`/tmp` is of no interest to the resolver. This is how the test suite exercises the whole
write path without touching the system file.
