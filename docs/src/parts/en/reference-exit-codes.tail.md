
## Using them

The distinction that matters in automation is `3` from `4`: a broken config needs a human,
a permission failure needs `sudo`. Both `check` and `apply` return `3` when the config has
errors, so the same branch covers a linting run and a real write.

```bash
#!/usr/bin/env bash
set -uo pipefail

hostsctl check
case $? in
  0) ;;
  3) echo "config is broken, not retrying" >&2; exit 1 ;;
  *) echo "unexpected failure" >&2; exit 1 ;;
esac

sudo hostsctl apply -y
```

`check` exits `0` when it found only warnings — a warning is something worth reading, not
something worth stopping a pipeline for.

## Output streams

Error messages go to stderr and normal output to stdout, so `hostsctl list | head` works
the way you expect — including the `SIGPIPE` behaviour of an ordinary unix tool, which
Rust otherwise suppresses.
