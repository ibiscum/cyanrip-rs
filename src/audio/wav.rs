use std::fs::File;
use std::io::{Cursor, Seek};
use std::path::Path;
use crate::audio::PcmTrackData;

#[derive(Debug)]
pub enum WavWriteError {
    InvalidSpec(&'static str),
    ChannelAlignment {
        channels: u16,
        sample_count: usize,
    },
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

fn validate_input(input: &PcmTrackData) -> Result<(), WavWriteError> {
    if input.spec.channels == 0 {
        return Err(WavWriteError::InvalidSpec("channels must be > 0"));
    }

    if input.spec.sample_rate == 0 {
        return Err(WavWriteError::InvalidSpec("sample_rate must be > 0"));
    }

    if input.spec.bits_per_sample != 16 {
        return Err(WavWriteError::InvalidSpec(
            "WAV writer currently supports 16-bit PCM only",
        ));
    }

    let channels = usize::from(input.spec.channels);
    if !input.interleaved_i16_samples.len().is_multiple_of(channels) {
        return Err(WavWriteError::ChannelAlignment {
            channels: input.spec.channels,
            sample_count: input.interleaved_i16_samples.len(),
        });
    }

    Ok(())
}

fn write_to_writer<W>(
    writer: W,
    input: &PcmTrackData,
) -> Result<(), WavWriteError>
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
    for sample in &input.interleaved_i16_samples {
        wav.write_sample(*sample)?;
    }
    wav.finalize()?;

    Ok(())
}

pub fn write_wav_file(path: &Path, input: &PcmTrackData) -> Result<(), WavWriteError> {
    let file = File::create(path)?;
    write_to_writer(file, input)
}

pub fn render_wav_bytes(input: &PcmTrackData) -> Result<Vec<u8>, WavWriteError> {
    let mut buf = Cursor::new(Vec::<u8>::new());
    write_to_writer(&mut buf, input)?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PcmSpec;

    fn sample_track_stereo() -> PcmTrackData {
        PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: vec![0, 1000, -1000, 32767, -32768, 42],
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
        assert_eq!(samples, track.interleaved_i16_samples);
    }

    #[test]
    fn rejects_non_16_bit_spec_for_now() {
        let mut track = sample_track_stereo();
        track.spec.bits_per_sample = 24;
        let err = render_wav_bytes(&track).expect_err("must reject non-16-bit path");
        assert!(format!("{err}").contains("16-bit"));
    }

    #[test]
    fn rejects_non_divisible_interleaved_samples() {
        let mut track = sample_track_stereo();
        track.interleaved_i16_samples.push(7);
        let err = render_wav_bytes(&track).expect_err("must reject misaligned channels");
        assert!(format!("{err}").contains("not divisible"));
    }
}
