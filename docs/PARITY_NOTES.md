# Parity Notes

Last updated: 2026-09-11

This document tracks implementation differences that are currently accepted and not treated as regressions.

## Current Accepted Differences

1. Help text rendering
- Clap-driven formatting may differ in spacing/wrapping from the original C parser depending on terminal width.
- Option grouping and descriptions are aligned semantically.

2. Codec/output scope
- Current Rust runtime scope is WAV/FLAC-centric.
- Wider codec parity remains explicitly deferred and tracked in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).

3. AccurateRip finish-summary parity (accepted remaining gap)
- Per-track AccurateRip v1/v2 verification is implemented and emitted during ripping.
- Remaining parity gap is finish-summary aggregation counts (`Tracks ripped accurately` / `Tracks ripped partially accurately`) in runtime bridge output.
- This is tracked in [../PARITY_MATRIX.md](../PARITY_MATRIX.md) and [Next_Steps.md](Next_Steps.md).

4. Hardware/backend-dependent eject behavior
- `-Q/--eject` is implemented in Linux `backend-libcdio-sys` path with capability checks.
- On unsupported builds/backends/hardware, behavior safely degrades to no-op.

5. M6 practical hardware validation evidence
- Automated real-drive scenarios (TOC read, frame read, paranoia run, interruption abort path) are passing on `/dev/cdrom` via `scripts/run_m6_hardware_validation.sh`.
- Manual media-change scenario executed and recorded in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).

6. Paranoia callback/status closure on real hardware (accepted remaining gap)
- Physical full-rip now uses the integrated paranoia reader path with a single native session reused across tracks, and paranoia-produced frames are consumed directly.
- Remaining work is broader real-hardware edge-case coverage and callback/status parity closure.
- This is tracked in [../PARITY_MATRIX.md](../PARITY_MATRIX.md) and [paranoia_upstream_parity_plan.md](paranoia_upstream_parity_plan.md).

7. Ripping availability vs parity completeness
- Full-rip execution paths are functional in current scope (image + linux physical), including acquisition, processing, naming, writing, and per-track summary output.
- Open items are parity-completeness deltas, not baseline ripping availability gaps.

## Policy

Any new accepted difference must be:
- justified with scope and impact,
- linked to a roadmap step,
- referenced in parity acceptance criteria.

Related documents:
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
