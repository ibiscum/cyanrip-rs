use crate::audio::{PcmTrackData, ProcessedPcmTrackData};
use std::io::Write;
use std::process::{Command, Stdio};

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
    BackendUnavailable(String),
    BackendFailure(String),
}

impl std::fmt::Display for TrackProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSpec(msg) => write!(f, "invalid processing input spec: {msg}"),
            Self::BackendUnavailable(msg) => write!(f, "hdcd backend unavailable: {msg}"),
            Self::BackendFailure(msg) => write!(f, "hdcd backend failure: {msg}"),
        }
    }
}

impl std::error::Error for TrackProcessingError {}

pub fn process_track_pcm(
    input: &PcmTrackData,
    options: TrackProcessingOptions,
) -> Result<ProcessedPcmTrackData, TrackProcessingError> {
    match options.selected_processing_path() {
        ProcessingPath::Hdcd => apply_hdcd_ffmpeg(input),
        ProcessingPath::Deemphasis => apply_cd_deemphasis(input),
        ProcessingPath::None => Ok(ProcessedPcmTrackData {
            spec: input.spec,
            interleaved_i32_samples: input
                .interleaved_i16_samples
                .iter()
                .map(|&sample| i32::from(sample))
                .collect(),
        }),
    }
}

fn apply_hdcd_ffmpeg(input: &PcmTrackData) -> Result<ProcessedPcmTrackData, TrackProcessingError> {
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

    let channels = usize::from(input.spec.channels);
    if !input.interleaved_i16_samples.len().is_multiple_of(channels) {
        return Err(TrackProcessingError::InvalidSpec(
            "sample count must be divisible by channels",
        ));
    }

    let mut raw_input = Vec::with_capacity(input.interleaved_i16_samples.len() * 4);
    for sample in &input.interleaved_i16_samples {
        // Match ffmpeg's native s16->s32 conversion semantics (left shift by 16).
        let widened = i32::from(*sample) << 16;
        raw_input.extend_from_slice(&widened.to_le_bytes());
    }

    let sample_rate = input.spec.sample_rate.to_string();
    let channel_count = input.spec.channels.to_string();

    let mut child = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-auto_conversion_filters",
            "-f",
            "s32le",
            "-ar",
            sample_rate.as_str(),
            "-ac",
            channel_count.as_str(),
            "-i",
            "pipe:0",
            "-af",
            "hdcd",
            "-f",
            "s32le",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                TrackProcessingError::BackendUnavailable("ffmpeg executable not found in PATH".to_string())
            } else {
                TrackProcessingError::BackendFailure(format!("failed to spawn ffmpeg: {err}"))
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&raw_input).map_err(|err| {
            TrackProcessingError::BackendFailure(format!("failed writing PCM to ffmpeg stdin: {err}"))
        })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        TrackProcessingError::BackendFailure(format!("failed waiting for ffmpeg: {err}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("ffmpeg exited with status {}", output.status)
        } else {
            format!("{stderr} (status {})", output.status)
        };
        return Err(TrackProcessingError::BackendFailure(detail));
    }

    if !output.stdout.len().is_multiple_of(4) {
        return Err(TrackProcessingError::BackendFailure(
            "ffmpeg produced invalid PCM byte length".to_string(),
        ));
    }

    let mut samples = Vec::with_capacity(output.stdout.len() / 4);
    for chunk in output.stdout.chunks_exact(4) {
        let widened = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        // Convert ffmpeg's s32 domain into a 24-bit PCM container while preserving
        // HDCD's expanded precision (effective 20-bit content).
        let rounded = if widened >= 0 {
            widened.saturating_add(0x80)
        } else {
            widened.saturating_sub(0x80)
        };
        let narrowed = rounded >> 8;
        let clamped = narrowed.max(-8_388_608).min(8_388_607);
        samples.push(clamped);
    }

    if !samples.len().is_multiple_of(channels) {
        return Err(TrackProcessingError::BackendFailure(
            "ffmpeg output sample count is not channel-aligned".to_string(),
        ));
    }

    let mut spec = input.spec;
    spec.bits_per_sample = 24;

    Ok(ProcessedPcmTrackData {
        spec,
        interleaved_i32_samples: samples,
    })
}

fn apply_cd_deemphasis(input: &PcmTrackData) -> Result<ProcessedPcmTrackData, TrackProcessingError> {
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

    Ok(ProcessedPcmTrackData {
        spec: input.spec,
        interleaved_i32_samples: out.into_iter().map(i32::from).collect(),
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

        assert_eq!(output.spec, input.spec);
        let expected: Vec<i32> = input
            .interleaved_i16_samples
            .iter()
            .map(|&sample| i32::from(sample))
            .collect();
        assert_eq!(output.interleaved_i32_samples, expected);
    }

    #[test]
    fn hdcd_path_selected_when_hdcd_is_requested() {
        let input = sample_track();
        let result = process_track_pcm(
            &input,
            TrackProcessingOptions {
                decode_hdcd: true,
                deemphasis: true,
                force_deemphasis: false,
                track_has_preemphasis: false,
            },
        );

        match result {
            Ok(output) => {
                assert_eq!(output.spec.channels, input.spec.channels);
                assert_eq!(output.spec.sample_rate, input.spec.sample_rate);
                assert_eq!(output.spec.bits_per_sample, 24);
                assert_eq!(
                    output.interleaved_i32_samples.len(),
                    input.interleaved_i16_samples.len()
                );
            }
            Err(TrackProcessingError::BackendUnavailable(_)) => {
                // Some CI/dev environments do not provide ffmpeg.
            }
            Err(err) => panic!("unexpected hdcd processing failure: {err}"),
        }
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

        let input_as_i32: Vec<i32> = input
            .interleaved_i16_samples
            .iter()
            .map(|&sample| i32::from(sample))
            .collect();
        assert_ne!(output.interleaved_i32_samples, input_as_i32);
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
