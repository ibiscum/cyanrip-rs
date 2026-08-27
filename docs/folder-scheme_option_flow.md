# `-D/--folder-scheme` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--folder-scheme`, how it is parsed and validated, and how it affects runtime output path generation.

## Purpose

`--folder-scheme` controls the directory naming template used for emitted output files.

It is applied as part of naming-path generation for track outputs.

## CLI Surface

Options:
- `-D`
- `--folder-scheme`

Alias:
- `--folder_scheme`

Value type:
- string template

Default:
- `{album}{if #releasecomment# > #0# (|releasecomment|)} [{format}]`

Example:

```bash
cyanrip-rs -o wav,flac -D "{album}/{format}" -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--folder-scheme` is read into `CliArgs.folder_scheme`.
2. `CliArgs::to_config` maps this into `settings.folder_name_scheme`.

## Validation Flow

After option parsing, folder scheme validation runs with the selected output count.

Rule:
- if more than one output format is selected, folder scheme must include `{format}`.

If the rule is violated, parsing fails with a deterministic error.

## Runtime Flow

At runtime, the folder scheme is consumed by naming helpers during output path planning and file writes.

Flow:

1. Naming context is built from settings and track count.
2. Folder template is rendered with metadata and selected format suffix.
3. Relative output paths are built and resolved under output root.
4. Paths are used for collision checks and final file emission.

## Implementation Status

Status: complete

Implemented now:
- CLI declaration, alias support, parse mapping to settings.
- Validation rule enforcing `{format}` for multi-output runs.
- Runtime consumption by naming/rendering path builders.
- Deterministic regression coverage for validation and runtime dispatch behavior.

No known pending gap specific to `--folder-scheme` within current WAV/FLAC runtime scope.

## Regression Coverage

Coverage includes:
- parser-level rejection when `{format}` is missing for multi-output selection.
- validation unit tests for single-output vs multi-output behavior.
- app/workflow tests that exercise naming-driven output path generation.
