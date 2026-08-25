# FLAC-Only HDCD Integration Plan (No FIFO / No Threads)

## Scope

- Encoder scope: FLAC only.
- Runtime model: synchronous single-threaded processing, no producer/consumer FIFOs.
- Option scope:
  - `-H/--hdcd`
  - `-E/--force-deemphasis`
  - `-W/--no-deemphasis`

This plan follows the upstream behavior notes in `docs/hdcd-option-flow.md`, adapted to this Rust codebase architecture.

## Current baseline in cyanrip-rs

- CLI options already parse into `Settings`.
- Runtime reporting already prints `HDCD decoding: enabled/disabled`.
- Audio write path was direct PCM -> FLAC/WAV with no processing stage hook.

## Target architecture (single-threaded)

Per track:

1. Acquire PCM.
2. Run processing stage in-process (same thread):
   - HDCD decode (when available),
   - De-emphasis according to options/track flags.
3. Write FLAC file.
4. Embed tags.

No queue handoff, no background encoder thread, no packet staging.

## Data model changes

## Phase 1 (implemented)

- Keep `PcmTrackData` as 16-bit interleaved PCM.
- Add processing options model:
  - `TrackProcessingOptions` in `src/audio/process.rs`.
- Add explicit processing error model:
  - `TrackProcessingError`.

## Phase 2 (next)

- Extend PCM representation to support post-HDCD higher precision:
  - either enum sample storage (`I16` / `I32`) or canonical `Vec<i32>` + bit-depth metadata.
- Update FLAC writer path to support 24-bit encode output where HDCD expands effective depth.

## Processing stage API

Implemented API in `src/audio/process.rs`:

- `process_track_pcm(input, options) -> Result<PcmTrackData, TrackProcessingError>`

Behavior:

- `decode_hdcd=true`:
  - returns explicit `HdcdDecodeUnavailable` until backend is wired.
- De-emphasis decision:
  - apply when `force_deemphasis` is true, or
  - when `deemphasis` is enabled and track metadata marks pre-emphasis.
- De-emphasis implementation:
  - in-process first-order IIR based on CD 50/15us constants.

## Track metadata behavior

- Added `preemphasis` track metadata parsing (`1/true/yes/on`) to trigger automatic de-emphasis path.

## Metadata parity behaviors

- Align `media` metadata default with upstream option semantics:
  - `HDCD` when `decode_hdcd` is requested,
  - `CD` otherwise.

## Test matrix

## Unit tests

- `src/audio/process.rs`:
  - passthrough when no processing applies,
  - explicit failure when HDCD requested but backend unavailable,
  - force-deemphasis modifies samples.

## Integration tests

- `src/app.rs` dispatch tests:
  - FLAC dispatch returns `Processing` error when `decode_hdcd=true` (current Phase 1 behavior).

## Future integration tests (Phase 2/3)

- HDCD fixture decode regression:
  - known HDCD input produces expected sample/bit-depth deltas.
- FLAC bit-depth verification:
  - resulting FLAC streaminfo reflects 24-bit path when HDCD active.
- Option interaction table:
  - `-H`, `-E`, `-W` combinations across pre-emphasis flagged/non-flagged tracks.

## Implementation status

- Implemented now:
  - processing stage API hook,
  - de-emphasis processing path,
  - explicit HDCD unsupported error,
  - HDCD-aware `media` metadata default.
- Deferred until backend choice is finalized:
  - true HDCD decode and 24-bit propagation.

## Backend decision checkpoint for true HDCD decode

Before Phase 2/3 implementation, decide one of:

1. Bind FFmpeg filter path (`hdcd`) and map samples into Rust pipeline.
2. Integrate a native Rust HDCD decoder implementation.
3. Provide external-process bridge (least preferred for determinism and testability).

Once selected, replace `HdcdDecodeUnavailable` with real decode path while preserving the same processing API.
