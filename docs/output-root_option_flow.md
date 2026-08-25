# `-B/--outputroot` Option Flow

Last updated: 2026-08-25

This document defines how cyanrip-rs resolves the rip output base directory.

## Scope

- Applies to run workflow output path resolution in `src/app.rs`.
- Applies to both full-rip bridge mode and synthetic full-rip mode.

## Resolution Order

The output root is resolved with the following precedence:

1. CLI `-B` / `--outputroot` value, if provided and non-empty.
2. Environment variable `CYANRIP_RS_OUTPUT_ROOT`, if set and non-empty.
3. Current working directory (`std::env::current_dir()`), with `.` as fallback if cwd lookup fails.

## Behavior Notes

- `--outputroot` overrides `CYANRIP_RS_OUTPUT_ROOT` unconditionally when both are present.
- Empty or whitespace-only values are ignored for both CLI and environment sources.
- The selected output root is used for all emitted track files and shown in run output as `Output root: ...`.

## Examples

Use CLI override:

```bash
cyanrip-rs -o flac -d disc.cue -B /mnt/music-rips
```

Use environment fallback:

```bash
CYANRIP_RS_OUTPUT_ROOT=/mnt/music-rips cyanrip-rs -o flac -d disc.cue
```

Use current working directory fallback:

```bash
cd /mnt/music-rips
cyanrip-rs -o flac -d disc.cue
```

## Regression Coverage

- CLI parse mapping: `src/cli.rs` (`maps_outputroot_to_settings`)
- Help text contract: `src/cli.rs` (`help_contains_c_style_descriptions`)
- Runtime cwd fallback: `tests/run_workflow_cli.rs` (`run_mode_defaults_output_root_to_current_working_directory`)
- Runtime CLI-over-env precedence: `tests/run_workflow_cli.rs` (`run_mode_outputroot_cli_overrides_env_output_root`)
