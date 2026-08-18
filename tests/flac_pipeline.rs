use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cyanrip_rs::audio::flac::write_flac_file;
use cyanrip_rs::audio::{PcmSpec, PcmTrackData};

fn unique_temp_flac_path() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cyanrip-rs-flac-{now}.flac"))
}

#[test]
fn writes_flac_file_end_to_end() {
    let path = unique_temp_flac_path();
    let input = PcmTrackData {
        spec: PcmSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: vec![
            0, 10, -10, 300, -300, 1200, -1200, 50, -50, 75, -75, 90, -90, 110, -110, 130,
            -130,
        ],
    };

    write_flac_file(&path, &input).expect("flac file should be written");

    let mut reader = claxon::FlacReader::open(&path).expect("written flac should be readable");
    let info = reader.streaminfo();
    assert_eq!(info.channels, 1);
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.bits_per_sample, 16);

    let samples: Vec<i32> = reader
        .samples()
        .map(|s| s.expect("sample should decode"))
        .collect();
    let expected: Vec<i32> = input
        .interleaved_i16_samples
        .iter()
        .map(|&s| i32::from(s))
        .collect();
    assert_eq!(samples, expected);

    let cleanup = fs::remove_file(&path);
    assert!(cleanup.is_ok(), "temporary flac should be removable");
}
