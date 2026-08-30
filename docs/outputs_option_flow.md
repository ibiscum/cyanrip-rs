# `-o/--outputs` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--outputs`, how it is parsed and validated, and what output execution scope is currently implemented.

## Purpose

`--outputs` selects which output formats are produced during run workflow mode.

It accepts a comma-separated list of format names and controls per-track dispatch to writer backends.

## CLI Surface

Options:
- `-o`
- `--outputs`

Value format:
- comma-separated list of output format names

Examples:

```bash
cyanrip-rs -o flac -d disc.cue
```

```bash
cyanrip-rs --outputs wav,flac -d /dev/cdrom
```

Show supported output names/help mode:

```bash
cyanrip-rs -o help
```

## Parse and Validation Flow

`--outputs` value is split on commas, normalized, and passed through output-format validation.

Rules:
- empty list defaults to `flac`
- duplicate output names are rejected
- unknown output names are rejected
- `help` returns outputs-help action flow

Validation also interacts with folder naming rules:
- when multiple outputs are selected, folder scheme must contain `{format}`

## Runtime Flow

In run workflow mode:

1. Parsed formats are carried in `settings.outputs`.
2. Workflow preflight checks whether each requested format is implemented in the current runtime.
3. Supported formats are dispatched per track to concrete writer paths.
4. Unsupported formats return an explicit unsupported-output error.

## Implementation Status

Status: in progress (runtime execution is currently WAV/FLAC scope)

Implemented now:
- Full CLI parse/validation for output names, duplicate detection, and help mode.
- Per-track runtime dispatch for:
  - `wav`
  - `flac`
- Multi-output writing in one run for supported combinations (for example `wav,flac`).

Not yet implemented (runtime parity gap):
- Full encoder/runtime execution for the broader output list present in CLI parsing (for example mp3, tta, opus, aac, wavpack, vorbis, alac, pcm).

Current behavior for not-yet-implemented formats:
- CLI may parse the format name successfully,
- runtime returns an explicit unsupported-output error before writing.

## Regression Coverage

Coverage includes:
- parser-level output validation and outputs-help behavior.
- duplicate-output rejection checks.
- run-workflow and app-level dispatch tests for WAV/FLAC writing.
- explicit unsupported-format runtime rejection tests.
