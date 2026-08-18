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

### M3 Start
- Added metadata module scaffolding in [../src/metadata/](../src/metadata/).
- Ported DiscID core flow in [../src/metadata/discid.rs](../src/metadata/discid.rs).
- Added deterministic regression vectors for MusicBrainz DiscID, CDDB ID, and submission TOC URL generation.
- Implemented MusicBrainz lookup/mapping service in [../src/metadata/musicbrainz.rs](../src/metadata/musicbrainz.rs) with injectable HTTP client trait.
- Added wiremock-backed fixtures and tests for release selection, not-found handling, and track metadata mapping.
- Implemented cover art lookup/downloader service in [../src/metadata/coverart.rs](../src/metadata/coverart.rs) with injectable HTTP client trait.
- Added wiremock-backed cover art tests and deterministic coverart fixtures.
- Implemented AccurateRip lookup/parser service in [../src/metadata/accurip.rs](../src/metadata/accurip.rs) with injectable HTTP client trait.
- Added deterministic AccurateRip fixture blobs and wiremock-backed lookup tests.
- Evaluated libarcstk/arcstk/accurip crates on crates.io; no directly reusable Rust binding crate available in this environment, so native Rust implementation remains in-tree.
- Added app-level metadata flow orchestration in [../src/app.rs](../src/app.rs) with deterministic order and fallback behavior tests.

### M4 Start
- Added audio module scaffold in [../src/audio/mod.rs](../src/audio/mod.rs).
- Implemented WAV output writer in [../src/audio/wav.rs](../src/audio/wav.rs) using hound.
- Added WAV end-to-end integration test in [../tests/wav_pipeline.rs](../tests/wav_pipeline.rs).
- Implemented FLAC output writer in [../src/audio/flac.rs](../src/audio/flac.rs) using flacenc.
- Added FLAC end-to-end integration test in [../tests/flac_pipeline.rs](../tests/flac_pipeline.rs) using claxon for decode verification.

### Validation
- Test suite passing after each major change set (cargo test).

### Licensing and Compliance
- Re-licensed repository to LGPL-2.1-or-later and replaced top-level [../LICENSE](../LICENSE) text accordingly.
- Added SPDX license metadata in [../Cargo.toml](../Cargo.toml).
- Added upstream attribution/notice guidance in [../UPSTREAM_NOTICES.md](../UPSTREAM_NOTICES.md).
- Added dependency license inventory in [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
- Added bundled common license texts in [../licenses/Apache-2.0.txt](../licenses/Apache-2.0.txt), [../licenses/MIT.txt](../licenses/MIT.txt), and [../licenses/ISC.txt](../licenses/ISC.txt).
