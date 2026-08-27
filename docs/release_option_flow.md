# `-R/--release` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--release`, how it is parsed and consumed at runtime, and the current implementation status in cyanrip-rs.

## Purpose

`--release` selects a specific MusicBrainz release when multiple candidate releases are available for a disc.

It accepts either:

- a 1-based release index,
- or a MusicBrainz release ID string.

## CLI Surface

Options:

- `-R`
- `--release`

Value type:

- release selector (`index` or `id`)

Examples:

```bash
cyanrip-rs -I -R 2 -d /dev/cdrom
```

```bash
cyanrip-rs --release 76f0f93e-5c7f-4c07-a646-ec4f6de57f75 -J -s 0 -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--release` is captured in `CliArgs.release`.
2. `parse_release` interprets the value as:
   - `ReleaseSelection::Index(n)` for numeric `n > 0`,
   - `ReleaseSelection::Id(value)` for non-numeric input.
3. Parsed selector is stored in `settings.release`.

Validation behavior:

- numeric `0` or negative indices are rejected,
- non-numeric values are treated as release IDs.

## Runtime Flow

`settings.release` is consumed by metadata lookup flows that call MusicBrainz release selection.

### Info mode (`-I`)

Info-only workflow:

1. reads TOC and computes DiscID,
2. calls MusicBrainz `lookup_release` with `settings.release`,
3. enriches report output with selected release metadata.

### Cue-only mode (`-J`)

Cue-only workflow:

1. reads TOC and computes DiscID,
2. calls MusicBrainz `lookup_release` with `settings.release`,
3. uses the selected release to enrich report/CUE rendering context.

### Full-rip workflows

When metadata flow executes with MusicBrainz enabled, the same release-selection setting is passed into lookup and used to choose metadata used downstream.

## Multi-Release Behavior

When multiple releases are returned and no explicit `--release` selector is provided, runtime returns a disambiguation error message that lists candidates and requests `-R`.

## Interaction With Related Options

- `--no-musicbrainz` disables MusicBrainz lookup, so `--release` has no effect in that run.
- `--disc` (`discnumber`/`totaldiscs`) participates in release mapping context alongside `--release`.
- `--info` and `--cue-only` are the most visible user-facing paths for release-selected metadata output.

## Implementation Status

Status: complete (current metadata selection scope)

Implemented now:

- CLI declaration and parse-to-settings mapping for `-R/--release`.
- Shared release selector parsing (index or ID) with validation.
- Runtime consumption in info-only and cue-only MusicBrainz selection flows.
- Service-layer release-disambiguation behavior and explicit user guidance.
- Coverage for parser mapping and MusicBrainz release-selection scenarios.

Known limits:

- Effective behavior depends on MusicBrainz availability and build/runtime feature context.

## Regression Coverage

Coverage includes:

- `src/lib.rs`:
  - `release_parse_regression`
- `src/cli.rs`:
  - `parses_extended_fields`
  - `golden_c_style_full_rip_invocation`
  - `rejects_invalid_release_with_exact_message`
- `src/metadata/musicbrainz.rs`:
  - release-selection and multi-release disambiguation tests around `lookup_release`
- `tests/run_workflow_cli.rs`:
  - info-mode release disambiguation integration tests (`-R 1` and `-R 2`)
