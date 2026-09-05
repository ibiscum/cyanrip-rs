use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use cyanrip_rs::app::{
    AccuRipLookup, AppTrack, CoverArtLookup, MetadataFlowInput, MusicBrainzLookup,
    TrackOutputFlowInput, TrackOutputInput, orchestrate_metadata_flow, write_track_outputs,
};
use cyanrip_rs::audio::{PcmSpec, PcmTrackData};
use cyanrip_rs::cli::parse_from_iter;
use cyanrip_rs::metadata::accurip::{
    AccuDbStatus, AccuRipError, AccuRipLookupResult, AccuRipTrackInput,
};
use cyanrip_rs::metadata::coverart::{CoverArtError, CoverArtImage};
use cyanrip_rs::metadata::musicbrainz::{MusicBrainzError, MusicBrainzReleaseMeta};

#[derive(Clone)]
struct MbMock {
    called: Arc<Mutex<usize>>,
}

#[async_trait]
impl MusicBrainzLookup for MbMock {
    async fn lookup_release(
        &self,
        _discid: &str,
        _release_selection: Option<&cyanrip_rs::ReleaseSelection>,
        _discnumber: i32,
        _nb_cd_tracks: usize,
    ) -> Result<MusicBrainzReleaseMeta, MusicBrainzError> {
        *self.called.lock().expect("lock") += 1;
        Err(MusicBrainzError::NotFound)
    }
}

#[derive(Clone)]
struct CoverMock {
    called: Arc<Mutex<usize>>,
}

#[async_trait]
impl CoverArtLookup for CoverMock {
    async fn fill_release_coverart(
        &self,
        cover_arts: &mut Vec<CoverArtImage>,
        _release_id: Option<&str>,
        _disable_coverart_db: bool,
        _lookup_size: cyanrip_rs::CoverArtLookupSize,
        _info_only: bool,
    ) -> Result<(), CoverArtError> {
        *self.called.lock().expect("lock") += 1;
        cover_arts.push(CoverArtImage {
            title: "Front".to_string(),
            source: Some("mock".to_string()),
            source_url: "http://example/front.jpg".to_string(),
            extension: Some("jpg".to_string()),
            data: Some(vec![1, 2, 3]),
            content_type: Some("image/jpeg".to_string()),
        });
        Ok(())
    }
}

#[derive(Clone)]
struct ArMock {
    called: Arc<Mutex<usize>>,
}

#[async_trait]
impl AccuRipLookup for ArMock {
    async fn lookup(
        &self,
        _tracks: &[AccuRipTrackInput],
        _cddb_id: u32,
    ) -> Result<AccuRipLookupResult, AccuRipError> {
        *self.called.lock().expect("lock") += 1;
        Err(AccuRipError::Http("mock error".to_string()))
    }
}

fn sample_disc_tracks() -> Vec<AppTrack> {
    vec![
        AppTrack {
            number: 1,
            start_lsn: 0,
            end_lsn: 14_999,
            track_is_data: false,
        },
        AppTrack {
            number: 2,
            start_lsn: 15_000,
            end_lsn: 29_999,
            track_is_data: false,
        },
    ]
}

fn sample_pcm() -> PcmTrackData {
    PcmTrackData {
        spec: PcmSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: vec![
            0, 10, -10, 300, -300, 1200, -1200, 50, -50, 75, -75, 90, -90, 110, -110, 130, -130,
        ],
    }
}

fn unique_temp_output_root() -> PathBuf {
    let repo_tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp");
    std::fs::create_dir_all(&repo_tmp).expect("repo tmp root should be creatable");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    repo_tmp.join(format!("cyanrip-rs-cli-app-it-{now}"))
}

fn first_vorbis_value(tag: &metaflac::Tag, key: &str) -> Option<String> {
    tag.get_vorbis(key)
        .and_then(|values| values.into_iter().next())
        .map(ToString::to_string)
}

fn has_vorbis_key(tag: &metaflac::Tag, key: &str) -> bool {
    tag.get_vorbis(key).is_some()
}

#[tokio::test]
async fn cli_disable_flags_propagate_to_metadata_orchestration() {
    let cfg = parse_from_iter(["cyanrip-rs", "-N", "-A", "-o", "wav,flac"])
        .expect("cli parse should succeed");

    let mb_calls = Arc::new(Mutex::new(0usize));
    let cover_calls = Arc::new(Mutex::new(0usize));
    let ar_calls = Arc::new(Mutex::new(0usize));

    let mb = MbMock {
        called: mb_calls.clone(),
    };
    let cover = CoverMock {
        called: cover_calls.clone(),
    };
    let ar = ArMock {
        called: ar_calls.clone(),
    };

    let out = orchestrate_metadata_flow(
        MetadataFlowInput {
            settings: cfg.settings,
            tracks: sample_disc_tracks(),
            info_only: false,
            initial_cover_arts: Vec::new(),
        },
        &mb,
        &cover,
        &ar,
    )
    .await;

    assert!(out.musicbrainz.is_none());
    assert_eq!(out.accurip_status, AccuDbStatus::Disabled);
    assert!(out.accurip.is_none());
    assert_eq!(*mb_calls.lock().expect("lock"), 0);
    assert_eq!(*ar_calls.lock().expect("lock"), 0);
    assert_eq!(*cover_calls.lock().expect("lock"), 1);
}

#[test]
fn cli_outputs_and_disc_tags_drive_writer_dispatch_and_flac_tags() {
    let cfg = parse_from_iter([
        "cyanrip-rs",
        "-o",
        "wav,flac",
        "-c",
        "1/2",
        "-D",
        "{album} [{format}]",
        "-F",
        "{track} - {title}",
    ])
    .expect("cli parse should succeed");

    let output_root = unique_temp_output_root();
    let album_meta: HashMap<String, String> = [
        ("album".to_string(), "Example Album".to_string()),
        ("album_artist".to_string(), "Example Artist".to_string()),
    ]
    .into_iter()
    .collect();

    let tracks = vec![TrackOutputInput {
        track_number: 1,
        track_meta: [
            ("track".to_string(), "01".to_string()),
            ("title".to_string(), "Intro".to_string()),
            ("artist".to_string(), "Track Artist".to_string()),
        ]
        .into_iter()
        .collect(),
        pcm: sample_pcm(),
    }];

    let out = write_track_outputs(TrackOutputFlowInput {
        settings: cfg.settings,
        output_root: output_root.clone(),
        album_meta,
        cover_arts: Vec::new(),
        tracks,
    })
    .expect("writer dispatch should succeed");

    assert_eq!(out.written_files.len(), 2);
    let wav = output_root.join("Example Album [WAV]/01 - Intro.wav");
    let flac = output_root.join("Example Album [FLAC]/01 - Intro.flac");
    assert!(wav.exists());
    assert!(flac.exists());

    let tag = metaflac::Tag::read_from_path(&flac).expect("flac tag read should work");
    assert_eq!(
        first_vorbis_value(&tag, "ALBUM").as_deref(),
        Some("Example Album")
    );
    assert_eq!(
        first_vorbis_value(&tag, "ALBUMARTIST").as_deref(),
        Some("Example Artist")
    );
    assert_eq!(
        first_vorbis_value(&tag, "TRACKNUMBER").as_deref(),
        Some("01")
    );
    assert_eq!(first_vorbis_value(&tag, "DISCNUMBER").as_deref(), Some("1"));
    assert_eq!(first_vorbis_value(&tag, "DISCTOTAL").as_deref(), Some("2"));
    assert!(has_vorbis_key(&tag, "REPLAYGAIN_TRACK_GAIN"));
    assert!(has_vorbis_key(&tag, "REPLAYGAIN_TRACK_PEAK"));
    assert!(has_vorbis_key(&tag, "REPLAYGAIN_ALBUM_GAIN"));
    assert!(has_vorbis_key(&tag, "REPLAYGAIN_ALBUM_PEAK"));

    let cleanup = std::fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "temporary output root should be removable");
}

#[test]
fn cli_no_replaygain_disables_replaygain_flac_tags() {
    let cfg = parse_from_iter([
        "cyanrip-rs",
        "-K",
        "-o",
        "flac",
        "-c",
        "1/1",
        "-D",
        "{album} [{format}]",
        "-F",
        "{track} - {title}",
    ])
    .expect("cli parse should succeed");

    let output_root = unique_temp_output_root();
    let album_meta: HashMap<String, String> = [
        ("album".to_string(), "Example Album".to_string()),
        ("album_artist".to_string(), "Example Artist".to_string()),
    ]
    .into_iter()
    .collect();

    let tracks = vec![TrackOutputInput {
        track_number: 1,
        track_meta: [
            ("track".to_string(), "01".to_string()),
            ("title".to_string(), "Intro".to_string()),
            ("artist".to_string(), "Track Artist".to_string()),
        ]
        .into_iter()
        .collect(),
        pcm: sample_pcm(),
    }];

    let out = write_track_outputs(TrackOutputFlowInput {
        settings: cfg.settings,
        output_root: output_root.clone(),
        album_meta,
        cover_arts: Vec::new(),
        tracks,
    })
    .expect("writer dispatch should succeed");

    assert_eq!(out.written_files.len(), 1);
    let flac = output_root.join("Example Album [FLAC]/01 - Intro.flac");
    assert!(flac.exists());

    let tag = metaflac::Tag::read_from_path(&flac).expect("flac tag read should work");
    assert!(!has_vorbis_key(&tag, "REPLAYGAIN_TRACK_GAIN"));
    assert!(!has_vorbis_key(&tag, "REPLAYGAIN_TRACK_PEAK"));
    assert!(!has_vorbis_key(&tag, "REPLAYGAIN_ALBUM_GAIN"));
    assert!(!has_vorbis_key(&tag, "REPLAYGAIN_ALBUM_PEAK"));

    let cleanup = std::fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "temporary output root should be removable");
}
