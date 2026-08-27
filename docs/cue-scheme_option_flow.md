# `-M/--cue-scheme` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--cue-scheme`, how CUE output paths are built, and the current implementation status in cyanrip-rs.

## Purpose

`--cue-scheme` controls the base filename template for generated CUE sheets.

The final CUE path is assembled from:

1. folder template (`-D/--folder-scheme`),
2. cue template (`-M/--cue-scheme`),
3. fixed `.cue` extension,
4. resolved output root (CLI/env/default precedence).

## CLI Surface

Options:
- `-M`
- `--cue-scheme`

Alias:
- `--cue_scheme`

Value type:
- string template

Default:
- `{album}{if #totaldiscs# > #1# CD|disc|}`

Example:

```bash
cyanrip-rs -o flac -M "disc-sheet" -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--cue-scheme` is read into `CliArgs.cue_scheme`.
2. `CliArgs::to_config` maps this into `settings.cue_name_scheme`.

## Runtime Path Flow

Runtime CUE path generation uses naming helpers:

1. Render folder component from `settings.folder_name_scheme`.
2. Render CUE filename from `settings.cue_name_scheme` against album metadata.
3. Append `.cue` extension.
4. Resolve under output root using this precedence:
   - CLI `--output-root` / `-B`
   - `CYANRIP_RS_OUTPUT_ROOT`
   - current working directory fallback
5. Create parent directories as needed.
6. Write CUE content to the resolved path.

## Mode Integration

For full-rip flows (synthetic and full-rip bridge):

- CUE files are emitted after track files are written.
- One CUE file is produced per selected output format.
- Track entries in the CUE use written output paths from the corresponding format.

For cue-only mode (`-J/--cue-only`):

- The CUE text is rendered and returned in command output.
- No runtime CUE file write step is currently performed in cue-only mode.

## Template Semantics

Template rendering follows shared naming behavior:

- literal text outside `{...}` is preserved,
- `{tag}` injects metadata values,
- conditional `{if ...}` expressions are supported,
- path components are sanitized and trimmed consistently with naming rules.

For CUE names, expansion context is album metadata (not per-track metadata).

## Interaction With --outputs

One CUE file is written per selected output format.

With multiple outputs, directory naming should include `{format}` to avoid collisions between CUE paths (same guidance as output-file path naming).

## Implementation Status

Status: in progress

Implemented now:

- CLI declaration and parse-to-settings mapping for `-M/--cue-scheme`.
- Runtime CUE file emission in synthetic and full-rip bridge workflows.
- CUE path resolution through folder/cue templates plus output-root precedence.
- Integration tests for CUE-path resolution via:
  - CLI output root,
  - environment output root,
  - default current-working-directory fallback.

Still pending for full upstream parity:

- Full upstream cue lifecycle parity across every mode is not yet complete.
- Cue-only mode currently renders CUE output but does not persist a `.cue` file via the runtime writer path.

## Regression Coverage

Coverage includes:

- `tests/run_workflow_cli.rs`:
  - `run_mode_cue_scheme_uses_cli_output_root`
  - `run_mode_cue_scheme_uses_env_output_root_when_cli_unset`
  - `run_mode_cue_scheme_defaults_to_working_directory_output_root`