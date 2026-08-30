# `-m/--cover-size` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--cover-size`, how it is parsed and mapped, and what behavior is currently implemented at runtime.

## Purpose

`--cover-size` controls which Cover Art DB image variant is requested for missing album art.

It applies when cyanrip-rs performs Cover Art DB lookup and needs to fetch front/back covers.

## CLI Surface

Options:
- `-m`
- `--cover-size`

Alias:
- `--cover_size`

Value type:
- integer (`i32`)

Accepted values:
- `250`
- `500`
- `1200`
- `-1` (original)

Default:
- `-1`

Examples:

```bash
cyanrip-rs -m 500 -d /dev/cdrom
```

```bash
cyanrip-rs --cover-size 1200 -o flac -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--cover-size` is parsed into `CliArgs.cover_size`.
2. `CliArgs::to_config` validates the value using `parse_cover_size`.
3. Valid values map to `settings.coverart_lookup_size`.

Invalid values fail with a validation error:

- `Invalid max coverart size {size} (must be 250, 500, 1200 or -1)`

## Runtime Flow

1. Metadata orchestration calls cover-art lookup with `settings.coverart_lookup_size`.
2. Cover-art service maps that enum to the Cover Art DB request path component.
3. Front/back download requests use the selected size variant (`front-250`, `front-500`, `front-1200`, or `front`).

If Cover Art DB lookup is disabled with `--no-coverart-db`, size selection is not used because lookup is skipped.

## Interaction With Related Options

- `--no-coverart-db`: disables the lookup path where `--cover-size` is consumed.
- `--cover`: user-provided cover inputs may reduce or eliminate DB lookups for missing front/back art.
- `--info`: info-only mode can still resolve metadata/URLs, but binary download behavior differs from full write paths.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration for `-m/--cover-size` plus alias `--cover_size`.
- Strict parse/validation for allowed values.
- Settings mapping to `settings.coverart_lookup_size`.
- Runtime consumption in Cover Art DB fetch path.
- CLI and parser regression coverage for valid and invalid values.

Known limits:

- `--cover-size` only affects Cover Art DB fetch variants; it does not resize user-supplied local files.

## Regression Coverage

Coverage includes:

- `src/lib.rs` tests for `parse_cover_size` value acceptance/rejection.
- `src/cli.rs` tests for mapping (`cover_size` to `settings.coverart_lookup_size`) and invalid-size error behavior.