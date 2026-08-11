---
title: Remote blocklists
description: Attaching a remote hosts list as a group, and why apply never touches the network.
---

A group can take its entries from a remote hosts list instead of from the config. The list
is downloaded once, cached, and used from the cache afterwards.

```bash
hostsctl source add \
  https://raw.githubusercontent.com/StevenBlack/hosts/master/data/StevenBlack/hosts \
  --group ads --rewrite-ip 0.0.0.0 --allow analytics.google.com --update
```

In the config this becomes:

```yaml
- name: ads
  enabled: true
  source:
    url: https://raw.githubusercontent.com/StevenBlack/hosts/master/data/StevenBlack/hosts
    rewrite_ip: 0.0.0.0
    allow: [analytics.google.com]
```

## What the options do

`--rewrite-ip` replaces the address of every entry in the list. Most blocklists ship
`0.0.0.0`, some ship `127.0.0.1`; `0.0.0.0` is generally the better choice because a
connection to it fails immediately instead of hitting whatever is listening on localhost.

`--allow` is the exception list: names that are dropped from the downloaded list. Repeat
the flag for more than one name.

`--file` puts the group in a zone file. It has to be a `.yaml` zone — plain hosts syntax
cannot express a `source`.

## Updating

```bash
hostsctl source update              # every source
hostsctl source update ads          # one of them
hostsctl source update ads --force  # ignore the cached ETag
hostsctl source update --apply      # download, then re-render /etc/hosts
```

Updates are conditional: hostsctl stores the `ETag` alongside the cached list and sends
`If-None-Match`, so an unchanged list costs one 304 and no download.

Lists are cached in `~/.cache/hostsctl/sources/` (or `$XDG_CACHE_HOME/hostsctl`, or
`$HOSTSCTL_CACHE`).

## apply never goes to the network

`apply` reads the cache and nothing else. That is a deliberate constraint: applying a
config should produce the same file whether or not the machine currently has internet, and
should never hang because a blocklist host is slow. If a remote group has no cache yet,
`apply` skips it and warns, naming the command that would fill it.

## What gets filtered out

While parsing a downloaded list, hostsctl drops comments, `localhost`,
`localhost.localdomain`, `broadcasthost`, `local`, and duplicate names, then applies
`allow` and `rewrite_ip`. Filtering happens both at download time and at render time, so
changing `allow` in the config takes effect without a re-download.

## Inspecting

```bash
hostsctl source list
hostsctl search doubleclick     # searches the cache too, not just the config
```

`source list` shows each source's URL, its rewrite address and allowlist, and the state of
its cache — how many entries and when it was fetched. It also warns when the cache was
downloaded from a different URL than the config now names.

## Removing

```bash
hostsctl source rm ads          # drops the group and its cache; -y skips the question
```

To keep the list but stop using it, disable the group instead:

```bash
hostsctl group disable ads --apply
```
