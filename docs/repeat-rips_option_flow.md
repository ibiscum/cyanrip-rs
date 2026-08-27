# `-Z/--repeat-rips` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--repeat-rips`, how it is parsed, and how it affects runtime retry policy.

## Purpose

`--repeat-rips` enables repeated rip-pass verification behavior intended for damaged or unstable media.

Conceptually:
- each rip pass computes a checksum,
- repeated passes are compared,
- ripping can continue until checksum-match goals are met or retry limits are reached.

## CLI Surface

Options:
- `-Z`
- `--repeat-rips`
- Alias: `--repeat_rips`

Value type:
- signed integer (`i32`)

Default:
- `0` (disabled)

Example:

```bash
cyanrip-rs -Z 2
```

## Parse and Mapping

During CLI parse:

1. `--repeat-rips` is parsed into `CliArgs.repeat_rips`.
2. `CliArgs::to_config` maps it to `settings.ripping_retries`.

Related option:
- `--retries` maps to `settings.max_retries` and provides the hard retry cap used with repeat-rips policy.

## Runtime Flow

When paranoia mode is active (`paranoia_level > 0`), runtime creates retry policy as follows:

- if `settings.ripping_retries > 0`:
  - `RetryPolicy::new(settings.ripping_retries as u32, settings.max_retries.max(1) as u32)`
- otherwise:
  - `RetryPolicy::disabled()`

This wiring is used in both:
- physical-drive paranoia path,
- image-reader paranoia path.

## RetryPolicy Semantics

`RetryPolicy` tracks:
- `required_matches` (from `--repeat-rips`),
- `max_retries` (from `--retries` cap),
- prior checksums and total attempts.

Decision behavior:
- if `required_matches == 0`: complete immediately (repeat-rips disabled),
- if enough prior checksum matches are observed: complete,
- if attempt limit is reached: complete,
- otherwise: continue retrying.

## Interaction Notes

- `--repeat-rips` only influences paranoia retry loops; if paranoia is disabled, repeat-rips logic is not executed.
- `--repeat-rips` does not replace `--retries`; it works together with `--retries` as goal-vs-cap controls.

## Related Options

- `--retries`: hard retry cap for frame/retry paths.
- `--paranoia`: enables/disables paranoia flow where repeat-rips policy applies.

## Regression Coverage

- CLI mapping assertion in `src/cli.rs` (`maps_basic_flags_to_settings`) checks `settings.ripping_retries`.
- Runtime wiring exists in `src/app.rs` where `settings.ripping_retries` gates `RetryPolicy::new(...)` for both image and physical paths.
