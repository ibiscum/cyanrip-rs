# Next Steps

<<<<<<< HEAD
This list is derived from the migration roadmap and current parity state.

## Immediate Priority (Post-M2)

1. Start M3 metadata services with mocked I/O first.
- Create metadata module structure in src/metadata.
- Introduce HTTP client abstraction and wiremock-based tests.

2. Port discid flow from discid.c.
- Define deterministic mapping tests before implementation.
- Keep I/O boundaries injectable for integration tests.

Status update:
- Metadata module structure: done
- DiscID deterministic flow and tests: done

3. Port MusicBrainz lookup and mapping from musicbrainz.c.
- Introduce injectable HTTP client trait and response decoding layer.
- Add wiremock fixtures for release/disc lookup paths.

Status update:
- MusicBrainz lookup/mapping core: done
- Injectable HTTP trait + wiremock fixtures: done

4. Port cover art lookup/download handling from coverart.c.
- Add metadata-to-download selection logic tests first.

Status update:
- Cover art lookup/download core: done
- Injectable HTTP + wiremock tests: done

5. Port AccurateRip lookup and parse behavior from accurip.c.
- Add parser tests using deterministic fixture blobs.

Status update:
- AccurateRip lookup/parser core: done
- Injectable HTTP + deterministic fixture and wiremock tests: done

6. Integrate metadata flow orchestration in app path.
- Wire DiscID -> MusicBrainz -> Cover Art -> AccurateRip in deterministic order.
- Keep disable flags and fallback behavior parity.

Status update:
- Metadata orchestration module in app path: done
- Disable-flag and fallback flow tests: done

7. Start M4 audio output pipeline.
- Implement WAV output path first with integration tests.

Status update:
- WAV output path: done
- WAV end-to-end write/read integration test: done

8. Continue M4 audio output pipeline.
- Implement FLAC output path with deterministic integration tests.

Status update:
- FLAC output path: done
- FLAC end-to-end write/read integration test: done

9. Continue M4 audio output pipeline.
- Add per-track writer flow and output dispatch for WAV/FLAC.

Status update:
- App-level per-track output dispatch for WAV/FLAC: done
- Dispatch behavior tests (write paths + unsupported format error): done

10. Continue M4 audio output pipeline.
- Add metadata embedding in per-track writer flow where codec support exists.
- Keep unsupported codecs behind explicit deferred errors.

Status update:
- FLAC Vorbis-comment metadata embedding in per-track writer flow: done
- Tag propagation tests for album/track/disc fields: done

11. Continue M4 and app integration.
- Add app-path integration tests that connect CLI settings to metadata orchestration entrypoints.
- Expand metadata embedding coverage to additional codecs as they are implemented.

Status update:
- App-path integration tests from CLI parsing to metadata orchestration/output writer entrypoints: done
- CLI-driven output dispatch and FLAC tag propagation integration checks: done

12. Continue M4 and app integration.
- Expand metadata embedding coverage to additional codecs as they are implemented.
- Keep unsupported codecs behind explicit deferred errors until implemented.

13. Start M5 paranoia-mode ripping control path.
- Add CDDA paranoia state-machine module with deterministic transition tests.
- Freeze parity expectations for retry and abort behavior against /cyanrip/src/cyanrip_main.c.

Status update:
- Paranoia state-machine module and regression tests: done
- Upstream behavior mapping and transition document: done

14. Continue M5 reader abstraction for paranoia mode.
- Define frame-reader traits that can emit read success/error/media-changed events.
- Add image-backed reader fixture path that drives the state machine without physical hardware.

Status update:
- Frame-reader traits and runtime state-machine bridge: done
- Image-backed fake reader with fault-injection tests: done

15. Continue M6 physical backend and reliability.
- Wire Linux drive reads into the same state-machine events.
- Add induced read-failure and interruption tests to assert parity around retries/finalization.

Status update:
- Linux physical-drive adapter implementing CddaFrameReader under cdda feature: done
- Hardware-free backend regression tests for seek/read/media-changed mapping: done
- Real libcdio-backed compile/test path: done
- Real-drive hardware validation harness (TOC/frame/paranoia/interruption + manual media-change scenario): done
- Practical reliability acceptance runbook and notes template: done
- Full practical reliability evidence capture on target hardware matrix: done

## After M2

16. Continue parity matrix maintenance as each feature lands.
- Update statuses in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).
- Record accepted differences in [PARITY_NOTES.md](PARITY_NOTES.md).

17. Start M7 differential harness against C binary.
- Add hardware-independent CLI differential cases first.
- Expand to fixture-backed workflow comparisons once runtime command path is complete.

Status update:
- Differential CLI first-slice harness: done
- Option-surface parity audit for CLI short flags: done
- Fixture-backed and end-to-end workflow differential comparisons: pending
- Run action dispatch wiring (mode/codec gating): done

18. Continue M7 run-workflow implementations after dispatch slice.
- Harden default reader-selected full-rip bridge into production workflow (true TOC-derived track boundaries instead of bridge-calculated synthetic windows, robust physical-drive error handling, and final output/log parity details).
- Keep synthetic full-rip path as hardware-free regression path while production flow matures.
- Continue find-offset parity hardening using [FIND_OFFSET_PARITY_PLAN.md](FIND_OFFSET_PARITY_PLAN.md).

Status update:
- Image-source cue-derived TOC boundaries (`-d *.cue`, `INDEX 01` based) in default run path: done
- Image-source explicit override path (`CYANRIP_RS_IMAGE_TOC`) with precedence over metadata/defaults: done
- Physical-source true TOC extraction and boundary/error parity hardening: pending
- Find-offset core parity (multi-track confirmation, conflicting-offset replacement, and radius escalation): done
- Find-offset remaining parity: mixed-mode TOC track typing and differential-vs-C validation pending

## Paranoia Mode Design Reference

- See [CDDA_PARANOIA_STATE_MACHINE.md](CDDA_PARANOIA_STATE_MACHINE.md) for states, events, and parity notes.

## Definition of Ready For Each Task

A task should only move to implementation when:
- acceptance behavior is described in tests,
=======
Last updated: 2026-08-28

This list tracks remaining work to close migration milestones and release parity.

## Active Milestone Focus

1. M5 closeout: offset and overread policy parity
- Port and validate offset/overread policy behavior against C references for both image and physical readers.
- Add deterministic regression tests for boundary handling and overread edge cases.

2. M7 hardening: production full-rip path
- Harden physical-drive full-rip boundary/error handling (track boundaries, media-change, read-retry escalation).
- Keep synthetic full-rip path as the hardware-free regression baseline while production path matures.

3. M7 parity: upstream-integrated paranoia loop
- Replace current precheck-plus-direct-read behavior with one integrated paranoia frame loop for paranoia-enabled runs.
- Align retry/repair behavior and frame-failure policy to upstream expectations, including callback-driven status accounting.
- Add differential and regression coverage for media-change, retry-limit finalize, and repeat-rip interactions.

4. M7 parity: AccurateRip runtime verification
- Wire rip-time AccurateRip checksum generation/match reporting into run path.
- Feed finish-summary parity lines from runtime match outcomes.

5. M7 differential expansion
- Expand differential harness beyond CLI/verify-log into fixture-backed workflow slices (run-path and metadata/output behavior).
- Add focused differential cases for find-offset and selected-track run behavior.

6. Release parity package
- Finalize known differences and accepted deviations in parity notes/matrix.
- Prepare release checklist and migration notes.

## Milestone Health Snapshot

- M0 baseline/parity contract: complete
- M1 CLI/config parity: complete
- M2 deterministic modules: complete
- M3 metadata services: complete
- M4 audio output pipeline (WAV/FLAC scope): complete
- M5 CD reader abstraction/image-backed behavior: in progress
- M6 Linux physical-drive support/reliability: complete
- M7 full workflow integration/release parity: in progress

## Definition of Ready

A task is ready only when:
- acceptance behavior is represented in tests,
>>>>>>> feature/metadata_a
- fixture inputs are available or created,
- expected outputs are deterministic and documented.
