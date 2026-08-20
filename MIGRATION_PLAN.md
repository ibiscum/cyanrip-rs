# cyanrip Rust Migration Plan

This document is the implementation roadmap for porting to Rust in phases, using native Rust libraries whenever practical.

## Goals

- Preserve user-visible behavior and CLI semantics from the C implementation.
- Prefer native Rust crates over C FFI dependencies.
- Keep the project releasable at each milestone.
- Add regression tests at each step so behavior is locked before moving on.

## Current State

- Base crate exists in this repository.
- Initial core parsing and validation logic has been ported with unit tests.
- No end-to-end ripping flow yet.
- Paranoia-mode rip control state machine is now scaffolded in Rust with regression tests; CDDA frame I/O wiring is still pending.

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

Status: [~]

Checklist:
- [x] Introduce PCM frame model and processing pipeline interfaces.
- [x] Implement WAV output path.
- [x] Implement FLAC output path.
- [x] Add per-track writer flow and output dispatch for WAV/FLAC.
- [x] Add metadata embedding for FLAC tags (Vorbis comments).
- [ ] Add metadata embedding for additional codecs as implemented.
- [ ] Keep unsupported codecs behind explicit feature flags or deferred list.

Suggested crates:
- hound
- flacenc

Exit criteria:
- WAV and FLAC output work end-to-end in integration tests.

---

### M5 - CD reader abstraction and image-backed reader

Status: [ ]

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
- [~] Wire end-to-end command workflow (main now dispatches to app-level run workflow; info-only, cue-only, and find-offset modes now execute successfully with structured output; `--find-offset` now runs a real physical-drive path on linux+libcdio builds using drive TOC + AccurateRip lookup + sample-offset probing around frame 450, including multi-track confirmation/conflict replacement and radius escalation; default Run now selects reader source from CLI settings/device kind (image vs physical), runs paranoia/retry validation on selected tracks, resolves TOC-like per-track frame boundaries (cue-derived image TOC + image-TOC env override + metadata override + deterministic fallback), acquires frames, and writes selected tracks through a multi-track full-rip bridge path; opt-in synthetic full-rip mode remains available for hardware-free testing; hardware-backed production ripping flow is still pending hardening).
- [~] Add differential tests against C binary on shared fixtures (CLI-first-slice harness added and verify-log fixture cases included; broader workflow differentials pending).
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

## Repository Implementation Plan (working sequence)

1. Establish structure and ownership
- [ ] Create module tree and placeholder files for M1 and M2.
- [ ] Keep domain types centralized in src/lib.rs or src/domain.rs.

2. Finish M1 fully
- [ ] Add Clap parser and map to existing Settings.
- [ ] Add CLI regression tests for valid/invalid command lines.

3. Implement M2 in this order
- [ ] naming
- [ ] cue
- [ ] fun512/checksum
- [ ] deterministic log report formatting

4. Implement M3 with mocked I/O first
- [ ] Add HTTP client traits.
- [ ] Add wiremock tests before live calls.

5. Implement M4 minimal output set
- [ ] WAV first.
- [ ] FLAC second.
- [ ] Defer unsupported formats with explicit errors.

6. Implement M5 and M6
- [x] Add paranoia state machine scaffold and regression tests.
- [ ] Build image-reader backend for CI.
- [ ] Add physical-reader backend behind cfg(target_os = "linux").
- [ ] Integrate state machine events into read/decode/encode loop.

7. Complete M7
- [ ] Differential output comparison harness.
- [ ] Final parity report.

## Testing Plan

- Unit tests for parsers, validators, naming, checksum logic.
- Snapshot tests for cue/log outputs.
- Integration tests for metadata modules with mocked services.
- End-to-end tests using local image fixtures.
- Differential tests against the C binary for selected scenarios.
- State-machine regression tests for paranoia transitions (retry loops, media-change abort, retry-limit finalize).

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
