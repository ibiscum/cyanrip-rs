# `-l/--tracks` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--tracks`, how the value is parsed and normalized, where it is applied at runtime, and the current implementation status in cyanrip-rs.

## Purpose

`--tracks` selects which track numbers are processed.

It is used to limit rip/info workflows to a subset of tracks instead of processing all tracks.

## CLI Surface

Options:
- `-l`
- `--tracks`

Value type:
- comma-separated list of positive track numbers

Examples:

```bash
cyanrip-rs -o flac -d disc.cue -l 1,3,5
```

```bash
cyanrip-rs --tracks 2 -I
```

## Parse and Validation Flow

During CLI parse:

1. Raw value is split by comma and trimmed.
2. Each element is parsed as an integer index.
3. Indices are normalized through shared index parsing (dedupe rejection + numeric sorting).
4. Normalized result is stored into:
   - `settings.rip_indices`
   - `settings.rip_indices_count`

Parsing rejects malformed values with explicit errors (for example non-numeric input).

## Runtime Flow

`--tracks` is consumed via selected track numbers derived from `settings.rip_indices`.

### Info mode (`-I`)

- Requested indices are validated against TOC size.
- Invalid indices return an error like `Invalid rip index X, list has N tracks!`.
- Output report includes `Tracks to rip: ...` and limits displayed track blocks to the selected subset.

### Full-rip workflows

- Selected indices are used to build requested track numbers.
- Image and physical boundary resolution uses these requested numbers.
- Only selected tracks are read, encoded, and written.
- If selected tracks are not available as audio tracks in physical TOC resolution, runtime errors out.

### Cue-only mode (`-J`)

- Current cue-only rendering path does not apply `--tracks` filtering; CUE output generation remains based on full runtime TOC/render flow.

## Interaction With Related Options

- `--track-meta` and `--pregap` entries can still target specific track numbers; when `--tracks` is used, only selected tracks are processed in rip/info flows.
- `--outputs` affects file formats written for the selected tracks.

## Implementation Status

Status: in progress

Implemented now:

- CLI declaration for `-l/--tracks`.
- Parse-to-settings mapping with normalized indices.
- Runtime enforcement in info mode and full-rip workflows (selected tracks only).
- Validation errors for out-of-range/invalid track indices in TOC-aware paths.
- Integration and unit tests covering parse mapping and selected-track full-rip behavior.

Still pending for full upstream parity:

- Cue-only mode track filtering parity is not yet wired to `--tracks`.

## Regression Coverage

Coverage includes:

- `src/cli.rs` parse tests:
  - `maps_outputs_tracks_and_pregap`
  - `golden_c_style_full_rip_invocation`
- `src/app.rs` tests:
  - `validate_requested_track_indices_against_toc_rejects_out_of_range_and_zero`
- `tests/run_workflow_cli.rs`:
  - `run_mode_full_rip_bridge_writes_selected_tracks`
