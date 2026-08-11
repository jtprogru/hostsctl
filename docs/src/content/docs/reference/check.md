---
title: Validation rules
description: Everything hostsctl check reports, what it means and whether it stops a write.
sidebar:
  order: 4
---

`hostsctl check` reads the config and reports what `/etc/hosts` would silently ignore. The
same rules run inside `apply`, `add`, `import` and `edit`, so nothing reaches the file that
`check` would have complained about.

Two levels, and the difference is not cosmetic:

| Level | What it means | Effect |
| --- | --- | --- |
| `error` | The line could not work, or is not a line at all. | `check` exits `3`; `apply` refuses to write anything. |
| `warn` | Legal, and usually a mistake. | Reported, nothing is blocked. `check` still exits `0`. |

An error inside a **disabled** entry or a disabled group is downgraded to a warning and
tagged `(entry is disabled)` — a line that never reaches `/etc/hosts` has no business
blocking an `apply`.

## Hostnames

| Rule | Level |
| --- | --- |
| A wildcard (`*.example.com`) — `/etc/hosts` has no such thing; use dnsmasq or `/etc/resolver/`. | error |
| A port or a path in the name (`api.local:8080`, `example.com/api`) — the name is the whole field. | error |
| An empty name, or an empty label (`a..b`). | error |
| Longer than 253 characters, or a label longer than 63. | error |
| A character outside `a–z`, `0–9`, `-` and `_` in a label. | error |
| A label that starts or ends with a hyphen. | warn |

A trailing dot (`example.com.`) is deliberately not a problem: a lookup without the dot
finds such a line, and blocklists ship names in that form.

## Addresses

| Rule | Level |
| --- | --- |
| `ip` is not an address — anything `std::net::IpAddr` refuses, IPv4 and IPv6 alike. | error |
| An entry with no address at all. | error |
| An entry with no hostnames. | error |
| The same address repeated inside one entry. | warn |

## Groups and sources

| Rule | Level |
| --- | --- |
| An empty group name. | error |
| `source.url` is not `http://` or `https://`. | error |
| `rewrite_ip` is not an address. | error |
| `source.url` is plain HTTP — the list can be tampered with in transit. | warn |
| The group has both a `source` and `entries` — the entries are ignored when rendering. | warn |

## What only `check` and `apply` see

Two warnings come from rendering rather than from the config alone, so they appear in
`check`, `diff` and `apply`, but not in `add`:

- the same address-plus-name pair twice — the duplicate is dropped, once, with a warning;
- a name declared in two different groups — both lines are kept, because several addresses
  for one name is a legitimate thing to want;
- a name that also appears **outside** the managed block on a different address. hostsctl
  does not touch that line, and in `/etc/hosts` the first match wins, so a line above the
  block quietly beats the one inside it. The warning names the file and the line number.

Repeated identical warnings are collapsed into one line with a `(×N)` counter — a
hundred-thousand-entry blocklist otherwise buries everything else.

## Reading the output

```bash
hostsctl check
```

```
error blocklist: ads.example: '0.0.0.0.' is not an IP address
warn  local: k8s.orb.local: label 'k8s-' starts or ends with a hyphen
```

The part before the colon is the location: a group name, or `group: hostnames` for an
entry. `hostsctl edit` runs `check` automatically after your editor exits, which is the
cheapest place to catch a typo.
