# Migration Changelog

Milestone status vocabulary in headings follows: complete, in progress, planned, deferred.

## 2026-08-26

### Documentation: `-a/--album-meta` Option Flow
- Added [ALBUM_META_OPTION_FLOW.md](album-meta_option_flow.md), documenting purpose, input format, precedence, and runtime consumption points for `-a/--album-meta`.
- Linked the new document from [README.md](README.md) in this docs directory.

## 2026-08-25

### `-B/--outputroot` Help and Option-Flow Documentation
- Clarified CLI `-h` help text for [../src/cli.rs](../src/cli.rs): `-B/--outputroot` now explicitly states it overrides `CYANRIP_RS_OUTPUT_ROOT`.
- Added help-output regression coverage in [../src/cli.rs](../src/cli.rs) to lock the new `-B/--outputroot` description.
- Updated CLI behavior freeze defaults and precedence notes in [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md) to include `output_root` and output-root resolution order.
- Added dedicated option-flow documentation in [OUTPUTROOT_OPTION_FLOW.md](OUTPUTROOT_OPTION_FLOW.md), including precedence rules, behavior notes, and usage examples.
- Linked the new flow document from [README.md](README.md) in this docs directory.

### Streaming Track-By-Track Full-Rip Pipeline
- Refactored full-rip execution in [../src/app.rs](../src/app.rs) to process one track at a time (acquire PCM, encode/write outputs, then drop PCM) instead of buffering all selected tracks before writing.
- Applied the same streaming approach to synthetic full-rip mode in [../src/app.rs](../src/app.rs).
- Added internal writer helpers in [../src/app.rs](../src/app.rs) to preserve naming context and collision-warning behavior while enabling per-track writes.
- Kept public writer API compatibility (`write_track_outputs`) and verified behavior through focused workflow integration tests.
- Added runtime benchmark reporting in [../src/app.rs](../src/app.rs): each track summary now includes a compact `Benchmark` line (elapsed ms, PCM buffer size, and current RSS when available), and full-rip output includes a Linux `/proc`-based `Peak RSS` line when available.
- Added live encoding-phase progress updates in [../src/app.rs](../src/app.rs) with percentage and ETA (`Encoding track ...`) during per-track output writing.
- Renamed the physical read-phase live status line from `Ripping and encoding track ...` to `Ripping track ...` in [../src/app.rs](../src/app.rs).

### Test Artifact Output Root Normalization (`tmp/`)
- Updated audio-producing tests to write generated files/directories under the repository-local `tmp/` folder (git-ignored) instead of system temp paths.
- Updated helper paths in [../tests/wav_pipeline.rs](../tests/wav_pipeline.rs), [../tests/flac_pipeline.rs](../tests/flac_pipeline.rs), [../tests/app_cli_integration.rs](../tests/app_cli_integration.rs), [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs), and [../src/app.rs](../src/app.rs) test module.

## 2026-08-24

### M7 Paranoia Main-Flow Wiring and `--no-accurip` Option Flow (in progress at the time)
- Cross-checked upstream docs and source (`main-flow.md`, `no-accurip-option-flow.md`, and `src/cyanrip_main.c`) and aligned info/no-accurip interactions in [../src/cli.rs](../src/cli.rs): `-I` keeps AccurateRip behavior user-controlled (via `-A`) while still disabling eject side effects.
- Kept find-offset override semantics intact in [../src/cli.rs](../src/cli.rs): `-f/--find-offset` re-enables AccurateRip while disabling MusicBrainz/Cover Art DB and resetting offset/eject side effects.
- Hardened physical-drive paranoia integration in [../src/app.rs](../src/app.rs) so the preflight paranoia run is now state-machine validated and must end in `TrackComplete`; non-complete end states are surfaced as runtime errors.
- Added integration coverage in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs) for info-mode AccurateRip default-vs-`-A` behavior and explicit paranoia/retry full-rip bridge execution.

## 2026-08-22

### M7 Info-Only MusicBrainz Release Selection Parity (in progress at the time)
- Updated `-I` info-only workflow in [../src/app.rs](../src/app.rs) to perform MusicBrainz release resolution when DiscID is available and MusicBrainz is enabled.
- Implemented upstream-compatible multi-release behavior for info-only mode: if multiple releases are returned and no `-R` selection is provided, the run now exits non-zero with a candidate list and explicit `-R` guidance.
- Extended `-I` reporting in [../src/app.rs](../src/app.rs) to include selected MusicBrainz release-level fields (`Release ID`, `Album`, `Album artist`, `Disc number`, `Total discs`) and per-track metadata blocks when a release is selected via `-R`.
- Added captured live DiscID request/response fixtures in [../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.request.txt](../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.request.txt), [../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.json](../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.json), and [../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.upstream_output.txt](../tests/fixtures/musicbrainz/discid_bkkz_multi_release_live.upstream_output.txt).
- Added regression coverage for the captured live fixture in [../src/metadata/musicbrainz.rs](../src/metadata/musicbrainz.rs), including release-index 1 and release-index 2 mapping assertions.
- Added ignored hardware/network integration coverage in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs) for `-I -R 1` and `-I -R 2`, asserting release-specific metadata differences in output.
- Added ignored hardware integration coverage for `-I` multi-release selection error-path behavior in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs).
- Added helper runner script [../scripts/run_m7_info_release_disambiguation.sh](../scripts/run_m7_info_release_disambiguation.sh) to execute the two ignored `run_workflow_cli` disambiguation tests (`-R 1` and `-R 2`) with required features and environment defaults.

### M7 Cue-Only Offset-Unset Parity (in progress at the time)
- Matched upstream `-J` behavior for unset offset: runtime returns the message `Offset is unset! To continue with an offset of 0, run with -s 0!` and exits successfully.
- Added `Settings.offset_is_set` tracking in [../src/lib.rs](../src/lib.rs) and CLI mapping in [../src/cli.rs](../src/cli.rs) to preserve explicit-vs-default offset intent.
- Added cue-only runtime parity handling in [../src/app.rs](../src/app.rs).
- Added recorded upstream observation fixture [../tests/fixtures/cli/cue_only_offset_unset_upstream.txt](../tests/fixtures/cli/cue_only_offset_unset_upstream.txt) and integration regression assertion in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs).

### Architecture Documentation: `cyanrip_ctx` Replacement
- Added [ARCHITECTURE_CONTEXT_MAPPING.md](ARCHITECTURE_CONTEXT_MAPPING.md) documenting how upstream monolithic context responsibilities are represented in Rust through `Settings`, workflow-level typed inputs/outputs, metadata orchestration structs, TOC/drive structs, and output flow structs.
- Documented rationale for avoiding a new monolithic mutable context in favor of explicit ownership boundaries and feature-localized state.

## 2026-08-20

### M7 CLI Info-Only Parity Tightening (in progress at the time)
- Enforced `-I` info-only no-eject semantics in [../src/cli.rs](../src/cli.rs) so `-Q` is ignored in info-only mode.
- Extended info-only runtime reporting in [../src/app.rs](../src/app.rs) to explicitly state no ripping/no eject behavior and include selected-track summary.
- Implemented linux+libcdio-backed `-I` TOC readout in [../src/app.rs](../src/app.rs), including per-track LSN/frame/duration lines and local DiscID/CDDB derivation from real TOC data.
- Added parser-level regression test for `-I -Q` behavior in [../src/cli.rs](../src/cli.rs).
- Extended integration coverage in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs) to assert info-only no-eject output semantics.
- Updated CLI freeze documentation in [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md).

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

### M2 Deterministic Core Modules (in progress at the time)
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

### M3 Metadata Services (in progress at the time)
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

### M4 Audio Output Pipeline (in progress at the time)
- Added audio module scaffold in [../src/audio/mod.rs](../src/audio/mod.rs).
- Implemented WAV output writer in [../src/audio/wav.rs](../src/audio/wav.rs) using hound.
- Added WAV end-to-end integration test in [../tests/wav_pipeline.rs](../tests/wav_pipeline.rs).
- Implemented FLAC output writer in [../src/audio/flac.rs](../src/audio/flac.rs) using flacenc.
- Added FLAC end-to-end integration test in [../tests/flac_pipeline.rs](../tests/flac_pipeline.rs) using claxon for decode verification.
- Implemented app-level per-track output dispatch in [../src/app.rs](../src/app.rs) so selected output formats drive concrete WAV/FLAC file emission.
- Added dispatch tests for successful per-track output writes and unsupported-format error paths.
- Implemented FLAC Vorbis-comment metadata embedding in [../src/app.rs](../src/app.rs) using metaflac, aligned with C metadata propagation behavior.
- Added app-level tests validating embedded FLAC tag fields for album/track/disc metadata.
- Added CLI-to-app integration tests in [../tests/app_cli_integration.rs](../tests/app_cli_integration.rs) that validate orchestration disable-flag behavior and CLI-driven output dispatch/tag propagation.

### M5 Paranoia Control Path (in progress at the time)
- Added CDDA paranoia state-machine scaffolding in [../src/cdda/paranoia.rs](../src/cdda/paranoia.rs).
- Added regression tests for retry-loop transitions, retry-limit finalize behavior, media-change abort handling, and fatal pipeline error handling.
- Updated roadmap and parity planning docs to include explicit milestones for wiring paranoia control logic into image-backed and physical-drive readers.

### M5 Reader Runtime Integration (in progress at the time)
- Added CDDA reader trait and paranoia-oriented track runner in [../src/cdda/reader.rs](../src/cdda/reader.rs).
- Added image-backed fake reader with injected read-failure and media-change behaviors for deterministic fault-injection tests.
- Added Cargo features for cdda/paranoia availability and backend planning flags (including libcdio-sys and a libcdio-rs planning feature) in [../Cargo.toml](../Cargo.toml).

### M6 Linux Physical-Drive Adapter (in progress at the time)
- Added Linux physical-drive adapter scaffold in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs) that implements the shared CDDA frame reader trait and maps media-change semantics into the same paranoia event pipeline.
- Added backend abstraction tests for seek/read progression, read-failure propagation, media-changed mapping parity, and backend cleanup behavior without requiring hardware.
- Added optional libcdio-sys backend wiring for real-drive reads behind feature flags.
- Updated libcdio-sys dependency/features to a safe set that avoids UDF headers and validates with `cargo check --features "backend-libcdio-sys paranoia"` and `cargo test --features "backend-libcdio-sys paranoia"`.

### M6 Linux Real-Drive Validation Harness (in progress at the time)
- Added ignored hardware regression tests in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs) that open `/dev/cdrom`, read TOC entries, and read one CDDA frame through the libcdio-backed adapter.
- Added prerequisite checker script in [../scripts/check_linux_cdda_stack.sh](../scripts/check_linux_cdda_stack.sh) for pkg-config-visible libcdio libraries and feature-gated Rust compilation.
- Documented real-drive validation commands and explicit "insert audio CD beforehand" prerequisite in [../README.md](../README.md).

### M6 Linux Paranoia Runtime Wiring (in progress at the time)
- Added Linux paranoia runner APIs in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs) that wire physical-drive reads into `run_track_with_paranoia` (including frame retries and finalize flush transitions).
- Added backend-mock regression tests for retry/error and media-change abort paths in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).
- Added ignored hardware integration test in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs) to validate paranoia-run completion against a readable audio CD.

### M6 Paranoia Heuristics and Callback Counters (in progress at the time)
- Added paranoia callback counter model in [../src/cdda/reader.rs](../src/cdda/reader.rs) with counters aligned to upstream status categories (READ/VERIFY/OVERLAP/READERR/WROTE/FINISHED and related entries).
- Added overlap/verify heuristic configuration and runtime handling in [../src/cdda/reader.rs](../src/cdda/reader.rs), including drift detection that forces retry behavior until convergence or retry limit.
- Added Linux helper `heuristics_for_paranoia_level` and heuristic-aware runner variants in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).
- Added regression tests for overlap-drift retry behavior and level-based verify/overlap defaults in [../src/cdda/reader.rs](../src/cdda/reader.rs) and [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).

### M6 Media-Changed and Interruption Handling (in progress at the time)
- Added interruptible paranoia runtime path in [../src/cdda/reader.rs](../src/cdda/reader.rs) that emits `QuitRequested` and transitions to aborted state while preserving callback counters.
- Added linux-drive interruptible runner variants in [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs) so physical-backend runs can share the same interruption semantics.
- Added regression tests for interruption abort behavior and existing media-change abort behavior in [../src/cdda/reader.rs](../src/cdda/reader.rs) and [../src/cdda/linux_drive.rs](../src/cdda/linux_drive.rs).

### M6 Practical Real-Hardware Reliability Scenarios (in progress at the time)
- Added real-hardware interruption validation test and manual media-change scenario reference in [../tests/linux_physical_drive_validation.rs](../tests/linux_physical_drive_validation.rs).
- Added practical M6 scenario runbook and acceptance-notes template in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).
- Added scripted scenario runner in [../scripts/run_m6_hardware_validation.sh](../scripts/run_m6_hardware_validation.sh) to execute TOC/frame/paranoia/interruption checks consistently.
- Executed manual media-change scenario once and recorded result details in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md), including operator prompt text shown during test run.

### M7 Differential Harness (in progress at the time)
- Added ignored differential CLI test harness in [../tests/differential_cli_vs_c.rs](../tests/differential_cli_vs_c.rs) to compare Rust and C binary behavior for deterministic CLI scenarios.
- Added CLI option-surface parity audit test in [../src/cli.rs](../src/cli.rs) to validate upstream short-option coverage.
- Added M7 runbook and execution script in [M7_DIFFERENTIAL_HARNESS.md](M7_DIFFERENTIAL_HARNESS.md) and [../scripts/run_m7_cli_diff.sh](../scripts/run_m7_cli_diff.sh).
- Implemented real `-Y/--verify-log` runtime handling in [../src/main.rs](../src/main.rs) using FUN512 verification outcomes and C-matching status messages.
- Expanded differential harness cases to include verify-log fixture outcomes (`valid`, `mismatch`, `no_checksum`, `trailing`, and missing-file I/O error).
- Added always-on Rust CLI integration coverage for verify-log status/message mapping in [../tests/verify_log_cli.rs](../tests/verify_log_cli.rs) so parity checks run without requiring the C binary.

### M7 Run Workflow Wiring (in progress at the time)
- Replaced placeholder `Run` print path in [../src/main.rs](../src/main.rs) with dispatch to app-level workflow handling.
- Added app-level workflow gate in [../src/app.rs](../src/app.rs) that now reports explicit not-yet-wired mode paths and unsupported output codecs with non-zero exit behavior.
- Added new tests in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs) and app unit tests to lock command-path behavior for this slice.
- Implemented `-I` info-only run path in [../src/app.rs](../src/app.rs) with deterministic runtime report output and success exit behavior.
- Implemented `-J` cue-only run path in [../src/app.rs](../src/app.rs) with deterministic CUE-preview output and success exit behavior.
- Implemented `-f` find-offset run path in [../src/app.rs](../src/app.rs) with deterministic status output and success exit behavior (disc-driven offset computation wiring remains pending).
- Added an opt-in synthetic full-rip run slice in [../src/app.rs](../src/app.rs) gated by `CYANRIP_RS_ENABLE_SYNTHETIC_RIP=1`, which exercises the real WAV/FLAC writer flow and reports written files.
- Added integration coverage for the synthetic full-rip path in [../tests/run_workflow_cli.rs](../tests/run_workflow_cli.rs), including output-root override via `CYANRIP_RS_OUTPUT_ROOT`.
- Extended synthetic full-rip mode with `CYANRIP_RS_SYNTHETIC_SOURCE=image-reader` to source PCM through the image-backed CDDA frame reader before writing outputs.
- Replaced default full-rip "not wired" path with a reader-selected full-rip bridge in [../src/app.rs](../src/app.rs): source is chosen from CLI device kind (`image` for cue/bin/nrg/toc or default, `physical` for device-like paths), frames are acquired from the selected reader, and outputs are produced via the existing writer flow.
- Extended default full-rip bridge to honor selected CLI tracks (`-l`) and write multi-track outputs through the same reader-acquisition and writer pipeline.
- Integrated paranoia/retry validation into the full-rip bridge frame acquisition path for both image and physical sources before output writing.
- Updated full-rip bridge track boundary mapping to use selected track numbers for start LSN calculation, and added explicit per-track START_LSN/FRAMES reporting in run output.
- Added TOC-like boundary resolution support in the full-rip bridge using per-track metadata overrides (`start_lsn`, `frames`, `end_lsn`) with deterministic fallback mapping.
- Added image-source TOC override support via `CYANRIP_RS_IMAGE_TOC` (`track:start-end` list), which takes precedence over metadata overrides in default image full-rip runs.
- Added cue-derived image TOC boundary extraction for `-d *.cue` runs in default full-rip mode (`INDEX 01` start LSN mapping with next-track frame spans), used before metadata fallback.
- Implemented a real linux+libcdio `--find-offset` runtime path: physical TOC read, AccurateRip lookup, and sample-offset probing around +450 frame checksums (with graceful unsupported-build reporting when required features are missing).

### Validation
- Test suite passing after each major change set (cargo test).

### Licensing and Compliance
- Re-licensed repository to LGPL-2.1-or-later and replaced top-level [../LICENSE](../LICENSE) text accordingly.
- Added SPDX license metadata in [../Cargo.toml](../Cargo.toml).
- Added upstream attribution/notice guidance in [../UPSTREAM_NOTICES.md](../UPSTREAM_NOTICES.md).
- Added dependency license inventory in [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
- Added bundled common license texts in [../licenses/Apache-2.0.txt](../licenses/Apache-2.0.txt), [../licenses/MIT.txt](../licenses/MIT.txt), and [../licenses/ISC.txt](../licenses/ISC.txt).
