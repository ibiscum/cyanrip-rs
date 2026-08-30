# `-L/--log-scheme` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--log-scheme`, how log file paths are built, and the current implementation status in cyanrip-rs.

## Purpose

`--log-scheme` controls the base filename template for rip log files.

The final log path is assembled from:

1. folder template (`-D/--folder-scheme`),
2. log template (`-L/--log-scheme`),
3. fixed `.log` extension,
4. resolved output root (CLI/env/default precedence).

## CLI Surface

Options:
- `-L`
- `--log-scheme`

Alias:
- `--log_scheme`

Value type:
- string template

Default:
- `{album}{if #totaldiscs# > #1# CD|disc|}`

Example:

```bash
cyanrip-rs -o flac -L "rip-log" -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--log-scheme` is read into `CliArgs.log_scheme`.
2. `CliArgs::to_config` maps this into `settings.log_name_scheme`.

## Runtime Path Flow

Runtime log path generation uses naming helpers:

1. Render folder component from `settings.folder_name_scheme`.
2. Render log filename from `settings.log_name_scheme` against album metadata.
3. Append `.log` extension.
4. Resolve under output root using this precedence:
   - CLI `--output-root` / `-B`
   - `CYANRIP_RS_OUTPUT_ROOT`
   - current working directory fallback
5. Create parent directories as needed.
6. Write log content to the resolved path.

## Template Semantics

Template rendering follows shared naming behavior:

- literal text outside `{...}` is preserved,
- `{tag}` injects metadata values,
- conditional `{if ...}` expressions are supported,
- path components are sanitized and trimmed consistently with naming rules.

For log names, expansion context is album metadata (not per-track metadata).

## Interaction With --outputs

One log file is written per selected output format.

With multiple outputs, directory naming should include `{format}` to avoid collisions between log paths (same guidance as output-file path naming).

## Implementation Status

Status: in progress

Implemented now:

- CLI declaration and parse-to-settings mapping for `-L/--log-scheme`.
- Runtime log file emission in synthetic and full-rip bridge workflows.
- Log path resolution through folder/log templates plus output-root precedence.
- Integration tests for log-path resolution via:
  - CLI output root,
  - environment output root,
  - default current-working-directory fallback.

Still pending for full upstream parity:

- full upstream log lifecycle parity (including all C-side multi-stream logging nuances and report completeness details across every mode).

## Regression Coverage

Coverage includes:

- `tests/run_workflow_cli.rs`:
  - `run_mode_log_scheme_uses_cli_output_root`
  - `run_mode_log_scheme_uses_env_output_root_when_cli_unset`
  - `run_mode_log_scheme_defaults_to_working_directory_output_root`
