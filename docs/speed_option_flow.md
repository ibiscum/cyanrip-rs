# `-S/--speed` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--speed`, how it is parsed, and the current runtime behavior.

## Purpose

`--speed` is intended to control optical-drive read speed for ripping operations.

## CLI Surface

Options:
- `-S`
- `--speed`

Value type:
- signed integer (`i32`)

Default:
- `0`

Example:

```bash
cyanrip-rs -S 4
```

## Parse and Mapping

During CLI parse:

1. `--speed` is parsed into `CliArgs.speed`.
2. `CliArgs::to_config` maps it to `settings.speed`.

## Current Runtime Behavior

Current implementation status:

- The option is accepted by CLI and stored in settings.
- Parse-level tests validate that the value is mapped.
- Drive-speed control is not currently wired to physical backend operations.

In info-only reporting, the current output explicitly states:
- `Speed:          default (unchangeable)`

This reflects the present runtime behavior where `settings.speed` is not applied to hardware speed control.

## Practical Meaning Today

- Use of `--speed` currently affects parsed configuration state.
- It does not currently force the drive to a configured read speed during ripping.

## Related Options

- `--device`: selects image path or physical device path for run workflows.
- `--paranoia`: controls paranoia-mode read/verification behavior.

## Regression Coverage

- CLI mapping assertion in `src/cli.rs` (`maps_basic_flags_to_settings`) checks `settings.speed`.
- Additional mapping coverage in `src/cli.rs` (`golden_c_style_full_rip_invocation`) also checks speed parsing.
