# Next Steps

Last updated: 2026-08-26

This list tracks remaining work to close migration milestones and release parity.

## Active Milestone Focus

1. M5 closeout: offset and overread policy parity
- Port and validate offset/overread policy behavior against C references for both image and physical readers.
- Add deterministic regression tests for boundary handling and overread edge cases.

2. M7 hardening: production full-rip path
- Harden physical-drive full-rip boundary/error handling (track boundaries, media-change, read-retry escalation).
- Keep synthetic full-rip path as the hardware-free regression baseline while production path matures.

3. M7 parity: AccurateRip runtime verification
- Wire rip-time AccurateRip checksum generation/match reporting into run path.
- Feed finish-summary parity lines from runtime match outcomes.

4. M7 differential expansion
- Expand differential harness beyond CLI/verify-log into fixture-backed workflow slices.
- Add focused differential cases for find-offset and selected-track run behavior.

5. Release parity package
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
- fixture inputs are available or created,
- expected outputs are deterministic and documented.
