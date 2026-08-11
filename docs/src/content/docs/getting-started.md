---
title: Getting started
description: From an empty config to a rendered /etc/hosts in five commands.
sidebar:
  order: 1
---

By the end of this page you will have a config, one entry in it, and that entry live in
`/etc/hosts` — with everything that was already in the file still there.

## Prerequisites

- Linux or macOS. hostsctl is built on `/etc/hosts`, `libc` and the platform's DNS cache
  flush, so there is no Windows build.
- `sudo` for the commands that write to `/etc/hosts`. Everything else runs as your own user.

## 1. Install

```bash
brew install jtprogru/tap/hostsctl
```

Other options — crates.io, the install script, a release archive — are on
[Installation](/hostsctl/install/).

## 2. Create the config

```bash
hostsctl init
```

This writes `~/.config/hostsctl/config.yaml`. Nothing else happens yet: `init` does not
touch `/etc/hosts`.

```bash
hostsctl config-path        # where the config actually is
```

## 3. Add an entry

```bash
hostsctl add 127.0.0.1 k8s.orb.local --comment orbstack
```

The entry goes into the group `local`, which is created on first use. It is in the config
now, not yet in `/etc/hosts`.

```bash
hostsctl list
```

## 4. Look before you write

```bash
hostsctl diff
```

`diff` renders what `apply` would produce and shows a unified diff against the current
file. It writes nothing and needs no root, which makes it the command to run when you are
not sure what a change does.

```bash
hostsctl check
```

`check` is the linter: it reports what `/etc/hosts` would silently ignore — a wildcard, a
port in a hostname, an address that is not an address.

## 5. Apply

```bash
sudo hostsctl apply
```

hostsctl shows the diff, asks for confirmation (`-y` skips it), takes a snapshot of the
current file, writes atomically, and flushes the DNS cache.

```bash
hostsctl status
```

`status` reports the config, the state of the managed block, every group and the backups.
If the file has drifted from the config, it says so and by how many lines.

## Undo

Three levels, from softest to hardest:

```bash
hostsctl disable k8s.orb.local && sudo hostsctl apply   # keep it, stop using it
sudo hostsctl off                                       # remove the whole block
sudo hostsctl backup restore                            # roll the file back
```

`off` leaves the config alone — it only removes the managed block from `/etc/hosts`, so a
later `apply` brings everything back.

## Where to go next

- [Configuration](/hostsctl/guides/configuration/) — what the YAML actually holds.
- [Groups](/hostsctl/guides/groups/) — switching sets of entries on and off.
- [Zone files](/hostsctl/guides/zones/) — splitting the config across files.
- [Blocklists](/hostsctl/guides/blocklists/) — attaching a remote hosts list.
