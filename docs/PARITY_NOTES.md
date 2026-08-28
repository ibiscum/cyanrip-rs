# Parity Notes

Last updated: 2026-08-28

This document tracks implementation differences that are currently accepted and not treated as regressions.

## Current Accepted Differences

1. Help text rendering
- Clap-driven formatting may differ in spacing/wrapping from the original C parser depending on terminal width.
- Option grouping and descriptions are aligned semantically.

2. Codec/output scope
- Current Rust runtime scope is WAV/FLAC-centric.
- Wider codec parity remains explicitly deferred and tracked in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).

3. AccurateRip runtime verification depth
- AccurateRip DB fetch/parsing exists and is exercised, but full rip-time checksum generation/match-summary parity is still planned.
- This is tracked as planned work in [../PARITY_MATRIX.md](../PARITY_MATRIX.md) and [Next_Steps.md](Next_Steps.md).

4. Hardware/backend-dependent eject behavior
- `-Q/--eject` is implemented in Linux `backend-libcdio-sys` path with capability checks.
- On unsupported builds/backends/hardware, behavior safely degrades to no-op.

5. M6 practical hardware validation evidence
- Automated real-drive scenarios (TOC read, frame read, paranoia run, interruption abort path) are passing on `/dev/cdrom` via `scripts/run_m6_hardware_validation.sh`.
- Manual media-change scenario executed and recorded in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).

## Policy

Any new accepted difference must be:
- justified with scope and impact,
- linked to a roadmap step,
- referenced in parity acceptance criteria.

Related documents:
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
