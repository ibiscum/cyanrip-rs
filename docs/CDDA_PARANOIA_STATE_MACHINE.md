# CDDA Paranoia Rip State Machine

This document captures the parity-oriented state machine for paranoia-mode ripping.
It is derived from src/cyanrip_main.c, especially:

- status callback accounting in status_cb + cyanrip_read_frame
- repeat-rip loop in cyanrip_rip_track
- media-change and quit abort checks in cyanrip_rip_track and main track loops
- flush/finalize sequence in cyanrip_rip_track

## Goal

Provide one deterministic control-path model that both image-backed and physical-drive backends can use.
This separates policy from transport so reliability behavior is testable in CI.

## Rust Location

- State machine and retry policy: src/cdda/paranoia.rs

## States

- Idle: no active track processing
- Reading: frame read/checksum/decode loop is running
- RetryPending: checksum did not reach required match count; another pass is planned
- Finalizing: flush encoders and finalize track metadata/checksums
- TrackComplete: track finished successfully
- Aborted: rip canceled due to quit request or media change
- Failed: unrecoverable decode/encode pipeline error

## Events

- StartTrack
- FrameReadOk
- FrameReadError
- ChecksumMismatch
- RetryReady
- RetryLimitReached
- ChecksumSatisfied
- FlushEncoders
- EncoderFlushDone
- QuitRequested
- MediaChanged
- FatalDecodeOrEncodeError

## Transition Summary

- Idle + StartTrack -> Reading
- Reading + FrameReadOk/FrameReadError -> Reading
- Reading + ChecksumMismatch -> RetryPending
- RetryPending + RetryReady -> Reading
- RetryPending + RetryLimitReached -> Finalizing
- Reading + ChecksumSatisfied -> Finalizing
- Finalizing + EncoderFlushDone -> TrackComplete
- Any active state + QuitRequested/MediaChanged -> Aborted
- Any active state + FatalDecodeOrEncodeError -> Failed

## Retry Policy (Parity Notes)

The retry policy mirrors upstream semantics used by repeated-rip mode:

- required_matches = ripping_retries
- total attempts capped by max_retries
- checksum match count is computed against prior attempts
- if next pass is potentially final, start encode path for that attempt

This behavior is modeled by RetryPolicy::on_checksum in src/cdda/paranoia.rs.

## Regression Tests

Current coverage in src/cdda/paranoia.rs:

- paranoia level to mode mapping
- retry threshold completion behavior
- retry-limit stop behavior when checksums do not converge
- happy-path completion through Finalizing -> TrackComplete
- retry transition flow
- media-change and quit abort transitions
- fatal decode/encode failure transition

## Migration Wiring Steps

1. Introduce CDDA frame reader traits that emit state-machine events.
2. Implement image-backed reader for deterministic CI replay and fault injection.
3. Add Linux physical-reader backend and map runtime callbacks into the same events.
4. Add differential regression tests vs cyanrip for selected damaged-disc scenarios.
