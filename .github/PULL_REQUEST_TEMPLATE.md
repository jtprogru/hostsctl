## Why

<!-- One or two sentences: what problem this solves. -->

## What changed

<!-- Bullets by component. -->

-

## How to check

<!-- Commands to run, or the test that covers it. -->

```bash
make ci
```

## Risks and breaking changes

<!-- Delete if none. Anything touching the write path, permissions or the config format
     belongs here. -->

## Checklist

- [ ] `make ci` passes
- [ ] `make gen` was run if `src/cli.rs` or `src/exit.rs` changed
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` if behaviour changed
- [ ] Docs updated if the change is user-visible
