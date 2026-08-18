# Parity Notes

This document tracks implementation differences that are currently accepted and not treated as regressions.

## Current Accepted Differences

1. Help text rendering
- Clap-driven formatting may differ in spacing/wrapping from the original C parser depending on terminal width.
- Option grouping and descriptions are aligned semantically.

2. Naming module scope
- Core naming and sanitation behavior is ported.
- Full integration with runtime metadata dictionaries and output path creation across all artifact types is still in progress.

3. Deterministic modules sequence
- CUE, FUN512 full parity, and log formatting are staged in M2 and not all complete yet.

4. M6 practical hardware validation evidence
- Automated real-drive scenarios (TOC read, frame read, paranoia run, interruption abort path) are now passing on `/dev/cdrom` via `scripts/run_m6_hardware_validation.sh`.
- Manual media-change scenario executed and recorded in [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md).

## Policy

Any new accepted difference must be:
- justified with scope and impact,
- linked to a roadmap step,
- referenced in parity acceptance criteria.

Related documents:
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
