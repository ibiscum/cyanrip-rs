# Real-Hardware Reliability Validation

This document defines practical, repeatable real-drive validation scenarios for M6 and records acceptance evidence.

## Preconditions

- Linux host with optical drive and read permissions.
- A readable audio CD inserted beforehand.
- Dependency stack validated via [../scripts/check_linux_cdda_stack.sh](../scripts/check_linux_cdda_stack.sh).
- Build/test features: `backend-libcdio-sys paranoia`.

## Scenario Matrix

1. TOC readability and drive open
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation reads_audio_cd_toc_from_real_drive -- --ignored`
- Expected result:
  - Test passes; TOC has at least one track and valid leadout LSN.

2. Raw frame read
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation reads_one_audio_frame_from_real_drive -- --ignored`
- Expected result:
  - Test passes; first frame length is 2352 bytes.

3. Paranoia run (normal media)
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation runs_paranoia_pipeline_on_real_drive -- --ignored`
- Expected result:
  - Either:
    - `TrackComplete` with `FrameReadOk` + flush events, or
    - `Aborted` with `MediaChanged` (drive-reported media change).

4. Interruption handling
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation interruption_request_aborts_paranoia_pipeline_on_real_drive -- --ignored`
- Expected result:
  - `Aborted` with `QuitRequested` event.

5. Manual media-change scenario (operator action)
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom CYANRIP_RUN_MANUAL_MEDIA_CHANGE=1 cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation manual_media_change_scenario_reference -- --ignored`
- Operator action:
  - Start test with readable audio CD inserted.
  - During active reads, eject/swap media.
- Expected result:
  - `Aborted` with `MediaChanged` event.

## Acceptance Notes Template

Run date:
Host:
Kernel:
Drive model:
Device path:
Disc used:

Scenario outcomes:
- TOC readability and drive open: pass/fail
- Raw frame read: pass/fail
- Paranoia run (normal media): pass/fail
- Interruption handling: pass/fail
- Manual media-change scenario: pass/fail

Observed states/events:
- TrackComplete path notes:
- Aborted path notes:
- MediaChanged frequency notes:
- Any read-error/retry behavior notes:

Conclusion:
- Practical reliability gate status: pass/fail
- Remaining risks:
- Follow-up actions:

## Initial Evidence Snapshot (2026-08-19)

Environment summary:
- Device: `/dev/cdrom`
- Features: `backend-libcdio-sys paranoia`
- Command runner: `./scripts/run_m6_hardware_validation.sh`

Observed outcomes:
- TOC readability and drive open: pass
- Raw frame read: pass
- Paranoia run (normal media): pass
- Interruption handling: pass
- Manual media-change scenario: pass

Manual scenario run details:
- Command:
  - `CYANRIP_CDROM_DEVICE=/dev/cdrom CYANRIP_RUN_MANUAL_MEDIA_CHANGE=1 cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation manual_media_change_scenario_reference -- --ignored --nocapture`
- Console prompt shown during run:
  - `ACTION REQUIRED: eject or swap media now while this test is reading.`
- Observed result:
  - `media-changed detected at iteration 0`
  - `test manual_media_change_scenario_reference ... ok`

Current gate state:
- Practical reliability validation scenarios are complete for current M6 scope.
- Automated and manual media-change scenario evidence are both recorded.
