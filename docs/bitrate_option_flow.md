# `-b/--bitrate` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--bitrate`, how it is parsed and mapped, and what behavior is currently implemented at runtime.

## Purpose

`--bitrate` configures target bitrate (in kbps) for lossy output codecs.

It is intended to influence encoder settings for formats such as MP3, AAC, Opus, and related lossy paths.

## CLI Surface

Options:
- `-b`
- `--bitrate`

Value type:
- floating-point number (`f32`, kbps)

Default:
- `256.0`

Examples:

```bash
cyanrip-rs -b 320 -o mp3 -d disc.cue
```

```bash
cyanrip-rs --bitrate 192 -o opus -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--bitrate` is parsed into `CliArgs.bitrate`.
2. `CliArgs::to_config` maps it into `settings.bitrate_kbps`.

The parsed value is preserved in runtime settings regardless of selected output formats.

## Runtime Flow

Current run workflow checks requested output formats and dispatches only to implemented writer paths.

At present, active runtime writer support is limited to WAV and FLAC, which are lossless paths and do not consume `settings.bitrate_kbps`.

## Implementation Status

Status: in progress (parse/config implemented; runtime codec usage pending)

Implemented now:
- CLI declaration and parsing for `-b/--bitrate`.
- Mapping to `settings.bitrate_kbps`.
- Parse-level test coverage for value propagation.

Not yet implemented (parity gap):
- Runtime consumption of `settings.bitrate_kbps` by lossy encoder paths.
- End-to-end bitrate-effect validation for lossy outputs.

Practical behavior today:
- `--bitrate` is accepted and stored in settings.
- It has no effect for the currently active WAV/FLAC runtime output scope.

## Regression Coverage

Coverage includes:
- CLI mapping tests that assert `settings.bitrate_kbps` receives user-provided values.
