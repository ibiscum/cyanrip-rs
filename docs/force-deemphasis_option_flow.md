# `-E/--force-deemphasis` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--force-deemphasis`, how it interacts with related flags, and where the deemphasis path is selected at runtime.

## Purpose

`--force-deemphasis` forces CD deemphasis processing for track PCM.

Use this when you want deemphasis applied regardless of track pre-emphasis flags.

## CLI Surface

Options:
- `-E`
- `--force-deemphasis`

Alias:
- `--force_deemphasis`

Flag type:
- boolean switch (disabled by default)

Examples:

```bash
cyanrip-rs -E -o flac -d disc.cue
```

```bash
cyanrip-rs --force-deemphasis -o wav,flac -d /dev/cdrom
```

## Parse and Settings Mapping

`--force-deemphasis` is parsed into `CliArgs.force_deemphasis` and mapped into `settings.force_deemphasis` during CLI-to-settings conversion.

## Runtime Flow

Per track, processing path selection is performed via `TrackProcessingOptions`.

Deemphasis is selected when either of the following is true:
- `force_deemphasis == true`
- `deemphasis == true` and track metadata indicates pre-emphasis

Selection precedence:
- HDCD path has higher precedence than deemphasis.
- If `--hdcd` is enabled, HDCD is selected and deemphasis is not run.

## Interaction With Related Options

- `-W/--no-deemphasis`:
  - disables automatic deemphasis based on pre-emphasis flags,
  - does not disable `--force-deemphasis`.
- `-H/--hdcd`:
  - takes precedence over deemphasis,
  - `--force-deemphasis` does not override HDCD path selection.

## Processing Details

The deemphasis implementation is an in-process first-order IIR transform using CD 50/15us constants.

Input constraints for the current path:
- 16-bit PCM input
- valid channel count and sample rate
- channel-aligned interleaved samples

## Output Effects

When deemphasis is selected, output samples are transformed audio samples from the deemphasis filter path.

Bit depth remains on the standard non-HDCD path unless another processing path changes output format.

## Regression Coverage

Coverage includes:
- CLI mapping tests for `--force-deemphasis`.
- processing tests confirming forced deemphasis applies without track pre-emphasis.
- option-interaction tests confirming forced deemphasis still applies when auto-deemphasis is disabled.
- scenario-runner checks validating `-E` changes PCM output compared to plain mode.
