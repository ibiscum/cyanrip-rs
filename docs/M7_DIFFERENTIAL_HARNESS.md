# M7 Differential Harness (First Slice)

This document defines the first M7 differential-test slice against the upstream C binary, focused on CLI-visible behavior that is deterministic and hardware-independent.

## Scope (First Slice)

Included:
- CLI help structure categories.
- Output-codec help mode (`-o help`).
- Validation failures with frozen messages.
- Verify-log fixture outcomes (`-Y` with valid/mismatch/no-checksum fixtures).
- Verify-log edge outcomes (`-Y` with trailing-data and missing-file fixtures).
- Option-surface parity audit (short options).

Excluded for now:
- End-to-end ripping workflow parity.
- Verify-log runtime checksum parity.
- Hardware-dependent CD ripping workflow and encoder output parity.

## Prerequisites

- Rust workspace buildable.
- Upstream C binary built and accessible.
- Default C binary path used by tests:
  - `/home/ulf/data/cyanrip/build/src/cyanrip`
- Optional override:
  - `CYANRIP_C_BIN=/custom/path/to/cyanrip`

## Added Tests

1. Differential CLI first slice (ignored by default)
- File: [../tests/differential_cli_vs_c.rs](../tests/differential_cli_vs_c.rs)
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test differential_cli_vs_c -- --ignored`
- Covered cases:
  - `--help`
  - `-o help`
  - `-P 4` invalid paranoia level
  - `-I -J` mode conflict
  - `-Y tests/fixtures/log/valid.log`
  - `-Y tests/fixtures/log/mismatch.log`
  - `-Y tests/fixtures/log/no_checksum.log`
  - `-Y tests/fixtures/log/trailing.log`
  - `-Y tests/fixtures/log/does_not_exist.log`

2. Upstream option-surface audit
- File: [../src/cli.rs](../src/cli.rs)
- Test name:
  - `matches_upstream_short_option_surface`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" matches_upstream_short_option_surface`

3. Rust-only verify-log CLI regression guard (always-on)
- File: [../tests/verify_log_cli.rs](../tests/verify_log_cli.rs)
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test verify_log_cli`

4. Run-workflow synthetic full-rip guard (env-gated)
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `synthetic_full_rip_mode_writes_real_output_files_when_enabled`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli synthetic_full_rip_mode_writes_real_output_files_when_enabled`

5. Run-workflow synthetic full-rip image-reader source guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `synthetic_full_rip_mode_supports_image_reader_source`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli synthetic_full_rip_mode_supports_image_reader_source`

6. Default run full-rip bridge source-selection guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `run_mode_defaults_to_image_reader_full_rip_bridge`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli run_mode_defaults_to_image_reader_full_rip_bridge`

7. Default run full-rip selected-tracks guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `run_mode_full_rip_bridge_writes_selected_tracks`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli run_mode_full_rip_bridge_writes_selected_tracks`

8. Default run full-rip boundary-override guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `run_mode_full_rip_bridge_honors_track_boundary_metadata`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli run_mode_full_rip_bridge_honors_track_boundary_metadata`

9. Default run full-rip image-TOC override guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `run_mode_full_rip_bridge_honors_image_toc_env_overrides`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli run_mode_full_rip_bridge_honors_image_toc_env_overrides`

10. Default run full-rip cue-derived boundary guard
- File: [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs)
- Test name:
  - `run_mode_full_rip_bridge_honors_cue_toc_boundaries`
- Command:
  - `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli run_mode_full_rip_bridge_honors_cue_toc_boundaries`

## CLI Completeness Check (Current)

Options:
- Short-option surface from upstream C is fully represented in Rust parser for the M1/M0 scope.
- Audit coverage exists via `matches_upstream_short_option_surface`.

Commands/actions:
- Parse-level action routing exists for:
  - Run
  - ShowOutputsHelp
  - VerifyLog
- Runtime workflow parity is not complete yet:
  - `Run` action still uses migration placeholder output path in [../src/main.rs](../src/main.rs).
  - Full end-to-end command workflow remains an M7 task.

## Next M7 Steps

1. Expand differential cases using fixture-backed scenarios.
2. Add normalized output comparators for log/cue/name outputs.
3. Add differential verify-log scenario with controlled fixture logs.
4. Introduce end-to-end command workflow parity tests once runtime path is wired.
5. Update accepted differences in [PARITY_NOTES.md](PARITY_NOTES.md) for any intentional deviations.
