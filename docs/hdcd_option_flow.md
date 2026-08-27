# `-H/--hdcd` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--hdcd`, when it is applied, and how audio data flows through the HDCD processing path.

## Purpose

`--hdcd` enables HDCD decoding in the track-processing stage.

When enabled, cyanrip-rs runs the decoded signal through an HDCD filter path and preserves expanded precision in a 24-bit output container for supported outputs.

## CLI Surface

Options:
- `-H`
- `--hdcd`

Flag type:
- boolean switch (disabled by default)

Examples:

```bash
cyanrip-rs -H -o flac -d disc.cue
```

```bash
cyanrip-rs --hdcd -o wav,flac -d /dev/cdrom
```

## Parse and Settings Mapping

`--hdcd` is parsed into `CliArgs.hdcd` and mapped into `settings.decode_hdcd` during CLI-to-settings conversion.

Reporting reflects this setting in run output as:
- `HDCD decoding:  enabled`
- `HDCD decoding:  disabled`

## Runtime Processing Flow

Per track, processing is selected by `TrackProcessingOptions::selected_processing_path()`:

1. If `decode_hdcd=true`, HDCD path is selected.
2. Else, deemphasis path may be selected depending on `-E/-W` and track flags.
3. Else, passthrough path is used.

Selection precedence matches upstream intent:
- HDCD has precedence over deemphasis.

## HDCD Backend Behavior

Current backend:
- external `ffmpeg` invocation with `-af hdcd`

Bridge format:
- input to ffmpeg: `s32le`
- output from ffmpeg: `s32le`

Internal sample flow:
1. Input track PCM arrives as 16-bit interleaved samples.
2. Samples are widened to signed 32-bit domain for ffmpeg compatibility.
3. ffmpeg applies the `hdcd` filter.
4. Output is converted to 24-bit PCM domain with rounding and clamping.
5. Output spec bit depth is set to 24.

Why 24-bit output:
- HDCD decode yields effective extra precision beyond 16-bit; writing 24-bit avoids truncating this expanded signal.

## Output Effects

When `--hdcd` is active and processing succeeds:
- WAV output is written as 24-bit PCM.
- FLAC output is encoded with 24-bit stream bit depth.

Without `--hdcd`:
- normal 16-bit input path remains in use unless another processing path changes the format.

## Error Handling

HDCD processing can fail with explicit processing errors:
- backend unavailable (for example, ffmpeg not found)
- backend failure (ffmpeg invocation/filter execution failure)
- invalid input spec (non-16-bit input or invalid channel/rate constraints)

These failures are surfaced through the track-processing error path rather than silently falling back.

## Interaction With Related Options

- `-E/--force-deemphasis`: ignored when HDCD path is selected, because HDCD takes precedence.
- `-W/--no-deemphasis`: only affects deemphasis auto-application and does not disable HDCD.

## Regression Coverage

Key coverage includes:
- CLI mapping tests for `--hdcd` to settings.
- processing-path tests for HDCD precedence and backend behavior.
- writer tests validating 24-bit WAV/FLAC support.
- run-workflow CLI parity test ensuring one `-H -o wav,flac` run emits both WAV and FLAC as 24-bit outputs.
