# cyanrip Rust Migration Plan

Last updated: 2026-08-26

This document is the implementation roadmap for porting to Rust in phases, using native Rust libraries whenever practical.

## Goals

- Preserve user-visible behavior and CLI semantics from the C implementation.
- Prefer native Rust crates over C FFI dependencies.
- Keep the project releasable at each milestone.
- Add regression tests at each step so behavior is locked before moving on.

## Current State

- Base crate exists in this repository.
- CLI/config parity is complete and regression-covered.
- Deterministic modules (naming, CUE, log formatting, FUN512/checksum) are complete and test-covered.
- Metadata services (DiscID, MusicBrainz, cover art, AccurateRip fetch/parse) are integrated and test-covered with mocked I/O.
- WAV/FLAC output paths and FLAC tag embedding are implemented with integration tests.
- End-to-end run workflow exists for info-only, cue-only, verify-log, find-offset, and default run bridging; production hardening and final parity closure remain in progress.

## Milestone Checklist

### M0 - Baseline and parity contract

Status: [x]

Checklist:
- [x] Create a feature parity matrix (C feature -> Rust status: done/in-progress/deferred).
- [x] Freeze CLI behavior expectations for defaults, validation, and error messages.
- [x] Collect sample fixtures for cue/log/naming/checksum outputs.
- [x] Define acceptance criteria for parity and allowed differences.

Artifacts:
- PARITY_MATRIX.md
- CLI_BEHAVIOR_FREEZE.md
- tests/fixtures/
- PARITY_ACCEPTANCE_CRITERIA.md

Exit criteria:
- A written parity matrix exists in this repo.
- Deterministic fixtures are available for regression tests.

---

### M1 - CLI and config parity

Status: [x]

Checklist:
- [x] Replace ad-hoc parsing with Clap-based CLI definitions.
- [x] Port all option defaults from cyanrip_main.c.
- [x] Port all validation rules (paranoia, cover size, sanitize, pregap, outputs, mode conflicts).
- [x] Ensure stable and test-covered settings construction.

Suggested crates:
- clap
- thiserror
- anyhow

Exit criteria:
- CLI parsing and validation tests pass and mirror C behavior for target cases.

---

### M2 - Deterministic core modules

Status: [x]

Checklist:
- [x] Port naming and sanitation rules from naming.c.
- [x] Port cue writer behavior from cue_writer.c.
- [x] Port FUN512 and checksum logic from fun512.c and checksums.h path.
- [x] Port logging text formatting behavior from cyanrip_log.c where deterministic.

Suggested crates:
- sha2
- base64
- crc32fast
- chrono
- camino

Exit criteria:
- Snapshot/regression tests for naming, cue, logs, and checksums pass.

---

### M3 - Metadata services

Status: [x]

Checklist:
- [x] Port discid flow from discid.c.
- [x] Port MusicBrainz lookup and mapping from musicbrainz.c.
- [x] Port cover art lookup/download handling from coverart.c.
- [x] Port AccurateRip lookup and parse behavior from accurip.c.

Suggested crates:
- reqwest
- tokio
- serde
- serde_json
- wiremock (tests)

Exit criteria:
- Integration tests with mocked network responses pass.
- Metadata mapping parity is documented for key fields.

---

### M4 - Audio output pipeline (native Rust first)

Status: [x]

Checklist:
- [x] Introduce PCM frame model and processing pipeline interfaces.
- [x] Implement WAV output path.
- [x] Implement FLAC output path.
- [x] Add per-track writer flow and output dispatch for WAV/FLAC.
- [x] Add metadata embedding for FLAC tags (Vorbis comments).
- [x] Keep unsupported codecs behind explicit deferred scope (FLAC/WAV target for current migration).

Suggested crates:
- hound
- flacenc

Exit criteria:
- WAV and FLAC output work end-to-end in integration tests.

---

### M5 - CD reader abstraction and image-backed reader

Status: [~]

Checklist:
- [x] Add a deterministic paranoia-mode rip state machine model (retry, abort, finalize transitions).
- [x] Define Rust traits for drive/media/frame read operations.
- [x] Implement image-backed reader for deterministic CI testing.
- [ ] Port offset and overread policy logic to Rust.
- [x] Add synthetic/fault-injection tests for retry behavior.

Suggested crates:
- cdrtoc (TOC/image metadata parsing)
- thiserror (backend error surface)
- tokio (optional async backend adapters)

Exit criteria:
- Image-based reads and policy behavior are testable and stable.
- Paranoia state transitions are covered by regression tests and reusable by both image and physical backends.

---

### M6 - Physical drive support and reliability layer

Status: [x]

Checklist:
- [x] Implement Linux physical drive backend (feature-gated libcdio-sys adapter, dependency checks, hardware validation scenarios, and manual media-change validation all completed).
- [x] Wire the Rust paranoia state machine to real frame reads, retries, and encoder flush transitions.
- [x] Port/replicate paranoia-like overlap/verify/retry heuristics and callback counters.
- [x] Add media-changed and interruption handling.
- [x] Validate practical ripping behavior on real hardware (TOC/frame/paranoia/interruption scenarios passed on target drive; manual media-change scenario executed and recorded).

Suggested crates:
- nix (Linux ioctl and descriptor safety)
- libc (minimal FFI shims where no higher-level Rust crate exists)

Exit criteria:
- Real-drive ripping works with acceptable reliability.
- Real-drive runs demonstrate parity for retry/abort/finalize behavior under induced read-failure scenarios.

---

### M7 - Full workflow integration and release parity

Status: [~]

Checklist:
- [~] Wire end-to-end command workflow.
- [x] Main dispatches into app-level run workflow.
- [x] `-I`, `-J`, and `--verify-log` runtime modes are functional and test-covered.
- [x] `--find-offset` uses Linux/libcdio TOC + AccurateRip lookup + sample-offset probing, including multi-track confirmation/conflict replacement and radius escalation.
- [~] Default run path bridges selected tracks through reader -> paranoia validation -> writer flow for image and physical sources; production hardening and remaining parity details are pending.
- [~] Add differential tests against C binary on shared fixtures (CLI and verify-log slices complete; broader workflow differentials pending).
- [ ] Finalize compatibility notes and known differences.
- [ ] Prepare release checklist and migration notes.

Exit criteria:
- End-to-end regression suite is green.
- Known differences are documented and accepted.

## Module Mapping (C -> Rust target)

- cyanrip_main.c -> src/main.rs + src/cli.rs + src/app.rs
- naming.c -> src/naming.rs
- cue_writer.c -> src/cue.rs
- cyanrip_log.c -> src/log_report.rs
- fun512.c -> src/fun512.rs
- utils.c -> src/utils.rs
- discid.c -> src/metadata/discid.rs
- musicbrainz.c -> src/metadata/musicbrainz.rs
- coverart.c -> src/metadata/coverart.rs
- accurip.c -> src/metadata/accurip.rs
- metadata orchestration in cyanrip_main.c flow -> src/app.rs
- cyanrip_encode.c -> src/audio/mod.rs + format-specific modules
- fifo_frame.c/fifo_packet.c -> src/audio/queue.rs (channel-based)

## Repository Implementation Plan (remaining sequence)

1. Close remaining M5 parity gaps
- [ ] Port and validate offset/overread policy behavior against C references for image and physical readers.

2. Harden default full-rip production path (M7)
- [ ] Improve physical-drive boundary/error handling parity in the non-synthetic run path.
- [ ] Complete AccurateRip checksum verification wiring in rip-time path and finish-summary reporting.

3. Expand differential coverage
- [ ] Add fixture-backed workflow differential tests beyond CLI/verify-log.
- [ ] Add targeted differential cases for find-offset and selected-track run behavior.

4. Finalize release parity package
- [ ] Document known differences and accepted deviations.
- [ ] Prepare release checklist and migration notes.

## Testing Plan

- Unit tests for parsers, validators, naming, checksum logic.
- Snapshot tests for cue/log outputs.
- Integration tests for metadata modules with mocked services.
- End-to-end tests using local image fixtures.
- Differential tests against the C binary for selected scenarios.
- State-machine regression tests for paranoia transitions (retry loops, media-change abort, retry-limit finalize).
- Hardware-gated validation scenarios for Linux/libcdio reader parity and interruption/media-change behavior.

## Risk and Mitigation

1. Native Rust codec parity gaps
- Mitigation: prioritize WAV/FLAC first, gate other codecs as deferred/experimental.

2. CDDA reliability complexity
- Mitigation: isolate retry/paranoia logic as policy layer with fault-injection tests.

3. External API behavior drift
- Mitigation: schema-validated parsing and recorded fixtures.

4. Cross-platform differences
- Mitigation: complete Linux first, add platform capability matrix.

## Definition of Done for each milestone

- Checklist complete.
- Tests added and passing.
- Behavior changes documented.
- Next milestone prerequisites confirmed.
