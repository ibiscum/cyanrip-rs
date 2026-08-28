# `-C/--cover` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--cover`, how values are parsed, how cover sources flow through runtime, and the current implementation status in cyanrip-rs.

## Purpose

`--cover` lets you provide cover art sources from local paths or URLs.

It supports:

1. album-level cover art entries (for example `Front` and `Back`),
2. per-track cover art entries by track number.

## CLI Surface

Options:

- `-C`
- `--cover`

Value format:

- repeatable values in one of these forms:
  - `Title=path_or_url`
  - `N=path_or_url`
  - `path_or_url` (implicit assignment)

Examples:

```bash
cyanrip-rs -C "Front=/path/front.jpg" -C "Back=/path/back.jpg"
```

```bash
cyanrip-rs -C "2=/path/track2.png"
```

```bash
cyanrip-rs -C /path/front.jpg -C /path/back.jpg
```

## Parse and Validation Flow

During parse:

1. each `-C` value is collected in `settings.cover_specs`,
2. cover specs are validated by shared parser logic.

Validation rules (current implementation):

- track index form must be in range `1..=198`,
- duplicate per-track entries are rejected,
- duplicate album-level titles are rejected,
- implicit unkeyed values are assigned to missing `Front` then missing `Back`,
- a third unkeyed entry errors,
- empty source values error.

## Runtime Flow

### 1) Staging user cover inputs

Runtime converts parsed cover specs into initial cover-art entries:

- album-level entries are staged as runtime `CoverArtImage` inputs,
- per-track cover specs are currently parsed/validated but not yet consumed in writer paths,
- local file sources are loaded as bytes,
- URL sources are staged without immediate file read.

### 2) Metadata orchestration

Staged entries are passed into metadata orchestration, where cover-art lookup may add missing art (for example Cover Art DB when enabled).

### 3) Cover file writing

Runtime writes album-level `Front` and `Back` covers when data bytes are available.

Cover destination paths are built using:

1. folder naming scheme,
2. sanitized cover title (`Front`/`Back`),
3. detected extension,
4. resolved output root.

## Output Root Resolution for Cover Paths

Cover file paths use the same output root precedence as other runtime artifacts:

1. CLI `--output-root` / `-B`,
2. `CYANRIP_RS_OUTPUT_ROOT`,
3. current working directory fallback.

This applies to cover files written by full-rip and synthetic full-rip runtime flows.

## Interaction With Related Options

For umbrella disable semantics, see `no-coverart_option_flow.md` (`--no-coverart` status and equivalents).

- `--no-coverart-db`: disables DB lookup for missing covers.
- `--cover-size`: controls DB cover variant selection.
- `--no-coverart-embed`: embedding control (separate from external cover-file writing).
- `--info`: info mode affects whether binary data is downloaded in lookup paths.

## Implementation Status

Status: in progress

Implemented now:

- CLI declaration and settings mapping for `-C/--cover`.
- Shared parser/validator for album, track, and implicit Front/Back forms.
- Runtime staging of album-level user covers.
- Runtime writing of album-level `Front`/`Back` cover files under resolved output root.
- Regression coverage for parser errors and output-root precedence behavior.

Still pending for full upstream parity:

- per-track cover-spec runtime consumption and track-scoped cover output behavior,
- full embedding/parity nuances across all codec and mode combinations.

## Regression Coverage

Coverage includes:

- parser and validation tests in `src/lib.rs` (`parse_cover_specs` tests),
- CLI validation tests in `src/cli.rs` (`-C` invalid/duplicate/unkeyed error checks),
- runtime staging tests in `src/app.rs` (`initial_cover_arts_from_settings`),
- workflow integration tests in `tests/run_workflow_cli.rs` for cover output root precedence (CLI/env/cwd).
