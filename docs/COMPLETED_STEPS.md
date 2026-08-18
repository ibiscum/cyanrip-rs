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

## M4 Audio Output Pipeline (WAV slice)

Completed subtopic:
- Added audio module scaffolding in [../src/audio/mod.rs](../src/audio/mod.rs).
- Implemented WAV writer in [../src/audio/wav.rs](../src/audio/wav.rs) for 16-bit PCM input.
- Added integration coverage in [../tests/wav_pipeline.rs](../tests/wav_pipeline.rs).

Coverage notes:
- Added unit tests for WAV byte rendering, sample roundtrip, and input validation.
- Added end-to-end file write/read test using hound reader verification.

## M4 Audio Output Pipeline (FLAC slice)

Completed subtopic:
- Implemented FLAC writer in [../src/audio/flac.rs](../src/audio/flac.rs) using native Rust flacenc.
- Reused shared PCM input model in [../src/audio/mod.rs](../src/audio/mod.rs) for WAV/FLAC parity.
- Added end-to-end FLAC integration coverage in [../tests/flac_pipeline.rs](../tests/flac_pipeline.rs).

Coverage notes:
- Added unit tests for FLAC byte rendering, stream decode roundtrip, and input validation.
- Added end-to-end file write/read test using claxon decoder verification.

## M4 Audio Output Pipeline (Per-track dispatch slice)

Completed subtopic:
- Added app-level per-track output writer flow in [../src/app.rs](../src/app.rs).
- Implemented output dispatch by configured format for WAV and FLAC.
- Integrated naming-based relative-path generation into file emission flow.

Coverage notes:
- Added app-level tests for per-track WAV/FLAC file emission and unsupported-format rejection behavior.

## M4 Audio Output Pipeline (FLAC metadata embedding slice)

Completed subtopic:
- Added FLAC Vorbis-comment metadata embedding in app-level writer flow in [../src/app.rs](../src/app.rs).
- Added canonical key mapping for C-style metadata keys into FLAC/Vorbis naming where required (e.g. track/disc fields).

Coverage notes:
- Added assertions that emitted FLAC files include embedded album, album artist, artist, title, track number, disc number, and disc total fields.

## App-path integration tests (CLI to app entrypoints)

Completed subtopic:
- Added integration tests in [../tests/app_cli_integration.rs](../tests/app_cli_integration.rs) that start from CLI argument parsing and drive app-level entrypoints.
- Verified CLI disable flags affect metadata orchestration behavior as expected.
- Verified CLI output/disc settings drive output dispatch and embedded FLAC tags.

## M5 CDDA and paranoia control-path scaffold

Status: partial

Completed subtopic:
- Added deterministic paranoia-mode rip state machine model in [../src/cdda/paranoia.rs](../src/cdda/paranoia.rs).
- Added regression tests for parity-critical transitions: retry pending, retry-limit finalize, media-change abort, and fatal-error failure transitions.

Coverage notes:
- Added retry-policy tests mirroring the upstream repeat-rip behavior threshold and max-retry stopping behavior from /cyanrip/src/cyanrip_main.c.
