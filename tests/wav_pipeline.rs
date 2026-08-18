use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cyanrip_rs::audio::wav::write_wav_file;
use cyanrip_rs::audio::{PcmSpec, PcmTrackData};

fn unique_temp_wav_path() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cyanrip-rs-wav-{now}.wav"))
}

#[test]
fn writes_wav_file_end_to_end() {
    let path = unique_temp_wav_path();
    let input = PcmTrackData {
        spec: PcmSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: vec![0, 10, -10, 300, -300, 1200, -1200],
    };

    write_wav_file(&path, &input).expect("wav file should be written");

    let mut reader = hound::WavReader::open(&path).expect("written wav should be readable");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 48_000);
    assert_eq!(spec.bits_per_sample, 16);

    let samples: Vec<i16> = reader
        .samples::<i16>()
        .map(|s| s.expect("sample should decode"))
        .collect();
    assert_eq!(samples, input.interleaved_i16_samples);

    let cleanup = fs::remove_file(&path);
    assert!(cleanup.is_ok(), "temporary wav should be removable");
}
