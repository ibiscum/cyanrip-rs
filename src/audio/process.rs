use crate::audio::PcmTrackData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingPath {
    Hdcd,
    Deemphasis,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackProcessingOptions {
    pub decode_hdcd: bool,
    pub deemphasis: bool,
    pub force_deemphasis: bool,
    pub track_has_preemphasis: bool,
}

impl TrackProcessingOptions {
    pub fn should_apply_deemphasis(self) -> bool {
        self.force_deemphasis || (self.deemphasis && self.track_has_preemphasis)
    }

    pub fn selected_processing_path(self) -> ProcessingPath {
        if self.decode_hdcd {
            // Upstream behavior: hdcd filter has precedence over deemphasis.
            ProcessingPath::Hdcd
        } else if self.should_apply_deemphasis() {
            ProcessingPath::Deemphasis
        } else {
            ProcessingPath::None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackProcessingError {
    InvalidSpec(&'static str),
}

impl std::fmt::Display for TrackProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSpec(msg) => write!(f, "invalid processing input spec: {msg}"),
        }
    }
}

impl std::error::Error for TrackProcessingError {}

pub fn process_track_pcm(
    input: &PcmTrackData,
    options: TrackProcessingOptions,
) -> Result<PcmTrackData, TrackProcessingError> {
    match options.selected_processing_path() {
        ProcessingPath::Hdcd => apply_hdcd_passthrough(input),
        ProcessingPath::Deemphasis => apply_cd_deemphasis(input),
        ProcessingPath::None => Ok(input.clone()),
    }
}

fn apply_hdcd_passthrough(input: &PcmTrackData) -> Result<PcmTrackData, TrackProcessingError> {
    if input.spec.channels == 0 {
        return Err(TrackProcessingError::InvalidSpec("channels must be > 0"));
    }
    if input.spec.sample_rate == 0 {
        return Err(TrackProcessingError::InvalidSpec("sample_rate must be > 0"));
    }
    if input.spec.bits_per_sample != 16 {
        return Err(TrackProcessingError::InvalidSpec(
            "hdcd path currently supports 16-bit PCM input only",
        ));
    }

    // Keep deterministic behavior for the FLAC-only path while preserving
    // upstream option precedence: when -H is set, HDCD path is selected.
    Ok(input.clone())
}

fn apply_cd_deemphasis(input: &PcmTrackData) -> Result<PcmTrackData, TrackProcessingError> {
    if input.spec.channels == 0 {
        return Err(TrackProcessingError::InvalidSpec("channels must be > 0"));
    }
    if input.spec.sample_rate == 0 {
        return Err(TrackProcessingError::InvalidSpec("sample_rate must be > 0"));
    }
    if input.spec.bits_per_sample != 16 {
        return Err(TrackProcessingError::InvalidSpec(
            "deemphasis currently supports 16-bit PCM only",
        ));
    }

    let channels = usize::from(input.spec.channels);
    let sample_count = input.interleaved_i16_samples.len();
    if !sample_count.is_multiple_of(channels) {
        return Err(TrackProcessingError::InvalidSpec(
            "sample count must be divisible by channels",
        ));
    }

    let fs = input.spec.sample_rate as f64;
    let t1 = 50e-6f64;
    let t2 = 15e-6f64;
    let k = 2.0f64 * fs;

    let a0 = 1.0 + k * t1;
    let a1 = 1.0 - k * t1;
    let b0 = 1.0 + k * t2;
    let b1 = 1.0 - k * t2;

    let b0n = b0 / a0;
    let b1n = b1 / a0;
    let a1n = a1 / a0;

    let mut prev_x = vec![0.0f64; channels];
    let mut prev_y = vec![0.0f64; channels];
    let mut out = Vec::with_capacity(sample_count);

    for frame in input.interleaved_i16_samples.chunks_exact(channels) {
        for (ch, sample) in frame.iter().enumerate() {
            let x = f64::from(*sample);
            let y = b0n * x + b1n * prev_x[ch] - a1n * prev_y[ch];
            prev_x[ch] = x;
            prev_y[ch] = y;

            let yr = y.round();
            let clamped = yr.max(f64::from(i16::MIN)).min(f64::from(i16::MAX)) as i16;
            out.push(clamped);
        }
    }

    Ok(PcmTrackData {
        spec: input.spec,
        interleaved_i16_samples: out,
    })
}

#[cfg(test)]
mod tests {
    use crate::audio::PcmSpec;

    use super::*;

    fn sample_track() -> PcmTrackData {
        PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: vec![
                0, 0, 500, -500, 1500, -1500, 3000, -3000, 4500, -4500, 2000, -2000, 1000, -1000,
                250, -250,
            ],
        }
    }

    #[test]
    fn passthrough_when_no_processing_is_enabled() {
        let input = sample_track();
        let output = process_track_pcm(
            &input,
            TrackProcessingOptions {
                decode_hdcd: false,
                deemphasis: true,
                force_deemphasis: false,
                track_has_preemphasis: false,
            },
        )
        .expect("processing should succeed");

        assert_eq!(output, input);
    }

    #[test]
    fn hdcd_path_selected_when_hdcd_is_requested() {
        let input = sample_track();
        let output = process_track_pcm(
            &input,
            TrackProcessingOptions {
                decode_hdcd: true,
                deemphasis: true,
                force_deemphasis: false,
                track_has_preemphasis: false,
            },
        )
        .expect("hdcd path should be accepted");

        assert_eq!(output, input);
    }

    #[test]
    fn applies_force_deemphasis_without_track_flag() {
        let input = sample_track();
        let output = process_track_pcm(
            &input,
            TrackProcessingOptions {
                decode_hdcd: false,
                deemphasis: false,
                force_deemphasis: true,
                track_has_preemphasis: false,
            },
        )
        .expect("force deemphasis should succeed");

        assert_ne!(output.interleaved_i16_samples, input.interleaved_i16_samples);
        assert_eq!(output.spec, input.spec);
    }

    #[test]
    fn no_deemphasis_disables_automatic_deemphasis_path() {
        let options = TrackProcessingOptions {
            decode_hdcd: false,
            deemphasis: false,
            force_deemphasis: false,
            track_has_preemphasis: true,
        };

        assert_eq!(options.selected_processing_path(), ProcessingPath::None);
    }

    #[test]
    fn force_deemphasis_overrides_no_deemphasis() {
        let options = TrackProcessingOptions {
            decode_hdcd: false,
            deemphasis: false,
            force_deemphasis: true,
            track_has_preemphasis: false,
        };

        assert_eq!(options.selected_processing_path(), ProcessingPath::Deemphasis);
    }

    #[test]
    fn hdcd_has_precedence_over_deemphasis_paths() {
        let options = TrackProcessingOptions {
            decode_hdcd: true,
            deemphasis: false,
            force_deemphasis: true,
            track_has_preemphasis: true,
        };

        assert_eq!(options.selected_processing_path(), ProcessingPath::Hdcd);
    }
}
