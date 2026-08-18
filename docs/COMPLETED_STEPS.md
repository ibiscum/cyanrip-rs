# Completed Steps

This document summarizes completed migration steps and major code changes.

## M0 Baseline and Parity Contract

Status: complete

Completed:
- Feature parity matrix created and maintained.
- CLI behavior freeze created for defaults, validation semantics, and custom error messages.
- Deterministic fixture set collected for naming, cue, log, and checksum domains.
- Acceptance criteria for parity and allowed differences documented.

Artifacts:
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
- [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md)
- [../tests/fixtures/README.md](../tests/fixtures/README.md)
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)

## M1 CLI and Config Parity

Status: complete

Completed:
- Clap-based parser implemented with grouped help sections and C-style descriptions.
- Defaults and validations mapped to Settings model.
- Special-flow semantics aligned:
  - verify-log short-circuit
  - outputs-help short-circuit
  - cue-only side effects
  - find-offset side effects
- Golden C-style CLI invocation tests and exact custom error-message tests added.

Key files:
- [../src/cli.rs](../src/cli.rs)
- [../src/lib.rs](../src/lib.rs)
- [../src/main.rs](../src/main.rs)

## M2 Deterministic Core Modules

Status: complete

Completed subtopic:
- Naming and sanitation core rules ported from naming.c.
- CUE writer core behavior ported from cue_writer.c.
- FUN512 digest, log verification outcomes, and checksum core logic ported from fun512.c/checksums.h.
- Deterministic log report formatting sections ported from cyanrip_log.c.

Implemented in naming module:
- append_missing_keys with escaped separator handling
- integer checks
- sanitation mappings across simple/os_simple/unicode/os_unicode
- conditional scheme rendering with if #...# support
- path component trimming behavior
- track path builder helper

Key file:
- [../src/naming.rs](../src/naming.rs)
- [../src/cue.rs](../src/cue.rs)
- [../src/fun512.rs](../src/fun512.rs)
- [../src/log_report.rs](../src/log_report.rs)

Coverage notes:
- Added snapshot-style fixtures for deterministic start and finish report rendering.
- Added deterministic checksum parity tests for first/last-track windows and chunked input processing.
