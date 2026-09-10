# `-p/--pregap` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--pregap`, accepted input format, and where pregap actions are applied at runtime.

## Purpose

`--pregap` controls per-track handling of detected pregap regions for CUE/runtime track-signature behavior.

It allows choosing how each target track should treat pregap boundaries:
- `default` (same as `merge`)
- `drop`
- `merge`
- `track`

Pregaps are detected from the source TOC:
- Physical drives: `DriveTrackTocEntry.pregap_lsn` in `src/cdda/linux_drive.rs`.
- Image files: the CUE parser in `src/cue.rs` extracts `pregap_lsn`, `dropped_pregap_start`, and `merged_pregap_end`.

## CLI Surface

Options:
- `-p`
- `--pregap`

Value format:
- `N=action`
- repeatable (one or more `-p` entries)

Where:
- `N` is a 1-based track index (`1..=197`)
- `action` starts with one of: `default`, `drop`, `merge`, `track`

Examples:

```bash
cyanrip-rs -p 1=drop -p 3=merge
```

```bash
cyanrip-rs --pregap 2=track
```

## Parse and Validation

Parser behavior:

1. Split each entry on `=` into track index and action token.
2. Validate track index is in range `1..=197`.
3. Parse action by prefix match:
   - `default*` -> `PregapAction::Default`
   - `drop*` -> `PregapAction::Drop`
   - `merge*` -> `PregapAction::Merge`
   - `track*` -> `PregapAction::Track`
4. Convert to zero-based index and store in `settings.pregap_action[idx]`.

Invalid inputs return explicit parse errors (for example invalid track index or invalid action token).

## Runtime Application

Pregap actions are consumed during runtime CUE-track construction in `src/app.rs` when a pregap LSN is available.

For each track:
- `Drop`:
  - mark `dropped_pregap_start`,
  - keep `start_lsn_sig` at the track start.
- `Merge` or `Default`:
  - mark `merged_pregap_end`,
  - move `start_lsn_sig` to the pregap start.
- `Track`:
  - keep `start_lsn_sig` at the track start,
  - keep pregap-associated track boundary semantics for track handling.

These values feed downstream CUE rendering/signature behavior through track fields such as:
- `pregap_lsn`
- `dropped_pregap_start`
- `merged_pregap_end`
- `previous_start_lsn_sig`

## Physical-Drive Rip Boundary Note

For physical-drive ripping, the actual PCM read window is built from `track.start_lsn` and `track.end_lsn` in `resolve_physical_track_boundaries` (`src/app.rs`). `pregap_lsn` and the selected `--pregap` action currently influence the generated CUE sheet and track signature, but they do **not** expand or shrink the raw audio read boundaries. In other words, the pregap action determines how the pregap is represented in the CUE output, not how much audio is ripped from the disc for a given track.

## Image-Backed Rip Boundary Note

For image-backed workflows, pregap state is carried through `CueTrack` and directly influences CUE output generation in `src/cue.rs`. The same action semantics (`drop`/`merge`/`track`) are reflected in the emitted `INDEX 00`, `PREGAP`, and track-start markers.

## Interaction Notes

- `--pregap` is per-track and can be repeated.
- Later entries for the same track override earlier ones during parse application.
- If no explicit action is provided for a track, `Default` behavior applies.

## Related Options

- `--cue-only`: commonly used with explicit pregap policy when previewing CUE output.
- `--tracks`: determines which tracks are selected in broader workflows.

## Regression Coverage

- CLI mapping/application test: `src/cli.rs` (`maps_outputs_tracks_and_pregap`)
- Parser validation tests: `src/lib.rs` pregap parsing regression tests
- Runtime pregap action consumption: `src/app.rs` cue-track runtime assembly
