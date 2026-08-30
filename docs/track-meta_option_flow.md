# `-t/--track-meta` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--track-meta`, how entries are parsed, where they are consumed, and the current implementation status in cyanrip-rs.

## Purpose

`--track-meta` provides per-track metadata overrides from CLI.

It is used to:

- override or supply per-track naming inputs (for example `title`, `artist`),
- enrich cue rendering fields (for example `isrc`, `songwriter`, `composer`, `arranger`, flags),
- populate per-track output metadata (for example FLAC comments).

## CLI Surface

Options:

- `-t`
- `--track-meta`

Alias:

- `--track_meta`

Value format:

- repeatable entries: `N=key=value:key=value`

Examples:

```bash
cyanrip-rs -t "1=title=Intro:artist=Band" -t "2=title=Outro" -o flac -d disc.cue
```

```bash
cyanrip-rs --track-meta "1=title=Lead:isrc=USAAA9912345:preemphasis=1" -J -s 0
```

## Parse and Settings Mapping

During CLI parse:

1. Each `-t/--track-meta` occurrence is appended to `CliArgs.track_meta`.
2. `CliArgs::to_config` stores all raw entries into `settings.track_metadata`.

Runtime then parses each entry as:

1. split once on first `=` to get track index `N` and metadata payload,
2. parse `N` as positive track number,
3. split payload by `:` into `key=value` pairs,
4. trim each key/value and keep only non-empty pairs.

## Runtime Flow

### Full-rip and synthetic flows

- runtime builds a per-track metadata map from `settings.track_metadata`,
- per-track naming/tagging paths consume this map,
- missing defaults are filled for key fields like `track` and `title` when absent.

### Cue rendering

- cue-only preview and runtime cue generation consume parsed track metadata,
- extended cue fields are recognized when present (for example `isrc`, `preemphasis`, pregap/postgap and flags, plus songwriter/composer/arranger).

### Output metadata

- per-track metadata is merged into output comment maps,
- for FLAC, merged fields are propagated into Vorbis comments through canonical key mapping.

## Precedence and Merge Notes

- Explicit `--track-meta` values for a track override fallback defaults.
- When no track entry exists, runtime synthesizes minimal defaults (`track`, `title`) to keep output generation deterministic.

## Interaction With Related Options

- `--album-meta` provides album-level fields, while `--track-meta` is per-track.
- `--tracks` limits which tracks are processed; only processed tracks consume their per-track metadata in rip/info flows.
- naming options (`--track-scheme`, `--cue-scheme`, and related folder/log flows) consume track metadata where applicable.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration, alias support, and raw-entry mapping into settings.
- Runtime parser for per-track metadata entry format.
- Runtime consumption in full-rip, synthetic, and cue flows.
- Propagation to naming, cue fields, and FLAC metadata paths.
- Unit/integration coverage for representative behavior.

Known limits:

- Malformed `--track-meta` entries are currently ignored by runtime parsing rather than failing fast with explicit CLI errors.

## Regression Coverage

Coverage includes:

- `src/cli.rs`:
  - `golden_c_style_metadata_invocation`
- `src/app.rs`:
  - `cue_only_preview_ingests_extended_track_fields`
  - full-rip/cue-only tests using `track_metadata` fixtures
- `tests/run_workflow_cli.rs`:
  - full-rip bridge tests with `-t` entries (for example track summaries and selected-track runs)
- `tests/app_cli_integration.rs`:
  - output writer/tagging integration using per-track metadata
