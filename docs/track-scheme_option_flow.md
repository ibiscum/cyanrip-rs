# `-F/--track-scheme` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--track-scheme`, how it is parsed, and how it is applied during runtime output path generation.

## Purpose

`--track-scheme` controls the filename template for per-track output files.

It defines the track-level path component (file name stem) used by writer dispatch after folder naming is resolved.

## CLI Surface

Options:
- `-F`
- `--track-scheme`

Alias:
- `--track_scheme`

Value type:
- string template

Default:
- `{if #totaldiscs# > #1#|disc|.}{track} - {title}`

Example:

```bash
cyanrip-rs -o wav,flac -F "{track} - {title}" -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--track-scheme` is read into `CliArgs.track_scheme`.
2. `CliArgs::to_config` maps this into `settings.track_name_scheme`.

## Runtime Flow

At runtime, the track scheme participates in output path rendering for each selected format and track.

Flow:

1. Naming context is created from settings and track count.
2. Folder template and track template are rendered.
3. The rendered track template provides the per-track filename component.
4. Relative and absolute output paths are resolved and used for file emission.

The same scheme is used consistently across supported output formats in the current runtime scope.

## Interaction With Related Naming Options

- `--folder-scheme` determines the directory component.
- `--track-scheme` determines the track filename component.
- both are consumed by the shared naming/rendering path before writer dispatch.

## Implementation Status

Status: complete

Implemented now:
- CLI declaration and alias support.
- parse-to-settings mapping (`track_name_scheme`).
- runtime naming-path consumption in output dispatch.
- integration coverage that verifies resulting emitted filenames.

No known pending gap specific to `--track-scheme` within the active WAV/FLAC runtime scope.

## Regression Coverage

Coverage includes:
- CLI parse coverage in golden invocation tests.
- app integration tests asserting expected output filenames when `-F` is provided.
- naming fixture coverage with expected relative paths.
