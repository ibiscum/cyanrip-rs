# `-A/--no-accurip` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--no-accurip`, how it maps into runtime settings, and the current implementation status in cyanrip-rs.

## Purpose

`--no-accurip` disables AccurateRip database lookup and validation.

In cyanrip-rs, AccurateRip status/reporting is enabled by default for normal metadata orchestration when disc ID context is available. This option disables the AccurateRip network lookup path.

## CLI Surface

Options:
- `-A`
- `--no-accurip`

Alias:
- `--no_accurip`

Value type:
- boolean flag (disabled by default)

Examples:

```bash
cyanrip-rs -A -I -d /dev/cdrom
```

```bash
cyanrip-rs --no-accurip -o wav,flac -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--no-accurip` sets `CliArgs.no_accurip = true`.
2. `CliArgs::to_config` maps this to `settings.disable_accurip = true`.

Mode interaction side effects:

- `--cue-only` forces `settings.disable_accurip = true` even without `-A`.
- `--find-offset` forces `settings.disable_accurip = false` because find-offset mode requires AccurateRip data to resolve drive offset.

## Runtime Flow

Metadata orchestration gates AccurateRip lookup on `settings.disable_accurip`:

1. Runtime initializes AccurateRip status as `Disabled` when `settings.disable_accurip == true`, otherwise starts from `Error` pending lookup.
2. If `settings.disable_accurip == false`, the app computes lookup inputs and queries AccurateRip.
3. If `settings.disable_accurip == true` (via `-A` or mode side effect), AccurateRip lookup is skipped and no AccurateRip result payload is attached.

This behavior applies to the shared metadata orchestration used by run and info/reporting paths.

## Output Impact

With AccurateRip enabled (default):
- runtime attempts AccurateRip lookup and reports resolved status/results when available.

With `--no-accurip`:
- AccurateRip lookup is skipped.
- AccurateRip status is reported as disabled.
- no AccurateRip confidence/result payload is emitted.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration for `-A/--no-accurip` plus alias `--no_accurip`.
- Parse-to-settings mapping: `no_accurip -> settings.disable_accurip`.
- Runtime gating that skips AccurateRip lookup and marks status as disabled.
- Integration coverage asserting no AccurateRip calls occur when `-A` is set.
- Help text exposure in `-h/--help` output.

Known limits:

- Disabling AccurateRip removes confidence/match reporting by design; this is expected behavior, not a parity gap.
- In find-offset mode, AccurateRip is intentionally re-enabled because offset detection depends on it.

## Regression Coverage

Coverage includes:

- `src/cli.rs` tests that assert `-A` maps to `settings.disable_accurip = true` and mode interactions are preserved.
- `tests/app_cli_integration.rs`:
  - `cli_disable_flags_propagate_to_metadata_orchestration` (verifies AccurateRip status is disabled and call count is zero with `-A`).
- `src/app.rs` unit tests for metadata orchestration paths with `disable_accurip` enabled.