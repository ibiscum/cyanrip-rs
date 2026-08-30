# `-Y/--verify-log` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--verify-log`, how it is parsed and dispatched, and the current implementation status in cyanrip-rs.

## Purpose

`--verify-log` verifies a rip log FUN512 checksum.

It is a dedicated verification action and does not run normal ripping workflows.

## CLI Surface

Options:
- `-Y`
- `--verify-log`

Alias:
- `--verify_log`

Value type:
- string path to a log file

Examples:

```bash
cyanrip-rs -Y path/to/rip.log
```

```bash
cyanrip-rs --verify-log path/to/rip.log
```

## Parse and Action Mapping

During CLI parse:

1. `--verify-log` populates `CliArgs.verify_log`.
2. `CliArgs::to_config` sets `settings.verify_log`.
3. If `settings.verify_log` is present, parse action switches to `CliAction::VerifyLog` and returns early.

The early return mirrors upstream behavior: verify-log is treated as its own command path and short-circuits normal rip-mode parsing/dispatch.

## Runtime Dispatch Flow

In `main` dispatch:

1. `CliAction::VerifyLog` selects the log-verification branch.
2. Runtime calls `verify_log_path`.
3. Result is mapped to user-facing status text and process exit code semantics.

Result categories include:

- valid checksum,
- mismatch,
- trailing data after checksum,
- checksum missing,
- IO/read error.

## Verification Core Flow

`verify_log_path` reads file bytes and delegates to `verify_log_bytes`.

`verify_log_bytes`:

1. locates and parses the FUN512 line,
2. computes checksum over the intended content,
3. compares expected vs computed digest,
4. returns a typed verification outcome.

## Interaction With Other Options

`--verify-log` runs as a separate action path and does not execute normal rip/info/find-offset mode workflows.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration for `-Y/--verify-log` with alias `--verify_log`.
- Parse-time action switch to `CliAction::VerifyLog` with short-circuit behavior.
- Main dispatch branch that executes checksum verification.
- FUN512 verification logic for valid/mismatch/no-checksum/trailing-data/io outcomes.
- Integration and differential coverage for status/exit-code behavior.

Known limits:

- Behavior depends on readable local file input; IO errors are reported as verification failure status.

## Regression Coverage

Coverage includes:

- `src/cli.rs` verify-log action short-circuit tests.
- `src/fun512.rs` fixture-based verification outcome tests.
- `tests/verify_log_cli.rs` CLI status/message/exit behavior tests.
- `tests/differential_cli_vs_c.rs` verify-log parity scenarios against upstream C behavior.