# Completed Steps

Last updated: 2026-08-28

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

## M3 Metadata Services

Status: complete

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

## M6 Linux physical-drive support and reliability layer

Status: complete

Completed subtopic:
- Added Linux physical-drive adapter module in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs) implementing the shared CDDA frame reader contract.
- Added feature-gated backend wiring for libcdio-sys and fallback unsupported backend behavior when no physical backend is enabled.

Coverage notes:
- Added hardware-free backend regression tests for media-changed mapping parity, seek/read progression, read-failure propagation, and backend cleanup.
- Validated libcdio-sys backend compilation/tests with safe feature set (`backend-libcdio-sys` + `paranoia`) after switching to libcdio-sys 2.x without UDF default features.
- Added real-drive smoke test harness in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs) (ignored by default; env-driven device path), including TOC-read regression coverage against `/dev/cdrom` as a first hardware validation slice.
- Added Linux dependency/feature verification helper script in [../scripts/check_linux_cdda_stack.sh](../scripts/check_linux_cdda_stack.sh).
- Added Linux real-drive paranoia runtime wiring in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs) so the shared state machine now runs against physical-frame reads, frame retries, and finalize/encoder-flush transitions.
- Added regression coverage for this wiring with backend-mock tests and a hardware-gated paranoia integration test in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs).
- Added paranoia-like overlap/verify/retry heuristic layer and callback counters in [../src/cdda/reader.rs](../src/cdda/reader.rs), modeled after upstream status/repeat behavior from /home/ulf/data/cyanrip/src/cyanrip_main.c and /home/ulf/data/cyanrip/src/cyanrip_log.c.
- Added regression tests for overlap drift forcing retry-limit finalize and callback-counter updates in [../src/cdda/reader.rs](../src/cdda/reader.rs) and [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).
- Added explicit interruption handling in paranoia runtime via interruptible runner entrypoints in [../src/cdda/reader.rs](../src/cdda/reader.rs), mapping interruptions to `QuitRequested` -> `Aborted` transitions.
- Added linux-backend interruptible wrappers and regression coverage for interruption abort behavior while retaining media-change abort handling in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).
- Added practical real-hardware reliability scenario runbook and acceptance-notes template in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).
- Added scripted M6 scenario runner in [../scripts/run_m6_hardware_validation.sh](../scripts/run_m6_hardware_validation.sh) and real-drive interruption validation in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs).
- Executed manual media-change scenario once and recorded console prompt + outcome evidence in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).

## M7 Full workflow integration and release parity (in progress)

Status: partial

Completed subtopic:
- Added CLI-focused differential harness test in [../tests/differential_cli_vs_c.rs](../tests/differential_cli_vs_c.rs) for help, output-help, and frozen validation-failure scenarios.
- Added upstream short-option parity audit test in [../src/cli.rs](../src/cli.rs).
- Added runbook and runner script in [M7_DIFFERENTIAL_HARNESS.md](M7_DIFFERENTIAL_HARNESS.md) and [../scripts/run_m7_cli_diff.sh](../scripts/run_m7_cli_diff.sh).
- Implemented runtime verify-log mode in [../src/main.rs](../src/main.rs) with FUN512 outcome mapping and C-aligned result text.
- Expanded differential harness to include verify-log fixture outcomes (`valid`, `mismatch`, `no_checksum`, `trailing`, and missing-file I/O error).
- Added Rust-only integration test [../tests/verify_log_cli.rs](../tests/verify_log_cli.rs) to keep verify-log exit-code/message behavior guarded in normal test runs.
- Implemented `-G/--no-coverart-embed` runtime behavior in [../src/app.rs](../src/app.rs): FLAC picture embedding now follows `settings.disable_coverart_embedding` while preserving normal cover discovery/output flow.
- Implemented `-Q/--eject` runtime behavior in [../src/app.rs](../src/app.rs) with capability-gated Linux libcdio ejection helper in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).

Coverage notes:
- CLI option surface and parse-level action routing are now explicitly audited.
- Added FLAC embedding gate regression tests in [../src/app.rs](../src/app.rs) (`flac_embeds_cover_art_by_default_when_available`, `no_coverart_embed_skips_flac_picture_embedding`).
- Added eject gate regression test in [../src/app.rs](../src/app.rs) (`eject_gate_requires_flag_and_physical_source`).
- End-to-end workflow parity remains pending while run-path hardening and broader differential coverage continue.

Run-path progress:
- `Run` action in [../src/main.rs](../src/main.rs) now executes app-level dispatch instead of placeholder output.
- [../src/app.rs](../src/app.rs) now enforces explicit unsupported-codec and not-yet-wired mode errors for full rip mode.
- `-I` info-only mode now runs and returns a deterministic structured runtime report.
- `-J` cue-only mode now runs and returns deterministic CUE-preview output.
- `-f` find-offset mode now runs and returns deterministic status output.
- Opt-in synthetic full-rip mode now runs through real WAV/FLAC writer flow when `CYANRIP_RS_ENABLE_SYNTHETIC_RIP=1`.
- Synthetic full-rip mode now also supports an image-reader-backed PCM source (`CYANRIP_RS_SYNTHETIC_SOURCE=image-reader`) to exercise reader-to-output plumbing.
- Default `Run` full-rip path now uses reader selection from CLI device kind (image vs physical), acquires frames through the selected reader bridge, and writes outputs via the existing writer flow.
- Default `Run` full-rip bridge now supports selected-track output generation from CLI track selection (`-l`) across configured output formats.
- Default `Run` full-rip bridge now runs paranoia/retry validation during frame acquisition before encoding/writing track outputs.
- Default `Run` full-rip bridge now maps per-track start LSN by selected track number and emits explicit track boundary lines in runtime output.
- Default `Run` full-rip bridge now supports TOC-like boundary overrides via track metadata (`start_lsn`, `frames`, `end_lsn`) with deterministic fallback.
- Default `Run` image-source full-rip bridge now supports `CYANRIP_RS_IMAGE_TOC` boundary overrides (`track:start-end`), taking precedence over track metadata boundaries.
- Default `Run` image-source full-rip bridge now derives per-track starts from CUE `INDEX 01` entries when running with `-d *.cue`, and computes frame spans from adjacent track starts.
- `--find-offset` now uses real physical-drive TOC + AccurateRip lookup + sample-offset probing on linux `backend-libcdio-sys` builds, replacing the staged placeholder runtime.
- `--find-offset` now mirrors core C search behavior more closely: multi-track offset confirmation, conflicting-offset replacement, and doubled-radius retries when no candidate is found.
- Regression coverage added in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs) and `app` unit tests for this dispatch behavior.
