# `-P/--paranoia` Option Flow

Last updated: 2026-08-26

This document explains the purpose of `--paranoia`, accepted values, and where paranoia behavior is applied during runtime.

## Purpose

`--paranoia` controls read-verification and retry behavior in track acquisition paths.

Higher paranoia levels enable stronger overlap/verify heuristics and retry handling to improve reliability on problematic media.

## CLI Surface

Options:
- `-P`
- `--paranoia`

Value format:
- integer level in `0..max`
- symbolic values: `none`, `max`

Current max level:
- `3`

Examples:

```bash
cyanrip-rs -P none
```

```bash
cyanrip-rs -P 2
```

```bash
cyanrip-rs --paranoia max
```

## Parse and Validation

`--paranoia` is parsed into `CliArgs.paranoia`, then mapped into `settings.paranoia_level` via `parse_paranoia(...)`.

Parsing behavior:
- `none` -> `0`
- `max` -> `MAX_PARANOIA_LEVEL`
- numeric-like strings are parsed with C-style `strtol` semantics
- accepted range is `0..=MAX_PARANOIA_LEVEL`
- out-of-range values return an explicit error

## Runtime Flow

### Trigger condition

Paranoia runs execute only when:
- `settings.paranoia_level > 0`

If level is `0`, paranoia loops are skipped.

### Physical-drive path

When active, the physical reader path runs an integrated paranoia frame loop:
- uses level-specific defaults and heuristics,
- applies per-frame retry cap and whole-pass repeat-rip policy,
- produces paranoia-corrected frames that are converted directly to PCM,
- validates completion state.

There is no separate "precheck then raw reread" step: the frames returned by the paranoia reader are the same frames consumed for checksum and encode decisions.

### Image-reader path

When active, the image reader executes paranoia heuristics/interruptible runs and returns finalized frames for PCM acquisition for each selected boundary.

### Retry interaction

Paranoia runs integrate with `--retries` and `--repeat-rips` at two nested levels:

1. **Frame-level retry** (`--retries`, default `10`): each frame is retried up to the cap before it is replaced with silence and the pass continues.
2. **Track-level retry** (`--repeat-rips` goal, `--retries` budget): whole-pass checksums are compared across repeated reads; the pass is repeated until the required match count is reached or the retry budget is exhausted.

Both levels are driven by `settings.max_retries`, while `--repeat-rips` sets the checksum-match threshold (`settings.ripping_retries`).

## Info Reporting

Info output reflects paranoia mode as:
- `none` for level `0`,
- `max` for max level,
- numeric level otherwise.

## Related Options

- `--retries`: max retry cap used in paranoia runs.
- `--repeat-rips`: checksum-match retry goal used with paranoia policy.
- `--device`: selects physical vs image source paths where paranoia can run.

## Regression Coverage

- CLI mapping/parsing coverage in `src/cli.rs` (including `max` and non-numeric C-style parsing behavior)
- parser validation coverage in `src/lib.rs` (`parse_paranoia` regression tests)
- runtime paranoia path coverage in `src/cdda/reader.rs`, `src/cdda/linux_drive.rs`, and workflow integration tests
