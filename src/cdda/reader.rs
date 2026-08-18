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
}

pub fn run_track_with_paranoia<R, F>(
    reader: &mut R,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    mut checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    R: CddaFrameReader,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    let mut state = RipState::Idle;
    let mut events = vec![RipEvent::StartTrack];
    state = next_rip_state(state, RipEvent::StartTrack);

    let mut pass = 0u32;
    loop {
        reader.seek_frame(start_lsn)?;
        let mut pass_frames = Vec::with_capacity(frame_count);

        for _ in 0..frame_count {
            if reader.media_changed() {
                events.push(RipEvent::MediaChanged);
                state = next_rip_state(state, RipEvent::MediaChanged);
                return Ok(ParanoiaTrackRunResult {
                    state,
                    events,
                    passes: pass.saturating_add(1),
                });
            }

            let mut success = None;
            for attempt in 0..=max_frame_retries {
                match reader.read_frame() {
                    Ok(frame) => {
                        events.push(RipEvent::FrameReadOk);
                        state = next_rip_state(state, RipEvent::FrameReadOk);
                        success = Some(frame);
                        break;
                    }
                    Err(err) => {
                        events.push(RipEvent::FrameReadError);
                        state = next_rip_state(state, RipEvent::FrameReadError);
                        if reader.media_changed() {
                            events.push(RipEvent::MediaChanged);
                            state = next_rip_state(state, RipEvent::MediaChanged);
                            return Ok(ParanoiaTrackRunResult {
                                state,
                                events,
                                passes: pass.saturating_add(1),
                            });
                        }

                        if attempt >= max_frame_retries {
                            events.push(RipEvent::FatalDecodeOrEncodeError);
                            return Err(err);
                        }
                    }
                }
            }

            if let Some(frame) = success {
                pass_frames.push(frame);
            } else {
                events.push(RipEvent::FatalDecodeOrEncodeError);
                return Err(CddaReadError::ReadFailed(
                    "frame retries exhausted".to_string(),
                ));
            }
        }

        let checksum = checksum_fn(pass, &pass_frames);
        let matches = retry_policy
            .prior_checksums
            .iter()
            .filter(|prior| **prior == checksum)
            .count() as u32;
        let expected_total_attempts = retry_policy.total_attempts.saturating_add(1);
        let would_hit_retry_limit = retry_policy.required_matches > 0
            && expected_total_attempts >= retry_policy.max_retries
            && matches < retry_policy.required_matches;

        let decision = retry_policy.on_checksum(checksum);
        pass = pass.saturating_add(1);

        match decision {
            RetryDecision::Complete => {
                if would_hit_retry_limit {
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
                events.push(RipEvent::EncoderFlushDone);
                state = next_rip_state(state, RipEvent::EncoderFlushDone);

                return Ok(ParanoiaTrackRunResult {
                    state,
                    events,
                    passes: pass,
                });
            }
            RetryDecision::RetryNow | RetryDecision::RetryAndStartEncoding => {
                events.push(RipEvent::ChecksumMismatch);
                state = next_rip_state(state, RipEvent::ChecksumMismatch);
                events.push(RipEvent::RetryReady);
                state = next_rip_state(state, RipEvent::RetryReady);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn run_track_aborts_when_media_changes() {
        let mut r = FaultInjectedImageReader::new(sample_frames()).with_media_change_at_attempt(2);
        let mut policy = RetryPolicy::disabled();

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |_pass, _| 1)
            .expect("media change should be reported as aborted result");

        assert_eq!(out.state, RipState::Aborted);
        assert!(out.events.contains(&RipEvent::MediaChanged));
    }

    #[test]
    fn run_track_emits_retry_limit_reached_when_checksums_do_not_converge() {
        let mut r = FaultInjectedImageReader::new(sample_frames());
        let mut policy = RetryPolicy::new(3, 2);

        let out = run_track_with_paranoia(&mut r, 0, 3, 1, &mut policy, |pass, _| pass + 1)
            .expect("run should finalize after retry limit");

        assert_eq!(out.state, RipState::TrackComplete);
        assert!(out.events.contains(&RipEvent::RetryLimitReached));
    }
}
