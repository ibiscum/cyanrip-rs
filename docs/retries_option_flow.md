# `-r/--retries` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--retries`, how it is mapped into settings, and where it affects runtime behavior.

## Purpose

`--retries` controls the maximum number of retry attempts used by paranoia-oriented read paths.

It is intended to bound retry work during frame-read/paranoia runs, especially when media quality or read stability is poor.

## CLI Surface

Options:
- `-r`
- `--retries`

Value type:
- signed integer (`i32`)

Default:
- `10`

Example:

```bash
cyanrip-rs -r 20
```

## Parse and Mapping

During CLI parse:

1. `--retries` is parsed into `CliArgs.retries`.
2. `CliArgs::to_config` maps it to `settings.max_retries`.

Related setting:
- `--repeat-rips` maps to `settings.ripping_retries` and works alongside `max_retries` in retry-policy wiring.

## Runtime Consumption

### Info-only report (`-I`)

The report prints `Frame retries` from `settings.max_retries` so the active retry cap is visible.

### Full-rip image and physical flows

When paranoia mode is active (`paranoia_level > 0`), retry wiring uses `settings.max_retries` in two places:

- Retry policy construction for repeat-rip matching:
  - `RetryPolicy::new(settings.ripping_retries as u32, settings.max_retries.max(1) as u32)`
- Paranoia run limit passed into track runners:
  - `settings.max_retries.max(0) as u32`

This means:
- `max_retries` provides a hard cap for retry attempts.
- `ripping_retries` controls checksum-match goals, while `max_retries` limits total retry budget.

## Behavior Notes

- `max_retries` is clamped differently by call site (`max(1)` for policy construction, `max(0)` for runner cap) before conversion to `u32`.
- If paranoia mode is disabled (`paranoia_level == 0`), paranoia retry loops are not executed.

## Related Options

- `--repeat-rips`: checksum-repeat goal used by retry policy.
- `--paranoia`: enables/disables paranoia flows where retry caps are applied.

## Regression Coverage

- CLI mapping assertion in `src/cli.rs` (`maps_basic_flags_to_settings`) checks `settings.max_retries`.
