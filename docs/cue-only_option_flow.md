# `-J/--cue-only` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--cue-only`, how it is parsed and routed at runtime, and the current implementation status in cyanrip-rs.

## Purpose

`--cue-only` runs CUE-only mode.

In this mode, cyanrip-rs generates and prints CUE/report output without running the full audio ripping/encoding output pipeline.

## CLI Surface

Options:
- `-J`
- `--cue-only`

Alias:
- `--cue_only`

Value type:
- boolean flag

Example:

```bash
cyanrip-rs -J -s 0 -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--cue-only` sets `CliArgs.cue_only = true`.
2. `CliArgs::to_config` maps this to `settings.generate_cue_only = true`.
3. Cue-only side effects disable metadata network paths that are not needed for ripping output:
   - `settings.disable_accurip = true`
   - `settings.disable_coverart_db = true`

Validation interactions:

- `-J/--cue-only` cannot be combined with `-I/--info`.
- `-J/--cue-only` cannot be combined with `-f/--find-offset`.

## Runtime Dispatch Flow

Workflow routing checks mode flags in order:

1. find-offset mode
2. info-only mode
3. cue-only mode
4. synthetic/full-rip modes

When `settings.generate_cue_only` is true:

1. runtime first checks offset guard semantics,
2. then enters `run_cue_only_mode`.

## Offset Guard Behavior

Cue-only mode requires an explicitly set offset marker.

If `-J` is used without `-s`, runtime returns:

- `Offset is unset! To continue with an offset of 0, run with -s 0!`

This preserves the explicit-vs-default offset behavior contract.

## Runtime Behavior

With linux+cdda+backend-libcdio-sys features enabled:

- reads TOC from the selected device,
- computes DiscID values when available,
- optionally resolves MusicBrainz release metadata,
- renders info-style report content,
- renders CUE text and returns combined output.

Without that feature set:

- falls back to deterministic preview rendering path.

## Implementation Status

Status: in progress

Implemented now:

- CLI declaration, alias, parse-to-settings mapping, and mode-conflict validation.
- Cue-only runtime dispatch and feature-gated execution paths.
- Explicit offset-unset guard behavior matching expected contract.
- Integration/unit tests for cue-only success path, offset-unset behavior, and conflict handling.

Still pending for full upstream parity:

- Cue-only mode currently returns rendered report/CUE output and does not persist a `.cue` file through the runtime cue-file writer path.
- Cue-only track-filter parity with `--tracks` is not yet applied in the current cue-only rendering flow.

## Regression Coverage

Coverage includes:

- `src/cli.rs`:
  - `cue_only_applies_c_side_effects`
  - `cue_only_without_offset_keeps_offset_unset_marker`
  - mode-conflict validation tests for `-I/-J` and `-f/-J`
- `src/app.rs`:
  - `run_workflow_cue_only_mode_returns_preview`
  - `cue_only_preview_ingests_extended_track_fields`
- `tests/run_workflow_cli.rs`:
  - `cue_only_mode_returns_success_with_cue_preview`
  - `cue_only_mode_without_explicit_offset_matches_c_error`
  - `find_offset_mode_rejects_cue_only_combination`
