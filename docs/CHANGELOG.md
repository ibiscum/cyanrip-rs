# Migration Changelog

## 2026-08-18

### Planning and Governance
- Added migration roadmap and milestones in [../MIGRATION_PLAN.md](../MIGRATION_PLAN.md).
- Added parity matrix in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).
- Added CLI behavior freeze contract in [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md).
- Added parity acceptance criteria in [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md).

### CLI Migration
- Introduced Clap-based CLI parser in [../src/cli.rs](../src/cli.rs).
- Mapped parsed args into expanded Settings model in [../src/lib.rs](../src/lib.rs).
- Added C-like special-flow handling in [../src/main.rs](../src/main.rs) and [../src/cli.rs](../src/cli.rs).
- Added exact-message validation tests and help-structure tests.

### Fixtures
- Added deterministic fixture structure under [../tests/fixtures/](../tests/fixtures/).
- Added naming cases fixture in [../tests/fixtures/naming/cases.json](../tests/fixtures/naming/cases.json).
- Added CUE fixture samples in [../tests/fixtures/cue/](../tests/fixtures/cue/).
- Added log verification fixture samples in [../tests/fixtures/log/](../tests/fixtures/log/).
- Added FUN512 vector fixture in [../tests/fixtures/checksum/fun512_vectors.json](../tests/fixtures/checksum/fun512_vectors.json).

### M2 Start
- Ported naming and sanitation core rules into [../src/naming.rs](../src/naming.rs).
- Added naming-focused regression tests.
- Ported cue writer core behavior into [../src/cue.rs](../src/cue.rs).
- Added fixture-based CUE rendering regression tests (audio + data track scenarios).
- Ported FUN512 digest algorithm and log verification behavior into [../src/fun512.rs](../src/fun512.rs).
- Added fixture-based log verification tests and checksum processing core logic.
- Ported deterministic log report formatting sections into [../src/log_report.rs](../src/log_report.rs).
- Added fixture-based snapshot tests for start and finish report rendering.
- Expanded ChecksumCtx regression coverage for first/last-track windows and chunked processing.
- Aligned checksum window arithmetic with C-style u32 wrapping behavior.

### Validation
- Test suite passing after each major change set (cargo test).
