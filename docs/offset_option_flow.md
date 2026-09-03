# `-s/--offset` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--offset`, how its value is interpreted, and where it affects runtime behavior.

## Purpose

`--offset` sets the CD drive sample offset (in samples) used by rip-related flows.

The runtime also derives an over/under-read frame adjustment from this value, which is used in track boundary sizing.

## CLI Surface

Options:
- `-s`
- `--offset`

Value type:
- signed integer (`i32`), interpreted as samples.

Examples:

```bash
cyanrip-rs -s 6
```

```bash
cyanrip-rs --offset -589
```

## Parse and Mapping

During CLI parsing:

1. `offset` is read from CLI; if unset, value defaults to `0`.
2. `settings.offset` is set to that value.
3. `settings.offset_is_set` records whether user explicitly provided `-s/--offset`.
4. `settings.over_under_read_frames` is derived via `calc_over_under_read_frames(offset)`.

The derivation rounds sample offset to frame units using C-style behavior:
- `0 -> 0`
- `1..588 -> +1`
- `589 -> +2`
- `-1..-588 -> -1`
- `-589 -> -2`

## Runtime Effects

### Info-only report (`-I`)

The report includes:
- offset in samples,
- derived overread/underread frame count,
- overread/underread mode text.

### Full-rip boundary sizing

Track frame count is adjusted by the magnitude of `settings.over_under_read_frames`:
- `delta = abs(over_under_read_frames)`
- `frame_count += delta`

### Overread lead-in/lead-out behavior (`--overread`)

`--overread` controls whether the read window is allowed to extend past the disc boundaries (lead-in/lead-out).

- **Default**: disabled (`overread_leadinout = false`).
- When disabled, frames that would fall outside the disc TOC are clipped from the read request and replaced with silence padding. This keeps the final track at the correct length without requiring the drive to read past the disc edge.
- When enabled, the full shifted read window is requested from the drive and no silence padding is added for out-of-range frames.

This interacts with the paranoia frame loop in `src/cdda/reader.rs`: boundary trimming is computed by `plan_track_read` in `src/app.rs` before the pass begins, and the reader consumes the already-clipped frame list.

### Cue-only guard (`-J`)

Cue-only mode requires an explicitly set offset marker.

If `-J` is used without `-s`, runtime returns:
- `Offset is unset! To continue with an offset of 0, run with -s 0!`

This means `-s 0` is semantically different from omitting `-s` in cue-only mode.

### Find-offset mode (`-f`) side effect

When `-f` is enabled, parser side effects force:
- `settings.offset = 0`
- `settings.offset_is_set = false`
- `settings.over_under_read_frames = 0`

This keeps find-offset execution aligned with its dedicated offset-detection path.

## Related Options

- `--overread`: controls lead-in/lead-out strategy used with derived frame behavior.
- `--find-offset`: resets offset fields and runs offset detection flow.
- `--cue-only`: uses `offset_is_set` guard semantics.

## Regression Coverage

- CLI mapping and derived frame assertions: `src/cli.rs` (`maps_basic_flags_to_settings`)
- Cue-only offset-unset behavior: `src/cli.rs` (`cue_only_without_offset_keeps_offset_unset_marker`) and `src/app.rs` run workflow guard
- Find-offset side effects: `src/cli.rs` (`find_offset_applies_c_side_effects`)
- Frame derivation math: `src/lib.rs` (`over_under_read_frames_regression`)
