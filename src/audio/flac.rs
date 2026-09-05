use std::fs;
use std::path::Path;

use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::audio::ProcessedPcmTrackData;

#[derive(Debug)]
pub enum FlacWriteError {
    InvalidSpec(&'static str),
    ChannelAlignment { channels: u16, sample_count: usize },
    Config(String),
    Encode(String),
    Io(std::io::Error),
}

impl std::fmt::Display for FlacWriteError {
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
            Self::Config(msg) => write!(f, "flac config error: {msg}"),
            Self::Encode(msg) => write!(f, "flac encode error: {msg}"),
            Self::Io(err) => write!(f, "io error: {err}"),
        }
    }
}

impl std::error::Error for FlacWriteError {}

impl From<std::io::Error> for FlacWriteError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

fn validate_input(input: &ProcessedPcmTrackData) -> Result<(), FlacWriteError> {
    if input.spec.channels == 0 {
        return Err(FlacWriteError::InvalidSpec("channels must be > 0"));
    }

    if input.spec.sample_rate == 0 {
        return Err(FlacWriteError::InvalidSpec("sample_rate must be > 0"));
    }

    if input.spec.bits_per_sample != 16 && input.spec.bits_per_sample != 24 {
        return Err(FlacWriteError::InvalidSpec(
            "FLAC writer currently supports 16-bit and 24-bit PCM",
        ));
    }

    let channels = usize::from(input.spec.channels);
    if !input.interleaved_i32_samples.len().is_multiple_of(channels) {
        return Err(FlacWriteError::ChannelAlignment {
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
        return Err(FlacWriteError::InvalidSpec(
            "PCM sample value out of range for declared bit depth",
        ));
    }

    let frames = input.interleaved_i32_samples.len() / channels;
    if frames < 16 {
        return Err(FlacWriteError::InvalidSpec(
            "FLAC writer currently requires at least 16 frames",
        ));
    }

    Ok(())
}

pub fn render_flac_bytes(input: &ProcessedPcmTrackData) -> Result<Vec<u8>, FlacWriteError> {
    validate_input(input)?;

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| FlacWriteError::Config(e.to_string()))?;

    let source = flacenc::source::MemSource::from_samples(
        &input
            .interleaved_i32_samples
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        usize::from(input.spec.channels),
        usize::from(input.spec.bits_per_sample),
        usize::try_from(input.spec.sample_rate)
            .map_err(|_| FlacWriteError::InvalidSpec("sample_rate out of range"))?,
    );

    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| FlacWriteError::Encode(e.to_string()))?;

    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| FlacWriteError::Encode(e.to_string()))?;
    Ok(sink.as_slice().to_vec())
}

pub fn write_flac_file(path: &Path, input: &ProcessedPcmTrackData) -> Result<(), FlacWriteError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = render_flac_bytes(input)?;
    fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::audio::PcmSpec;

    use super::*;

    fn sample_track_stereo() -> ProcessedPcmTrackData {
        ProcessedPcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i32_samples: vec![
                0, 1000, -1000, 32767, -32768, 42, 120, -120, 345, -345, 789, -789, 2000, -2000,
                1111, -1111, 4321, -4321, 99, -99, 12, -12, 34, -34, 56, -56, 78, -78, 90, -90,
                321, -321,
            ],
        }
    }

    #[test]
    fn renders_valid_flac_bytes_that_roundtrip() {
        let track = sample_track_stereo();
        let bytes = render_flac_bytes(&track).expect("flac bytes should render");
        assert!(bytes.starts_with(b"fLaC"));

        let mut reader = claxon::FlacReader::new(Cursor::new(bytes)).expect("reader should parse");
        let info = reader.streaminfo();
        assert_eq!(info.channels, 2);
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.bits_per_sample, 16);

        let samples: Vec<i32> = reader
            .samples()
            .map(|x| x.expect("sample decode should work"))
            .collect();
        let expected: Vec<i32> = track.interleaved_i32_samples.to_vec();
        assert_eq!(samples, expected);
    }

    #[test]
    fn rejects_unsupported_bit_depth() {
        let mut track = sample_track_stereo();
        track.spec.bits_per_sample = 20;
        let err = render_flac_bytes(&track).expect_err("must reject unsupported bit depth");
        assert!(format!("{err}").contains("16-bit and 24-bit"));
    }

    #[test]
    fn rejects_non_divisible_interleaved_samples() {
        let mut track = sample_track_stereo();
        track.interleaved_i32_samples.push(7);
        let err = render_flac_bytes(&track).expect_err("must reject misaligned channels");
        assert!(format!("{err}").contains("not divisible"));
    }
}
