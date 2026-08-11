---
title: Configuration
description: What lives in config.yaml, how entries map addresses to names, and where the file is looked up.
---

The config is the source of truth. `/etc/hosts` is an output of it, not an input — with
the exception of everything outside the managed block, which hostsctl never reads back and
never touches.

## Where the file is

In order of precedence:

1. `--config /path/to/config.yaml`
2. `$HOSTSCTL_CONFIG`
3. `$XDG_CONFIG_HOME/hostsctl/config.yaml`
4. `~/.config/hostsctl/config.yaml`

Under `sudo`, `$XDG_CONFIG_HOME` is ignored — it points at root's environment, not yours —
and the home directory is resolved from `SUDO_USER` through passwd. See
[Permissions](/hostsctl/guides/permissions/).

```bash
hostsctl config-path          # the config
hostsctl config-path --all    # the config and every attached zone file
```

## The shape of it

```yaml
version: 1
settings:
  target: /etc/hosts
  backup_dir: /var/db/hostsctl/backups
  keep_backups: 20
  flush_dns: true
include:
  - zones/*.yaml
  - zones/*.hosts
groups:
  - name: local
    enabled: true
    description: Local development
    entries:
      - ip: 127.0.0.1
        hostnames: [k8s.orb.local]
        enabled: true
        comment: orbstack
```

Every field is documented in the [config reference](/hostsctl/reference/config/).

## Entries map N addresses to M names

An entry is a link between addresses and hostnames. `ip` takes a scalar or a list;
`hostnames` is always a list:

```yaml
- ip: 10.0.0.7                          # one address, several names
  hostnames: [api.local, web.local]
- ip: [192.178.194.100, 192.178.194.101, 192.178.194.102]
  hostnames: [analytics.google.com]     # one name, several addresses
```

In `/etc/hosts` this becomes one line per address, each carrying the full set of names.
hostsctl drops nothing: the only thing collapsed is an exact repeat of an
address-plus-name pair, and it warns when that happens. A name declared in two different
groups is not an error either, but you get a warning — usually it is an accident.

## Adding and removing

`hostsctl add` appends an address to an existing entry when the set of names matches:

```bash
hostsctl add 192.178.194.100 analytics.google.com
hostsctl add 192.178.194.101 analytics.google.com   # becomes ip: [.100, .101]
hostsctl rm 192.178.194.101                         # drops that address only
hostsctl rm analytics.google.com                    # drops the name and all its addresses
```

That asymmetry is deliberate: removing by address should not take a name's other addresses
with it.

## Disabled, not deleted

```bash
hostsctl disable k8s.orb.local
hostsctl enable  k8s.orb.local
hostsctl list --all               # --all shows disabled entries too
```

A disabled entry stays in the config and never reaches `/etc/hosts`. In a `.hosts` zone
file it is written as a commented-out line, which is how the same file stays readable by
hand.

## Settings

| Key | Default | What it does |
| --- | --- | --- |
| `target` | `/etc/hosts` | The file the managed block is rendered into. |
| `backup_dir` | `/var/db/hostsctl/backups` on macOS, `/var/lib/hostsctl/backups` elsewhere | Where snapshots go. |
| `keep_backups` | `20` | How many snapshots to keep; `0` means never prune. |
| `flush_dns` | `true` | Flush the DNS cache after a successful write. |

`--target` overrides `settings.target` for one run, which is what the test suite uses to
work against a copy instead of the real file.

## Editing by hand

```bash
hostsctl edit                # opens the config in $EDITOR, then runs check
hostsctl edit work           # opens the file the group 'work' lives in
```

`edit` re-reads the config after your editor exits and runs `check` on it, so a typo
surfaces immediately rather than at the next `apply`.
