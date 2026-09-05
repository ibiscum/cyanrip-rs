use crate::audio::ProcessedPcmTrackData;
use std::fs::File;
use std::io::{Cursor, Seek};
use std::path::Path;

#[derive(Debug)]
pub enum WavWriteError {
    InvalidSpec(&'static str),
    ChannelAlignment { channels: u16, sample_count: usize },
    Io(std::io::Error),
    Hound(hound::Error),
}

impl std::fmt::Display for WavWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSpec(msg) => write!(f, "invalid pcm spec: {msg}"),
            Self::ChannelAlignment {
                channels,
                sample_count,
            } => write!(
                f,
                "sample count {sample_count} is not divisible by channel count {channels}"
            ),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Hound(err) => write!(f, "wav writer error: {err}"),
        }
    }
}

impl std::error::Error for WavWriteError {}

impl From<std::io::Error> for WavWriteError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<hound::Error> for WavWriteError {
    fn from(value: hound::Error) -> Self {
        Self::Hound(value)
    }
}

fn validate_input(input: &ProcessedPcmTrackData) -> Result<(), WavWriteError> {
    if input.spec.channels == 0 {
        return Err(WavWriteError::InvalidSpec("channels must be > 0"));
    }

    if input.spec.sample_rate == 0 {
        return Err(WavWriteError::InvalidSpec("sample_rate must be > 0"));
    }

    if input.spec.bits_per_sample != 16 && input.spec.bits_per_sample != 24 {
        return Err(WavWriteError::InvalidSpec(
            "WAV writer currently supports 16-bit and 24-bit PCM",
        ));
    }

    let channels = usize::from(input.spec.channels);
    if !input.interleaved_i32_samples.len().is_multiple_of(channels) {
        return Err(WavWriteError::ChannelAlignment {
            channels: input.spec.channels,
            sample_count: input.interleaved_i32_samples.len(),
        });
    }

    let (min_allowed, max_allowed) = if input.spec.bits_per_sample == 16 {
        (i32::from(i16::MIN), i32::from(i16::MAX))
    } else {
        (-8_388_608, 8_388_607)
    };
    if input
        .interleaved_i32_samples
        .iter()
        .any(|&sample| sample < min_allowed || sample > max_allowed)
    {
        return Err(WavWriteError::InvalidSpec(
            "PCM sample value out of range for declared bit depth",
        ));
    }

    Ok(())
}

fn write_to_writer<W>(writer: W, input: &ProcessedPcmTrackData) -> Result<(), WavWriteError>
where
    W: std::io::Write + Seek,
{
    validate_input(input)?;

    let wav_spec = hound::WavSpec {
        channels: input.spec.channels,
        sample_rate: input.spec.sample_rate,
        bits_per_sample: input.spec.bits_per_sample,
        sample_format: hound::SampleFormat::Int,
    };

    let mut wav = hound::WavWriter::new(writer, wav_spec)?;
    if input.spec.bits_per_sample == 16 {
        for sample in &input.interleaved_i32_samples {
            let narrowed = (*sample).max(i32::from(i16::MIN)).min(i32::from(i16::MAX)) as i16;
            wav.write_sample(narrowed)?;
        }
    } else {
        for sample in &input.interleaved_i32_samples {
            wav.write_sample(*sample)?;
        }
    }
    wav.finalize()?;

    Ok(())
}

pub fn write_wav_file(path: &Path, input: &ProcessedPcmTrackData) -> Result<(), WavWriteError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    write_to_writer(file, input)
}

pub fn render_wav_bytes(input: &ProcessedPcmTrackData) -> Result<Vec<u8>, WavWriteError> {
    let mut buf = Cursor::new(Vec::<u8>::new());
    write_to_writer(&mut buf, input)?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PcmSpec;

    fn sample_track_stereo() -> ProcessedPcmTrackData {
        ProcessedPcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i32_samples: vec![0, 1000, -1000, 32767, -32768, 42],
        }
    }

    #[test]
    fn renders_valid_wav_bytes_that_roundtrip() {
        let track = sample_track_stereo();
        let bytes = render_wav_bytes(&track).expect("wav bytes should render");
        assert!(bytes.starts_with(b"RIFF"));
        assert!(bytes.windows(4).any(|w| w == b"WAVE"));

        let mut reader = hound::WavReader::new(Cursor::new(bytes)).expect("reader should parse");
        let spec = reader.spec();
        assert_eq!(spec.channels, 2);
        assert_eq!(spec.sample_rate, 44_100);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, hound::SampleFormat::Int);

        let samples: Vec<i16> = reader
            .samples::<i16>()
            .map(|x| x.expect("sample decode should work"))
            .collect();
        let expected: Vec<i16> = track
            .interleaved_i32_samples
            .iter()
            .map(|&sample| sample as i16)
            .collect();
        assert_eq!(samples, expected);
    }

    #[test]
    fn rejects_unsupported_bit_depth() {
        let mut track = sample_track_stereo();
        track.spec.bits_per_sample = 20;
        let err = render_wav_bytes(&track).expect_err("must reject unsupported bit depth");
        assert!(format!("{err}").contains("16-bit and 24-bit"));
    }

    #[test]
    fn renders_24_bit_wav_without_truncating_container_depth() {
        let mut track = sample_track_stereo();
        track.spec.bits_per_sample = 24;
        track.interleaved_i32_samples =
            vec![0, 256, -256, 65_536, -65_536, 8_388_607, -8_388_608, 1_024];

        let bytes = render_wav_bytes(&track).expect("24-bit wav bytes should render");
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).expect("reader should parse");
        let spec = reader.spec();
        assert_eq!(spec.bits_per_sample, 24);
        let samples: Vec<i32> = reader
            .samples::<i32>()
            .map(|x| x.expect("24-bit sample decode should work"))
            .collect();
        assert_eq!(samples, track.interleaved_i32_samples);
    }

    #[test]
    fn rejects_non_divisible_interleaved_samples() {
        let mut track = sample_track_stereo();
        track.interleaved_i32_samples.push(7);
        let err = render_wav_bytes(&track).expect_err("must reject misaligned channels");
        assert!(format!("{err}").contains("not divisible"));
    }
}
