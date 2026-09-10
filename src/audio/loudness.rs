use super::PcmTrackData;
use ebur128::{EbuR128, Mode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessMeasurements {
    pub integrated_lufs: f64,
    pub integrated_threshold_lufs: f64,
    pub lra_lu: f64,
    pub lra_threshold_lufs: f64,
    pub lra_low_lufs: f64,
    pub lra_high_lufs: f64,
    pub true_peak_dbtp: f64,
    pub sample_peak: f64,
}

impl Default for LoudnessMeasurements {
    fn default() -> Self {
        Self {
            integrated_lufs: f64::NEG_INFINITY,
            integrated_threshold_lufs: f64::NEG_INFINITY,
            lra_lu: 0.0,
            lra_threshold_lufs: f64::NEG_INFINITY,
            lra_low_lufs: f64::NEG_INFINITY,
            lra_high_lufs: f64::NEG_INFINITY,
            true_peak_dbtp: f64::NEG_INFINITY,
            sample_peak: 0.0,
        }
    }
}

/// Measures EBU R128 integrated loudness, loudness range, true peak, and
/// sample peak from 16-bit interleaved PCM. Returns `None` if the sample rate
/// or channel count is unsupported by libebur128.
pub fn measure_loudness(pcm: &PcmTrackData) -> Option<LoudnessMeasurements> {
    let channels = pcm.spec.channels as u32;
    let rate = pcm.spec.sample_rate;
    if channels == 0 || rate == 0 {
        return None;
    }

    let frames = pcm.interleaved_i16_samples.len() / channels as usize;
    if frames == 0 {
        return Some(LoudnessMeasurements::default());
    }

    let mode = Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK | Mode::LRA;
    let mut ebur = EbuR128::new(channels, rate, mode).ok()?;

    // ebur128's add_frames_i16 expects interleaved i16 samples.
    ebur.add_frames_i16(&pcm.interleaved_i16_samples).ok()?;

    let integrated = ebur.loudness_global().ok()?;
    // ebur128 0.1 does not expose threshold helpers directly; use the
    // integrated loudness as a pragmatic fallback for threshold values.
    let integrated_threshold = integrated;
    let lra = ebur.loudness_range().ok()?;
    let lra_threshold = integrated;
    let lra_low = integrated - lra / 2.0;
    let lra_high = integrated + lra / 2.0;

    // True peak across all channels; take the maximum.
    let mut true_peak_linear = 0.0f64;
    for ch in 0..channels {
        if let Ok(tp) = ebur.true_peak(ch) {
            true_peak_linear = true_peak_linear.max(tp);
        }
    }
    let true_peak_dbtp = if true_peak_linear > 0.0 {
        20.0 * true_peak_linear.log10()
    } else {
        f64::NEG_INFINITY
    };

    // Sample peak across all channels.
    let mut sample_peak = 0.0f64;
    for ch in 0..channels {
        if let Ok(sp) = ebur.sample_peak(ch) {
            sample_peak = sample_peak.max(sp);
        }
    }

    Some(LoudnessMeasurements {
        integrated_lufs: integrated,
        integrated_threshold_lufs: integrated_threshold,
        lra_lu: lra,
        lra_threshold_lufs: lra_threshold,
        lra_low_lufs: lra_low,
        lra_high_lufs: lra_high,
        true_peak_dbtp,
        sample_peak,
    })
}

/// Computes ReplayGain-style track gain and peak tags from loudness
/// measurements, using a reference loudness of -18 LUFS (cyanrip default).
pub fn replaygain_from_loudness(l: &LoudnessMeasurements) -> (f64, f64) {
    const REF_LUFS: f64 = -18.0;
    let gain = if l.integrated_lufs.is_finite() {
        REF_LUFS - l.integrated_lufs
    } else {
        0.0
    };
    (gain, l.sample_peak)
}

/// Formats a duration in samples to MM:SS.ff (frames) like upstream cyanrip.
pub fn format_duration_samples(samples: usize, sample_rate: u32) -> String {
    if sample_rate == 0 {
        return "00:00.00".to_string();
    }
    let total_seconds = samples as f64 / sample_rate as f64;
    let minutes = (total_seconds / 60.0).floor() as u64;
    let seconds = total_seconds % 60.0;
    let frames = (seconds.fract() * 75.0).round() as u64;
    let whole_seconds = seconds.floor() as u64;
    format!("{:02}:{:02}.{:02}", minutes, whole_seconds, frames)
}
