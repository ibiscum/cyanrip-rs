# `-Q/--eject` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--eject`, how it is parsed and mapped, and what behavior is currently implemented at runtime.

## Purpose

`--eject` requests tray ejection after a successful run.

It is a cleanup-time action, not a mode by itself.

## CLI Surface

Options:
- `-Q`
- `--eject`

Value type:
- boolean flag (disabled by default)

Example:

```bash
cyanrip-rs -Q -d /dev/cdrom
```

## Parse and Settings Mapping

During CLI parse:

1. `--eject` sets `CliArgs.eject = true`.
2. `CliArgs::to_config` maps this to `settings.eject_on_success_rip = true`.

Mode interaction side effects:

- `--info` disables eject behavior (`settings.eject_on_success_rip = false`).
- `--find-offset` also disables eject behavior.

## Runtime Flow

Eject behavior runs after successful workflow completion.

1. Runtime determines selected source kind (physical vs image).
2. Eject is only considered when both are true:
   - `settings.eject_on_success_rip == true`
   - selected source is physical drive.
3. On supported Linux libcdio builds, runtime opens the drive, checks drive capabilities, and only attempts eject when `CDIO_DRIVE_CAP_MISC_EJECT` is present.
4. Eject attempt happens as best-effort cleanup and does not fail the run if eject is unsupported or unsuccessful.

This mirrors upstream intent: ejection is success-path cleanup, guarded by capability and context.

## Output Impact

With `--eject` in eligible physical-drive runs:
- tray eject is attempted at end of successful run.

Without `--eject`, or in non-eligible contexts:
- no eject attempt is made.

Image-based runs (`.cue`, `.nrg`, `.toc`, `.bin`) do not attempt tray eject.

## Interaction With Related Options

- `--info`: parser explicitly disables eject side effect.
- `--find-offset`: parser explicitly disables eject side effect.
- `--cue-only`: no dedicated parser override; if source is physical and run succeeds, cleanup-time eject gate can still apply.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration and parse-to-settings mapping for `-Q/--eject`.
- Success-path runtime ejection hook in app workflow.
- Physical-source-only eject gating.
- Linux libcdio capability check (`CDIO_DRIVE_CAP_MISC_EJECT`) before ejection.
- Non-fatal best-effort cleanup behavior for unsupported/unavailable eject scenarios.

Known limits:

- Actual tray movement depends on runtime OS/backend support and hardware capability.
- Current concrete eject implementation is in Linux + `backend-libcdio-sys` path; other builds safely no-op.

## Regression Coverage

Coverage includes:

- `src/cli.rs` parse/mapping and side-effect tests for `settings.eject_on_success_rip`.
- `src/app.rs` unit test for eject gate semantics:
  - `eject_gate_requires_flag_and_physical_source`