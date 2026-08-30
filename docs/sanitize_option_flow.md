# `-T/--sanitize` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--sanitize`, how sanitation mode is parsed and propagated, and the current implementation status in cyanrip-rs.

## Purpose

`--sanitize` controls filename/path character sanitation for rendered naming schemes.

It affects how invalid or risky characters are transformed when generating output directory/file names (track, log, cue, and cover paths).

## CLI Surface

Options:
- `-T`
- `--sanitize`

Value type:
- one of:
  - `simple`
  - `os_simple`
  - `unicode`
  - `os_unicode`

Examples:

```bash
cyanrip-rs -T simple -o flac -d disc.cue
```

```bash
cyanrip-rs --sanitize os_unicode -o wav,flac -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--sanitize` value is parsed through shared validation.
2. Parsed mode is mapped to `settings.sanitize_method`.
3. Invalid values fail parse with an explicit error.

Default behavior when unset:

- `settings.sanitize_method` defaults to `unicode`.

## Runtime Flow

Runtime naming path generation builds a `NamingContext` from `settings.sanitize_method`.

That context is used by scheme rendering and path builders for:

- per-track output file paths,
- log file paths,
- cue file paths,
- cover file paths.

Sanitation is applied when rendering template literals and metadata-derived tag values, with consistent trimming/path normalization afterward.

## Mode Semantics

### `simple`

- Uses ASCII-safe replacements for sanitized characters.

### `unicode`

- Uses Unicode replacements where available (for readability while remaining filesystem-safe).

### `os_simple` and `os_unicode`

- Preserve some characters considered locally available under OS-aware sanitation behavior.
- Still sanitize unsupported/path-risk characters according to the selected simple vs unicode mode.

## Interaction With Naming Options

`--sanitize` is shared infrastructure for naming templates and therefore interacts with:

- `--folder-scheme`
- `--track-scheme`
- `--log-scheme`
- `--cue-scheme`

Any option that renders names through shared scheme logic inherits the selected sanitation behavior.

## Implementation Status

Status: complete (current runtime naming scope)

Implemented now:

- CLI declaration and parse validation for `-T/--sanitize`.
- Parse-to-settings mapping into `settings.sanitize_method`.
- Runtime wiring via `NamingContext` in naming/path generation flows.
- Sanitization logic for scheme rendering and cover-title path generation.
- Unit/integration coverage for parse success/failure and sanitization behavior.

Known limits:

- Scope is naming/path sanitation only; this option does not alter audio data or metadata content itself.

## Regression Coverage

Coverage includes:

- `src/lib.rs`:
  - `sanitize_regression`
- `src/cli.rs`:
  - `parses_extended_fields`
  - `rejects_invalid_sanitize_with_exact_message`
- `src/naming.rs`:
  - `sanitize_simple_and_unicode`
  - `sanitize_keeps_dir_separator_for_literal_text`
