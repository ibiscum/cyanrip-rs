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

## M3 Metadata Services (in progress)

Completed subtopic:
- Metadata module scaffolding created under src/metadata.
- DiscID flow core rules ported from discid.c (MusicBrainz DiscID, CDDB, submission TOC URL).
- MusicBrainz lookup and release/track metadata mapping core rules ported from musicbrainz.c.
- Cover art lookup/download core behavior ported from coverart.c.
- AccurateRip lookup and binary parser core behavior ported from accurip.c.

Key files:
- [../src/metadata/mod.rs](../src/metadata/mod.rs)
- [../src/metadata/discid.rs](../src/metadata/discid.rs)
- [../src/metadata/musicbrainz.rs](../src/metadata/musicbrainz.rs)
- [../src/metadata/coverart.rs](../src/metadata/coverart.rs)
- [../src/metadata/accurip.rs](../src/metadata/accurip.rs)

Coverage notes:
- Added deterministic vector tests for DiscID/CDDB/TOC output and invalid-input behavior.
- Added wiremock-backed fixture tests for release lookup, selection behavior, not-found handling, and track metadata mapping.
- Added wiremock-backed cover art tests for DB lookup policy, URL hydration, and C-compatible front/back selection behavior.
- Added deterministic AccurateRip fixture-blob parser tests and wiremock lookup tests (404, html-not-found heuristic, valid binary payload).

### Metadata flow orchestration
- Added app-level metadata pipeline orchestration in [../src/app.rs](../src/app.rs).
- Flow order implemented and tested: DiscID -> MusicBrainz -> Cover Art -> AccurateRip.
- Added parity-focused tests for disable flags and fallback behavior when upstream metadata fails.
