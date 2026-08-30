# `-I/--info` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--info`, how it is parsed and dispatched, and the current implementation status in cyanrip-rs.

## Purpose

`--info` runs info-only mode.

In this mode, cyanrip-rs reports disc/track metadata and runtime context without performing audio ripping or writing track output files.

## CLI Surface

Options:
- `-I`
- `--info`

Value type:
- boolean flag

Example:

```bash
cyanrip-rs -I -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--info` sets `CliArgs.info = true`.
2. `CliArgs::to_config` maps this to `settings.print_info_only = true`.
3. Info-only side effect disables eject behavior (`settings.eject_on_success_rip = false`).

Validation interactions:

- `-I` cannot be combined with `-J/--cue-only`.
- `-I` cannot be combined with `-f/--find-offset`.

## Runtime Dispatch Flow

Workflow routing checks mode flags in order:

1. find-offset mode
2. info-only mode
3. cue-only mode
4. synthetic/full-rip modes

When `settings.print_info_only` is true, runtime calls the info-only workflow and returns its rendered report output.

## Runtime Behavior

Info-only mode:

- reads TOC/drive context where backend support is available,
- validates selected tracks against TOC when track filtering is requested,
- computes/prints disc-identification fields when available,
- performs MusicBrainz release selection/report enrichment when enabled,
- prints track-level properties/metadata sections,
- does not run ripping/encoding output pipelines.

## Interaction With Related Options

- `--tracks` limits displayed/processed track entries in the info report.
- `--release` can select a specific MusicBrainz release in info mode.
- `--no-accurip` controls whether AccurateRip status is displayed as enabled or disabled in report output.
- `--eject` is ignored in info-only mode (explicitly disabled by parser side effect).

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration and parse-to-settings mapping for `-I/--info`.
- Mode validation conflicts with cue-only/find-offset combinations.
- Dedicated info-only runtime dispatch path.
- TOC-based reporting, optional MusicBrainz enrichment, and selected-track filtering.
- Integration/unit coverage for info-only mode behavior and side effects.

Known limits:

- Detailed info-mode content may vary by build features and runtime backend availability (linux/cdda/libcdio feature gates).

## Regression Coverage

Coverage includes:

- `src/cli.rs`:
  - `info_only_disables_eject_side_effect`
  - mode-conflict validation tests (`-I` with `-J`, `-I` with `-f`)
- `src/app.rs`:
  - info-report rendering tests (including selected-track filtering and release metadata blocks)
- `tests/run_workflow_cli.rs`:
  - `info_only_mode_returns_success_with_report`
  - `info_only_mode_keeps_accurip_enabled_unless_a_is_set`
