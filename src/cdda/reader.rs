use std::collections::HashSet;

use crate::cdda::paranoia::{RipEvent, RipState, RetryDecision, RetryPolicy, next_rip_state};

pub const CDDA_FRAME_BYTES: usize = 2352;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CddaReadError {
    SeekFailed(String),
    ReadFailed(String),
}

pub trait CddaFrameReader {
    fn seek_frame(&mut self, lsn: i32) -> Result<(), CddaReadError>;
    fn read_frame(&mut self) -> Result<Vec<u8>, CddaReadError>;
    fn media_changed(&self) -> bool;

    /// Callback counters recorded by a real paranoia engine during the most recent
    /// `read_frame` call, if this reader is backed by one. Software-only readers
    /// (fault-injection, raw sector readers) use the default of `None`.
    fn take_native_callback_counts(&mut self) -> Option<ParanoiaCallbackCounters> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultInjectedImageReader {
    frames: Vec<Vec<u8>>,
    cursor: usize,
    read_attempt: usize,
    fail_on_attempts: HashSet<usize>,
    media_changed_at_attempt: Option<usize>,
}

impl FaultInjectedImageReader {
    pub fn new(frames: Vec<Vec<u8>>) -> Self {
        Self {
            frames,
            cursor: 0,
            read_attempt: 0,
            fail_on_attempts: HashSet::new(),
            media_changed_at_attempt: None,
        }
    }

    pub fn with_fail_on_attempts(mut self, attempts: &[usize]) -> Self {
        self.fail_on_attempts = attempts.iter().copied().collect();
        self
    }

    pub fn with_media_change_at_attempt(mut self, attempt: usize) -> Self {
        self.media_changed_at_attempt = Some(attempt);
        self
    }
}

impl CddaFrameReader for FaultInjectedImageReader {
    fn seek_frame(&mut self, lsn: i32) -> Result<(), CddaReadError> {
        if lsn < 0 {
            return Err(CddaReadError::SeekFailed("negative lsn".to_string()));
        }

        let next = lsn as usize;
        if next > self.frames.len() {
            return Err(CddaReadError::SeekFailed(format!(
                "lsn {lsn} beyond frame count {}",
                self.frames.len()
            )));
        }

        self.cursor = next;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, CddaReadError> {
        self.read_attempt = self.read_attempt.saturating_add(1);

        if self.fail_on_attempts.contains(&self.read_attempt) {
            return Err(CddaReadError::ReadFailed(format!(
                "injected read failure at attempt {}",
                self.read_attempt
            )));
        }

        let frame = self
            .frames
            .get(self.cursor)
            .ok_or_else(|| CddaReadError::ReadFailed("end of image".to_string()))?
            .clone();
        self.cursor = self.cursor.saturating_add(1);
        Ok(frame)
    }

    fn media_changed(&self) -> bool {
        match self.media_changed_at_attempt {
            Some(attempt) => self.read_attempt >= attempt,
            None => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParanoiaTrackRunResult {
    pub state: RipState,
    pub events: Vec<RipEvent>,
    pub passes: u32,
    pub callback_counters: ParanoiaCallbackCounters,
    pub final_frames: Option<Vec<Vec<u8>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ParanoiaCallbackKind {
    Read = 0,
    Verify = 1,
    FixupEdge = 2,
    FixupAtom = 3,
    Scratch = 4,
    Repair = 5,
    Skip = 6,
    Drift = 7,
    Backoff = 8,
    Overlap = 9,
    FixupDropped = 10,
    FixupDuped = 11,
    ReadErr = 12,
    CacheErr = 13,
    Wrote = 14,
    Finished = 15,
}

pub const PARANOIA_CALLBACK_KIND_COUNT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParanoiaCallbackCounters {
    counts: [u64; PARANOIA_CALLBACK_KIND_COUNT],
}

impl Default for ParanoiaCallbackCounters {
    fn default() -> Self {
        Self {
            counts: [0u64; PARANOIA_CALLBACK_KIND_COUNT],
        }
    }
}

impl ParanoiaCallbackCounters {
    pub fn increment(&mut self, kind: ParanoiaCallbackKind) {
        let idx = kind as usize;
        self.counts[idx] = self.counts[idx].saturating_add(1);
    }

    pub fn get(&self, kind: ParanoiaCallbackKind) -> u64 {
        self.counts[kind as usize]
    }

    pub fn merge(&mut self, other: &ParanoiaCallbackCounters) {
        for idx in 0..PARANOIA_CALLBACK_KIND_COUNT {
            self.counts[idx] = self.counts[idx].saturating_add(other.counts[idx]);
        }
    }
}

impl ParanoiaCallbackKind {
    /// Maps a raw `paranoia_cb_mode_t` value from libcdio's paranoia engine to
    /// this enum. The discriminants are defined to match libcdio's numbering 1:1.
    pub fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Read),
            1 => Some(Self::Verify),
            2 => Some(Self::FixupEdge),
            3 => Some(Self::FixupAtom),
            4 => Some(Self::Scratch),
            5 => Some(Self::Repair),
            6 => Some(Self::Skip),
            7 => Some(Self::Drift),
            8 => Some(Self::Backoff),
            9 => Some(Self::Overlap),
            10 => Some(Self::FixupDropped),
            11 => Some(Self::FixupDuped),
            12 => Some(Self::ReadErr),
            13 => Some(Self::CacheErr),
            14 => Some(Self::Wrote),
            15 => Some(Self::Finished),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParanoiaHeuristicConfig {
    pub overlap_frames: usize,
    pub verify_overlap: bool,
}

/// Hook point matching upstream's edge-byte trimming step between the raw frame
/// read and checksum/encode accumulation (see `cyanrip_read_frame`). Byte-level
/// offset/overread boundary trimming for this track is computed by the caller
/// before the read starts (see `apply_offset_frame_adjustment` in src/app.rs),
/// so this is currently a pass-through kept at the same pipeline position.
fn apply_offset_edge_trim(_frame_idx: usize, _frame_count: usize, frame: Vec<u8>) -> Vec<u8> {
    frame
}

fn overlap_signature(frames: &[Vec<u8>], overlap_frames: usize) -> Option<(u32, u32)> {
    if overlap_frames == 0 || frames.is_empty() {
        return None;
    }

    let edge = overlap_frames.min(frames.len());

    let mut head = 2166136261u32;
    for frame in &frames[..edge] {
        for byte in frame {
            head ^= *byte as u32;
            head = head.wrapping_mul(16777619);
        }
    }

    let mut tail = 2166136261u32;
    for frame in &frames[(frames.len() - edge)..] {
        for byte in frame {
            tail ^= *byte as u32;
            tail = tail.wrapping_mul(16777619);
        }
    }

    Some((head, tail))
}

pub fn run_track_with_paranoia_heuristics<R, F>(
    reader: &mut R,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    heuristics: ParanoiaHeuristicConfig,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    R: CddaFrameReader,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_track_with_paranoia_heuristics_interruptible(
        reader,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        heuristics,
        || false,
        checksum_fn,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_track_with_paranoia_heuristics_interruptible<R, F, I>(
    reader: &mut R,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    heuristics: ParanoiaHeuristicConfig,
    mut should_interrupt: I,
    mut checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    R: CddaFrameReader,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
    I: FnMut() -> bool,
{
    let mut state = RipState::Idle;
    let mut events = vec![RipEvent::StartTrack];
    state = next_rip_state(state, RipEvent::StartTrack);

    let mut counters = ParanoiaCallbackCounters::default();
    let mut pass = 0u32;
    let mut previous_overlap = None;

    loop {
        if should_interrupt() {
            counters.increment(ParanoiaCallbackKind::Skip);
            events.push(RipEvent::QuitRequested);
            state = next_rip_state(state, RipEvent::QuitRequested);
            return Ok(ParanoiaTrackRunResult {
                state,
                events,
                passes: pass,
                callback_counters: counters,
                final_frames: None,
            });
        }

        reader.seek_frame(start_lsn)?;
        let mut pass_frames = Vec::with_capacity(frame_count);

        for frame_idx in 0..frame_count {
            if should_interrupt() {
                counters.increment(ParanoiaCallbackKind::Skip);
                events.push(RipEvent::QuitRequested);
                state = next_rip_state(state, RipEvent::QuitRequested);
                return Ok(ParanoiaTrackRunResult {
                    state,
                    events,
                    passes: pass.saturating_add(1),
                    callback_counters: counters,
                    final_frames: None,
                });
            }

            if reader.media_changed() {
                counters.increment(ParanoiaCallbackKind::Skip);
                events.push(RipEvent::MediaChanged);
                state = next_rip_state(state, RipEvent::MediaChanged);
                return Ok(ParanoiaTrackRunResult {
                    state,
                    events,
                    passes: pass.saturating_add(1),
                    callback_counters: counters,
                    final_frames: None,
                });
            }

            let mut success = None;
            for attempt in 0..=max_frame_retries {
                if should_interrupt() {
                    counters.increment(ParanoiaCallbackKind::Skip);
                    events.push(RipEvent::QuitRequested);
                    state = next_rip_state(state, RipEvent::QuitRequested);
                    return Ok(ParanoiaTrackRunResult {
                        state,
                        events,
                        passes: pass.saturating_add(1),
                        callback_counters: counters,
                        final_frames: None,
                    });
                }

                counters.increment(ParanoiaCallbackKind::Read);
                let read_result = reader.read_frame();
                if let Some(native) = reader.take_native_callback_counts() {
                    counters.merge(&native);
                }
                match read_result {
                    Ok(frame) => {
                        events.push(RipEvent::FrameReadOk);
                        state = next_rip_state(state, RipEvent::FrameReadOk);
                        success = Some(frame);
                        break;
                    }
                    Err(_err) => {
                        counters.increment(ParanoiaCallbackKind::ReadErr);
                        counters.increment(ParanoiaCallbackKind::Backoff);
                        events.push(RipEvent::FrameReadError);
                        state = next_rip_state(state, RipEvent::FrameReadError);
                        if reader.media_changed() {
                            counters.increment(ParanoiaCallbackKind::Skip);
                            events.push(RipEvent::MediaChanged);
                            state = next_rip_state(state, RipEvent::MediaChanged);
                            return Ok(ParanoiaTrackRunResult {
                                state,
                                events,
                                passes: pass.saturating_add(1),
                                callback_counters: counters,
                                final_frames: None,
                            });
                        }

                        if attempt >= max_frame_retries {
                            // Matches upstream cyanrip_read_frame: an unrecoverable frame
                            // does not fail the track, it is replaced with silence.
                            events.push(RipEvent::FrameSubstitutedSilence);
                            state = next_rip_state(state, RipEvent::FrameSubstitutedSilence);
                            success = Some(vec![0u8; CDDA_FRAME_BYTES]);
                            break;
                        }
                    }
                }
            }

            let Some(frame) = success else {
                events.push(RipEvent::FatalDecodeOrEncodeError);
                return Err(CddaReadError::ReadFailed(
                    "frame retries exhausted".to_string(),
                ));
            };

            // Upstream applies offset/overread edge-byte trimming here, between the raw
            // frame read and checksum/encode accumulation. Boundary trimming for this
            // track is computed by the caller before the read starts (see
            // apply_offset_frame_adjustment in src/app.rs), so this is a pass-through
            // seam kept structurally in the same place as upstream's cyanrip_rip_track.
            let frame = apply_offset_edge_trim(frame_idx, frame_count, frame);
            pass_frames.push(frame);
        }

        let checksum = checksum_fn(pass, &pass_frames);
        let mut overlap_mismatch = false;

        if heuristics.verify_overlap {
            counters.increment(ParanoiaCallbackKind::Verify);
            if let Some(sig) = overlap_signature(&pass_frames, heuristics.overlap_frames) {
                if let Some(prev) = previous_overlap {
                    counters.increment(ParanoiaCallbackKind::Overlap);
                    if prev != sig {
                        counters.increment(ParanoiaCallbackKind::Drift);
                        overlap_mismatch = true;
                    }
                }
                previous_overlap = Some(sig);
            }
        }

        let retry_checksum = if overlap_mismatch {
            checksum.wrapping_add(pass.saturating_add(1))
        } else {
            checksum
        };

        let matches = retry_policy
            .prior_checksums
            .iter()
            .filter(|prior| **prior == retry_checksum)
            .count() as u32;
        let expected_total_attempts = retry_policy.total_attempts.saturating_add(1);
        let would_hit_retry_limit = retry_policy.required_matches > 0
            && expected_total_attempts >= retry_policy.max_retries
            && matches < retry_policy.required_matches;

        let decision = retry_policy.on_checksum(retry_checksum);
        pass = pass.saturating_add(1);

        match decision {
            RetryDecision::Complete => {
                if would_hit_retry_limit {
                    counters.increment(ParanoiaCallbackKind::Repair);
                    events.push(RipEvent::ChecksumMismatch);
                    state = next_rip_state(state, RipEvent::ChecksumMismatch);
                    events.push(RipEvent::RetryLimitReached);
                    state = next_rip_state(state, RipEvent::RetryLimitReached);
                } else {
                    events.push(RipEvent::ChecksumSatisfied);
                    state = next_rip_state(state, RipEvent::ChecksumSatisfied);
                }

                events.push(RipEvent::FlushEncoders);
                state = next_rip_state(state, RipEvent::FlushEncoders);
                counters.increment(ParanoiaCallbackKind::Wrote);
                events.push(RipEvent::EncoderFlushDone);
                state = next_rip_state(state, RipEvent::EncoderFlushDone);
                counters.increment(ParanoiaCallbackKind::Finished);

                return Ok(ParanoiaTrackRunResult {
                    state,
                    events,
                    passes: pass,
                    callback_counters: counters,
                    final_frames: Some(pass_frames),
                });
            }
            RetryDecision::RetryNow | RetryDecision::RetryAndStartEncoding => {
                counters.increment(ParanoiaCallbackKind::Repair);
                events.push(RipEvent::ChecksumMismatch);
                state = next_rip_state(state, RipEvent::ChecksumMismatch);
                events.push(RipEvent::RetryReady);
                state = next_rip_state(state, RipEvent::RetryReady);
            }
        }
    }
}

pub fn run_track_with_paranoia<R, F>(
    reader: &mut R,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    R: CddaFrameReader,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_track_with_paranoia_heuristics_interruptible(
        reader,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        ParanoiaHeuristicConfig::default(),
        || false,
        checksum_fn,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct PassVariantReader {
        frames_per_pass: Vec<Vec<Vec<u8>>>,
        pass: usize,
        cursor: usize,
    }

    impl PassVariantReader {
        fn new(frames_per_pass: Vec<Vec<Vec<u8>>>) -> Self {
            Self {
                frames_per_pass,
                pass: 0,
                cursor: 0,
            }
        }
    }

    impl CddaFrameReader for PassVariantReader {
        fn seek_frame(&mut self, lsn: i32) -> Result<(), CddaReadError> {
            if lsn != 0 {
                return Err(CddaReadError::SeekFailed("test reader supports lsn=0 only".to_string()));
            }
            self.cursor = 0;
            Ok(())
        }

        fn read_frame(&mut self) -> Result<Vec<u8>, CddaReadError> {
            let pass_idx = self.pass.min(self.frames_per_pass.len().saturating_sub(1));
            let frames = &self.frames_per_pass[pass_idx];
            let frame = frames
                .get(self.cursor)
                .ok_or_else(|| CddaReadError::ReadFailed("end of test pass".to_string()))?
                .clone();
            self.cursor = self.cursor.saturating_add(1);
            if self.cursor >= frames.len() {
                self.pass = self.pass.saturating_add(1);
            }
            Ok(frame)
        }

        fn media_changed(&self) -> bool {
            false
        }
    }

    fn sample_frame(seed: u8) -> Vec<u8> {
        vec![seed; CDDA_FRAME_BYTES]
    }

    fn sample_frames() -> Vec<Vec<u8>> {
        vec![sample_frame(1), sample_frame(2), sample_frame(3)]
    }

    #[test]
    fn fake_image_reader_reads_frames_in_order() {
        let mut r = FaultInjectedImageReader::new(sample_frames());
        r.seek_frame(0).expect("seek should work");

        assert_eq!(r.read_frame().expect("frame 1"), sample_frame(1));
        assert_eq!(r.read_frame().expect("frame 2"), sample_frame(2));
        assert_eq!(r.read_frame().expect("frame 3"), sample_frame(3));
    }

    #[test]
    fn run_track_emits_retry_events_until_checksum_converges() {
        let mut r = FaultInjectedImageReader::new(sample_frames());
        let mut policy = RetryPolicy::new(1, 4);

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |pass, _| match pass {
            0 => 100,
            1 => 200,
            _ => 200,
        })
        .expect("track run should succeed");

        assert_eq!(out.state, RipState::TrackComplete);
        assert_eq!(out.passes, 3);
        assert!(out.events.contains(&RipEvent::ChecksumMismatch));
        assert!(out.events.contains(&RipEvent::RetryReady));
        assert!(out.events.contains(&RipEvent::ChecksumSatisfied));
        assert_eq!(out.callback_counters.get(ParanoiaCallbackKind::Finished), 1);
        assert_eq!(out.final_frames.as_ref().map(Vec::len), Some(3));
    }

    #[test]
    fn run_track_handles_injected_read_faults_with_frame_retries() {
        let mut r = FaultInjectedImageReader::new(sample_frames()).with_fail_on_attempts(&[1, 4]);
        let mut policy = RetryPolicy::disabled();

        let out = run_track_with_paranoia(&mut r, 0, 3, 2, &mut policy, |_pass, _| 42)
            .expect("faults should be absorbed by retries");

        assert_eq!(out.state, RipState::TrackComplete);
        assert!(out.events.contains(&RipEvent::FrameReadError));
        assert!(out.events.contains(&RipEvent::FrameReadOk));
        assert!(out.callback_counters.get(ParanoiaCallbackKind::ReadErr) >= 1);
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Backoff) >= 1);
    }

    #[test]
    fn run_track_substitutes_silence_when_frame_retries_are_exhausted() {
        // Frames 0 and 1 succeed on their first read (calls 1 and 2), then frame 2's
        // two read attempts land on calls 3 and 4; both fail, so retries are
        // exhausted for that frame alone (upstream: substitute silence and continue
        // instead of failing the whole track).
        let mut r = FaultInjectedImageReader::new(sample_frames()).with_fail_on_attempts(&[3, 4]);
        let mut policy = RetryPolicy::disabled();

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |_pass, _| 7)
            .expect("unrecoverable frame should not fail the track");

        assert_eq!(out.state, RipState::TrackComplete);
        assert!(out.events.contains(&RipEvent::FrameSubstitutedSilence));
        assert!(!out.events.contains(&RipEvent::FatalDecodeOrEncodeError));
        let frames = out.final_frames.expect("track should still finalize frames");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[2], vec![0u8; CDDA_FRAME_BYTES]);
        assert_ne!(frames[0], vec![0u8; CDDA_FRAME_BYTES]);
        assert_ne!(frames[1], vec![0u8; CDDA_FRAME_BYTES]);
    }

    #[test]
    fn run_track_aborts_when_media_changes() {
        let mut r = FaultInjectedImageReader::new(sample_frames()).with_media_change_at_attempt(2);
        let mut policy = RetryPolicy::disabled();

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |_pass, _| 1)
            .expect("media change should be reported as aborted result");

        assert_eq!(out.state, RipState::Aborted);
        assert!(out.events.contains(&RipEvent::MediaChanged));
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Skip) >= 1);
        assert!(out.final_frames.is_none());
    }

    #[test]
    fn run_track_emits_retry_limit_reached_when_checksums_do_not_converge() {
        let mut r = FaultInjectedImageReader::new(sample_frames());
        let mut policy = RetryPolicy::new(3, 2);

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |pass, _| pass + 1)
            .expect("run should finalize after retry limit");

        assert_eq!(out.state, RipState::TrackComplete);
        assert!(out.events.contains(&RipEvent::RetryLimitReached));
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Repair) >= 1);
    }

    #[test]
    fn overlap_verify_heuristics_force_retry_when_edges_drift() {
        let pass1 = vec![sample_frame(1), sample_frame(2), sample_frame(3)];
        let mut pass2 = pass1.clone();
        pass2[0][0] = 9;

        let mut r = PassVariantReader::new(vec![pass1, pass2]);
        let mut policy = RetryPolicy::new(1, 2);

        let out = run_track_with_paranoia_heuristics(
            &mut r,
            0,
            3,
            0,
            &mut policy,
            ParanoiaHeuristicConfig {
                overlap_frames: 1,
                verify_overlap: true,
            },
            |_pass, _| 0xDEAD_BEEF,
        )
        .expect("run should finalize at retry limit after overlap drift");

        assert_eq!(out.state, RipState::TrackComplete);
        assert!(out.events.contains(&RipEvent::RetryLimitReached));
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Verify) >= 1);
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Overlap) >= 1);
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Drift) >= 1);
    }

    #[test]
    fn run_track_aborts_when_interrupted() {
        let mut r = FaultInjectedImageReader::new(sample_frames());
        let mut policy = RetryPolicy::disabled();
        let mut checks = 0usize;

        let out = run_track_with_paranoia_heuristics_interruptible(
            &mut r,
            0,
            3,
            1,
            &mut policy,
            ParanoiaHeuristicConfig::default(),
            || {
                checks = checks.saturating_add(1);
                checks >= 2
            },
            |_pass, _| 0,
        )
        .expect("interrupt should return aborted result");

        assert_eq!(out.state, RipState::Aborted);
        assert!(out.events.contains(&RipEvent::QuitRequested));
        assert!(out.callback_counters.get(ParanoiaCallbackKind::Skip) >= 1);
    }
}
