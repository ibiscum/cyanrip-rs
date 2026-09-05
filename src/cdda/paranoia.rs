use crate::MAX_PARANOIA_LEVEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParanoiaMode {
    Disable,
    Overlap,
    OverlapVerify,
    FullXorNeverSkip,
}

pub fn paranoia_mode_from_level(level: i32) -> Result<ParanoiaMode, String> {
    match level {
        0 => Ok(ParanoiaMode::Disable),
        1 => Ok(ParanoiaMode::Overlap),
        2 => Ok(ParanoiaMode::OverlapVerify),
        3 => Ok(ParanoiaMode::FullXorNeverSkip),
        _ => Err(format!(
            "Invalid paranoia level {level} must be between 0 and {MAX_PARANOIA_LEVEL}"
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RipState {
    Idle,
    Reading,
    Finalizing,
    TrackComplete,
    RetryPending,
    Aborted,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RipEvent {
    StartTrack,
    FrameReadOk,
    FrameReadError,
    /// Frame retries exhausted; a silent frame was substituted and the pass continues
    /// (matches upstream cyanrip's cyanrip_read_frame behavior, not a track failure).
    FrameSubstitutedSilence,
    ChecksumSatisfied,
    ChecksumMismatch,
    RetryReady,
    RetryLimitReached,
    FlushEncoders,
    EncoderFlushDone,
    QuitRequested,
    MediaChanged,
    FatalDecodeOrEncodeError,
}

pub fn next_rip_state(state: RipState, event: RipEvent) -> RipState {
    match event {
        RipEvent::QuitRequested | RipEvent::MediaChanged => return RipState::Aborted,
        RipEvent::FatalDecodeOrEncodeError => return RipState::Failed,
        _ => {}
    }

    match (state, event) {
        (RipState::Idle, RipEvent::StartTrack) => RipState::Reading,
        (RipState::Reading, RipEvent::FrameReadOk) => RipState::Reading,
        (RipState::Reading, RipEvent::FrameReadError) => RipState::Reading,
        (RipState::Reading, RipEvent::FrameSubstitutedSilence) => RipState::Reading,
        (RipState::Reading, RipEvent::ChecksumMismatch) => RipState::RetryPending,
        (RipState::RetryPending, RipEvent::RetryReady) => RipState::Reading,
        (RipState::RetryPending, RipEvent::RetryLimitReached) => RipState::Finalizing,
        (RipState::Reading, RipEvent::ChecksumSatisfied) => RipState::Finalizing,
        (RipState::Finalizing, RipEvent::FlushEncoders) => RipState::Finalizing,
        (RipState::Finalizing, RipEvent::EncoderFlushDone) => RipState::TrackComplete,
        (s, _) => s,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub required_matches: u32,
    pub max_retries: u32,
    pub prior_checksums: Vec<u32>,
    pub total_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    Complete,
    RetryNow,
    RetryAndStartEncoding,
}

impl RetryPolicy {
    pub fn new(required_matches: u32, max_retries: u32) -> Self {
        Self {
            required_matches,
            max_retries,
            prior_checksums: Vec::new(),
            total_attempts: 0,
        }
    }

    pub fn disabled() -> Self {
        Self::new(0, 0)
    }

    pub fn on_checksum(&mut self, checksum: u32) -> RetryDecision {
        if self.required_matches == 0 {
            return RetryDecision::Complete;
        }

        let mut matches = 0u32;
        for prior in &self.prior_checksums {
            if *prior == checksum {
                matches += 1;
            }
        }

        self.total_attempts = self.total_attempts.saturating_add(1);
        if matches >= self.required_matches {
            return RetryDecision::Complete;
        }
        if self.total_attempts >= self.max_retries {
            return RetryDecision::Complete;
        }

        self.prior_checksums.push(checksum);

        let last_chance =
            (matches + 1) >= self.required_matches || (self.total_attempts + 1) >= self.max_retries;
        if last_chance {
            RetryDecision::RetryAndStartEncoding
        } else {
            RetryDecision::RetryNow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_paranoia_level_to_mode_like_upstream() {
        assert_eq!(paranoia_mode_from_level(0).unwrap(), ParanoiaMode::Disable);
        assert_eq!(paranoia_mode_from_level(1).unwrap(), ParanoiaMode::Overlap);
        assert_eq!(
            paranoia_mode_from_level(2).unwrap(),
            ParanoiaMode::OverlapVerify
        );
        assert_eq!(
            paranoia_mode_from_level(3).unwrap(),
            ParanoiaMode::FullXorNeverSkip
        );
        assert!(paranoia_mode_from_level(4).is_err());
    }

    #[test]
    fn retries_until_checksum_match_threshold() {
        let mut p = RetryPolicy::new(2, 10);

        assert_eq!(p.on_checksum(0x1111_1111), RetryDecision::RetryNow);
        assert_eq!(
            p.on_checksum(0x1111_1111),
            RetryDecision::RetryAndStartEncoding
        );
        assert_eq!(p.on_checksum(0x1111_1111), RetryDecision::Complete);
    }

    #[test]
    fn stops_on_retry_limit_when_no_matches() {
        let mut p = RetryPolicy::new(3, 2);

        assert_eq!(
            p.on_checksum(0xAAAA_0001),
            RetryDecision::RetryAndStartEncoding
        );
        assert_eq!(p.on_checksum(0xBBBB_0002), RetryDecision::Complete);
    }

    #[test]
    fn state_machine_reaches_track_complete_happy_path() {
        let s0 = RipState::Idle;
        let s1 = next_rip_state(s0, RipEvent::StartTrack);
        let s2 = next_rip_state(s1, RipEvent::FrameReadOk);
        let s3 = next_rip_state(s2, RipEvent::ChecksumSatisfied);
        let s4 = next_rip_state(s3, RipEvent::FlushEncoders);
        let s5 = next_rip_state(s4, RipEvent::EncoderFlushDone);

        assert_eq!(s1, RipState::Reading);
        assert_eq!(s2, RipState::Reading);
        assert_eq!(s3, RipState::Finalizing);
        assert_eq!(s4, RipState::Finalizing);
        assert_eq!(s5, RipState::TrackComplete);
    }

    #[test]
    fn state_machine_retries_then_finalizes_at_limit() {
        let s1 = next_rip_state(RipState::Reading, RipEvent::ChecksumMismatch);
        let s2 = next_rip_state(s1, RipEvent::RetryReady);
        let s3 = next_rip_state(s2, RipEvent::ChecksumMismatch);
        let s4 = next_rip_state(s3, RipEvent::RetryLimitReached);

        assert_eq!(s1, RipState::RetryPending);
        assert_eq!(s2, RipState::Reading);
        assert_eq!(s3, RipState::RetryPending);
        assert_eq!(s4, RipState::Finalizing);
    }

    #[test]
    fn state_machine_aborts_on_media_change_or_quit() {
        assert_eq!(
            next_rip_state(RipState::Reading, RipEvent::MediaChanged),
            RipState::Aborted
        );
        assert_eq!(
            next_rip_state(RipState::Finalizing, RipEvent::QuitRequested),
            RipState::Aborted
        );
    }

    #[test]
    fn state_machine_fails_on_fatal_pipeline_error() {
        assert_eq!(
            next_rip_state(RipState::Reading, RipEvent::FatalDecodeOrEncodeError),
            RipState::Failed
        );
    }
}
