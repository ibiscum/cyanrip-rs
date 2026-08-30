# `-K/--no-replaygain` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--no-replaygain`, how it maps into runtime settings, and which ReplayGain paths are currently implemented.

## Purpose

`--no-replaygain` disables ReplayGain tag generation.

In cyanrip-rs, ReplayGain metadata is enabled by default for the active FLAC output flow. This option disables those ReplayGain tags while keeping normal non-ReplayGain metadata tags.

## CLI Surface

Options:
- `-K`
- `--no-replaygain`

Alias:
- `--no_replaygain`

Flag type:
- boolean switch (disabled by default)

Examples:

```bash
cyanrip-rs -K -o flac -d disc.cue
```

```bash
cyanrip-rs --no-replaygain -o wav,flac -d /dev/cdrom
```

## Parse and Settings Mapping

`--no-replaygain` is parsed into `CliArgs.no_replaygain` and mapped as:
- `settings.enable_replaygain = !no_replaygain`

So when `-K/--no-replaygain` is present, `settings.enable_replaygain` becomes `false`.

## Runtime Flow

ReplayGain behavior is applied in the FLAC metadata-embedding stage:

1. Track PCM is processed and encoded.
2. Standard FLAC Vorbis metadata tags are prepared.
3. If `settings.enable_replaygain == true`, ReplayGain fields are added:
   - track-level gain/peak
   - album-level gain/peak
   - reference loudness
4. If `settings.enable_replaygain == false` (`-K`), ReplayGain fields are skipped.

## Output Impact

With ReplayGain enabled (default):
- FLAC tags include ReplayGain fields.

With `--no-replaygain`:
- ReplayGain fields are omitted.
- Existing non-ReplayGain tags (album, artist, track number, disc fields, etc.) remain.

WAV output is unaffected by ReplayGain tag behavior.

## Implementation Status

Status: in progress (implemented for active FLAC tagging path)

Implemented now:
- CLI parse and settings mapping for `-K/--no-replaygain`.
- Runtime gating of FLAC ReplayGain tag generation with `settings.enable_replaygain`.
- Integration tests asserting:
  - ReplayGain tags are present by default.
  - ReplayGain tags are absent with `-K`.

Not yet implemented (parity gap):
- Full upstream ReplayGain/EBU R128 parity across all codec/output paths.
- Full deferred-writeout ReplayGain architecture parity from upstream C pipeline.

## Regression Coverage

Coverage includes:
- CLI mapping tests for `--no-replaygain`.
- FLAC integration tests that validate ReplayGain tag presence by default and absence with `-K`.
