#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmSpec {
	pub channels: u16,
	pub sample_rate: u32,
	pub bits_per_sample: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmTrackData {
	pub spec: PcmSpec,
	pub interleaved_i16_samples: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedPcmTrackData {
	pub spec: PcmSpec,
	pub interleaved_i32_samples: Vec<i32>,
}

pub mod flac;
pub mod loudness;
pub mod process;
pub mod wav;
