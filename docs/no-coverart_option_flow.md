# `--no-coverart` Option Flow

Last updated: 2026-08-28

This document explains the intended purpose of `--no-coverart`, how equivalent behavior is represented in cyanrip-rs today, and the current implementation status.

## Purpose

`--no-coverart` would mean "disable cover art processing."

That umbrella behavior can include two separate concerns:

1. disabling Cover Art DB lookup/download,
2. disabling cover-art embedding into output files.

## CLI Surface (Current)

There is currently no standalone `--no-coverart` flag in cyanrip-rs.

Implemented flags are:

- `-U` / `--no-coverart-db` (alias `--no_coverart_db`)
- `-G` / `--no-coverart-embed` (alias `--no_coverart_embed`)

Equivalent invocation for the umbrella intent is:

```bash
cyanrip-rs -U -G
```

## Parse and Settings Mapping

Current mapping is split across two booleans:

1. `--no-coverart-db` -> `settings.disable_coverart_db = true`
2. `--no-coverart-embed` -> `settings.disable_coverart_embedding = true`

There is no parser branch that recognizes `--no-coverart` directly.

## Runtime Flow

### Cover Art DB lookup control

- Metadata orchestration passes `settings.disable_coverart_db` to cover-art service lookup.
- When true, Cover Art DB fetch for missing front/back art is skipped.

### Cover embedding control

- Output writing paths check `settings.disable_coverart_embedding`.
- When true, cover art bytes are not embedded into output files in embedding-capable flows.

## Implementation Status

Status: not implemented as a standalone CLI option

Implemented now:

- Full split-flag behavior via `--no-coverart-db` and `--no-coverart-embed`.
- Parser/settings/runtime wiring for each split flag.

Not implemented:

- A single `--no-coverart` CLI alias/switch that toggles both behaviors together.

## Practical Guidance

If you need "no coverart" behavior today, pass both flags:

```bash
cyanrip-rs --no-coverart-db --no-coverart-embed
```

## Regression Coverage

Coverage exists for split-flag behavior (CLI mapping and cover-art service gating). There is currently no regression test for a literal `--no-coverart` option because that option is not implemented.