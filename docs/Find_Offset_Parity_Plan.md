# Find-Offset Parity Plan (C -> Rust)

This document defines the migration steps needed for full behavioral parity of `--find-offset` against the C implementation (`search_for_drive_offset` / `search_for_offset`).

## Scope

Target command path:
- C: `src/cyanrip_main.c` (`search_for_drive_offset`)
- Rust: `src/app.rs` (`run_find_offset_mode`)

## Parity Steps

1. Mode side-effects parity
- [x] Force offset baseline semantics for find-offset mode output/reporting.
- [x] Keep AccurateRip required and MusicBrainz/CoverArt disabled semantics in mode report.

2. Drive TOC + AccurateRip acquisition parity
- [x] Read physical drive TOC using libcdio in linux feature build.
- [x] Build DiscID/CDDB and AccurateRip IDs from runtime TOC.
- [x] Query AccurateRip and surface request URL/status in runtime report.

3. Offset probing parity core
- [x] Probe around track start +450 frames.
- [x] Scan sample shifts in 4-byte steps and compute ARv1 checksum over frame window.
- [x] Search preferred direction first, then reverse direction.

4. Multi-track decision parity
- [x] Keep scanning tracks after first hit.
- [x] Confirm same offset across additional tracks by increasing confidence.
- [x] If a different offset appears, scrap old candidate and restart confidence at 1.
- [x] Emit C-style progress lines for found/confirmed/replaced candidates.

5. Radius escalation parity
- [x] Retry with doubled radius when entries exist and eligible tracks were checked but no offset was found.
- [x] Emit radius escalation message.
- [~] C has no explicit hard cap; Rust keeps a defensive cap (`FIND_OFFSET_MAX_RADIUS_FRAMES`) to avoid unbounded probing.

6. Terminal outcomes parity
- [x] Distinguish: no AccurateRip entries, no long-enough tracks, found offset with confidence, and exhausted radius.

7. Remaining parity hardening
- [ ] Track-type parity for mixed-mode discs (`track_is_data`) from drive TOC, not audio-only assumption.
- [ ] Differential tests against C binary for find-offset on shared fixtures/hardware logs.
- [ ] Optional quit/interruption parity in the probing loop.

## Implementation Notes

Implemented in this repository:
- `src/cdda/linux_drive.rs`: physical TOC extraction helper for runtime track boundaries.
- `src/app.rs`: `run_find_offset_mode` now performs real TOC + AccurateRip + probing workflow.

## Validation Commands

Default build:
- `cargo build`

Feature build for real find-offset path:
- `cargo build --features "backend-libcdio-sys paranoia"`

Focused CLI check:
- `cargo test --features "backend-libcdio-sys paranoia" --test run_workflow_cli find_offset_mode_returns_success_with_report`

Manual runtime check (requires audio CD inserted):
- `cargo run --features "backend-libcdio-sys paranoia" -- -f -o flac -d /dev/cdrom`
