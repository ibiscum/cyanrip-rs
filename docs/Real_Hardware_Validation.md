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
  - `TrackComplete` with `FrameReadOk` + flush events.
  - An `Aborted`/`MediaChanged` result on untouched media is a failure, not an
    acceptable alternative outcome (see 2026-08-30 fix note below): it means
    the drive returned a media-changed query error that is being misread as
    an actual media change.

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

## Fix Note (2026-08-30): False-Positive MediaChanged Aborts

The "Paranoia run (normal media)" scenario above previously listed
`Aborted`/`MediaChanged` as an acceptable outcome even without any real media
change. That was masking a bug rather than documenting expected behavior:
`media_changed_from_code` in `src/cdda/linux_drive.rs` treated any
`cdio_get_media_changed` return code other than `0` and the specific
`DRIVER_OP_UNSUPPORTED` (-2) sentinel as a real media change. Some drive/kernel
combinations return other negative `driver_return_code_t` errors (e.g.
`DRIVER_OP_ERROR = -1`) when the media-changed query itself fails or isn't
fully supported, which was misinterpreted as the disc having been swapped and
aborted in-progress rips with zero finalized frames, regardless of the
configured retry limit.

Fixed by changing `media_changed_from_code` to treat only a strictly positive
code (in practice `1`) as a real change; `0` and any negative/error code are
now treated as "unchanged" (fail-open). Regression coverage:
`media_changed_treats_any_negative_driver_error_as_not_changed` in
`src/cdda/linux_drive.rs`.

Re-running scenario 3 after this fix should reliably produce `TrackComplete`
on untouched media. If `Aborted`/`MediaChanged` is still observed without an
actual media change, treat it as a regression, not an expected alternative.
