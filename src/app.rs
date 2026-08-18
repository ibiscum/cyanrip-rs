use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::flac::write_flac_file;
use crate::audio::wav::write_wav_file;
use crate::audio::PcmTrackData;
use crate::metadata::accurip::{
    AccuDbStatus, AccuRipError, AccuRipLookupResult, AccuRipService, AccuRipTrackInput,
};
use crate::metadata::coverart::{CoverArtError, CoverArtImage, CoverArtService};
use crate::metadata::discid::{DiscTrack, DiscidInfo, compute_discid};
use crate::metadata::musicbrainz::{MusicBrainzError, MusicBrainzReleaseMeta, MusicBrainzService};
use crate::naming::{NamingContext, build_track_relative_path};
use crate::{OutputFormat, ReleaseSelection, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTrack {
    pub number: u8,
    pub start_lsn: i32,
    pub end_lsn: i32,
    pub track_is_data: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataFlowInput {
    pub settings: Settings,
    pub tracks: Vec<AppTrack>,
    pub info_only: bool,
    pub initial_cover_arts: Vec<CoverArtImage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataFlowResult {
    pub discid: Option<DiscidInfo>,
    pub musicbrainz: Option<MusicBrainzReleaseMeta>,
    pub cover_arts: Vec<CoverArtImage>,
    pub accurip_status: AccuDbStatus,
    pub accurip: Option<AccuRipLookupResult>,
    pub warnings: Vec<String>,
}

#[async_trait]
pub trait MusicBrainzLookup {
    async fn lookup_release(
        &self,
        discid: &str,
        release_selection: Option<&ReleaseSelection>,
        discnumber: i32,
        nb_cd_tracks: usize,
    ) -> Result<MusicBrainzReleaseMeta, MusicBrainzError>;
}

#[async_trait]
pub trait CoverArtLookup {
    async fn fill_release_coverart(
        &self,
        cover_arts: &mut Vec<CoverArtImage>,
        release_id: Option<&str>,
        disable_coverart_db: bool,
        lookup_size: crate::CoverArtLookupSize,
        info_only: bool,
    ) -> Result<(), CoverArtError>;
}

#[async_trait]
pub trait AccuRipLookup {
    async fn lookup(
        &self,
        tracks: &[AccuRipTrackInput],
        cddb_id: u32,
    ) -> Result<AccuRipLookupResult, AccuRipError>;
}

#[async_trait]
impl<H> MusicBrainzLookup for MusicBrainzService<H>
where
    H: crate::metadata::musicbrainz::MusicBrainzHttpClient,
{
    async fn lookup_release(
        &self,
        discid: &str,
        release_selection: Option<&ReleaseSelection>,
        discnumber: i32,
        nb_cd_tracks: usize,
    ) -> Result<MusicBrainzReleaseMeta, MusicBrainzError> {
        MusicBrainzService::lookup_release(
            self,
            discid,
            release_selection,
            discnumber,
            nb_cd_tracks,
        )
        .await
    }
}

#[async_trait]
impl<H> CoverArtLookup for CoverArtService<H>
where
    H: crate::metadata::coverart::CoverArtHttpClient,
{
    async fn fill_release_coverart(
        &self,
        cover_arts: &mut Vec<CoverArtImage>,
        release_id: Option<&str>,
        disable_coverart_db: bool,
        lookup_size: crate::CoverArtLookupSize,
        info_only: bool,
    ) -> Result<(), CoverArtError> {
        CoverArtService::fill_release_coverart(
            self,
            cover_arts,
            release_id,
            disable_coverart_db,
            lookup_size,
            info_only,
        )
        .await
    }
}

#[async_trait]
impl<H> AccuRipLookup for AccuRipService<H>
where
    H: crate::metadata::accurip::AccuRipHttpClient,
{
    async fn lookup(
        &self,
        tracks: &[AccuRipTrackInput],
        cddb_id: u32,
    ) -> Result<AccuRipLookupResult, AccuRipError> {
        AccuRipService::lookup(self, tracks, cddb_id).await
    }
}

pub async fn orchestrate_metadata_flow<M, C, A>(
    input: MetadataFlowInput,
    musicbrainz: &M,
    coverart: &C,
    accurip: &A,
) -> MetadataFlowResult
where
    M: MusicBrainzLookup + Sync,
    C: CoverArtLookup + Sync,
    A: AccuRipLookup + Sync,
{
    let mut warnings = Vec::new();
    let mut cover_arts = input.initial_cover_arts;

    let disc_tracks: Vec<DiscTrack> = input
        .tracks
        .iter()
        .map(|t| DiscTrack {
            number: t.number,
            start_lsn: t.start_lsn,
            end_lsn: t.end_lsn,
            track_is_data: t.track_is_data,
        })
        .collect();

    let discid = match compute_discid(&disc_tracks) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(format!("discid computation failed: {e:?}"));
            None
        }
    };

    let mut musicbrainz_meta = None;
    if !input.settings.disable_mb {
        if let Some(d) = &discid {
            match musicbrainz
                .lookup_release(
                    &d.musicbrainz_discid,
                    input.settings.release.as_ref(),
                    input.settings.discnumber,
                    input.tracks.len(),
                )
                .await
            {
                Ok(v) => musicbrainz_meta = Some(v),
                Err(e) => warnings.push(format!("musicbrainz lookup failed: {e:?}")),
            }
        } else {
            warnings.push("musicbrainz lookup skipped: discid unavailable".to_string());
        }
    }

    let release_id = musicbrainz_meta
        .as_ref()
        .map(|m| m.musicbrainz_albumid.as_str());
    if let Err(e) = coverart
        .fill_release_coverart(
            &mut cover_arts,
            release_id,
            input.settings.disable_coverart_db,
            input.settings.coverart_lookup_size,
            input.info_only,
        )
        .await
    {
        warnings.push(format!("coverart lookup failed: {e:?}"));
    }

    let mut accurip_status = if input.settings.disable_accurip {
        AccuDbStatus::Disabled
    } else {
        AccuDbStatus::Error
    };
    let mut accurip_result = None;

    if !input.settings.disable_accurip {
        if let Some(d) = &discid {
            let parsed_cddb = u32::from_str_radix(&d.cddb, 16);
            let cddb = match parsed_cddb {
                Ok(v) => v,
                Err(_) => {
                    warnings.push("accurip lookup failed: invalid cddb id".to_string());
                    return MetadataFlowResult {
                        discid,
                        musicbrainz: musicbrainz_meta,
                        cover_arts,
                        accurip_status,
                        accurip: accurip_result,
                        warnings,
                    };
                }
            };

            let mut ar_tracks = Vec::new();
            for t in &input.tracks {
                if t.start_lsn < 0 || t.end_lsn < 0 {
                    warnings.push("accurip lookup failed: negative LSN in track data".to_string());
                    return MetadataFlowResult {
                        discid,
                        musicbrainz: musicbrainz_meta,
                        cover_arts,
                        accurip_status,
                        accurip: accurip_result,
                        warnings,
                    };
                }
                ar_tracks.push(AccuRipTrackInput {
                    number: t.number as u32,
                    start_lsn: t.start_lsn as u32,
                    end_lsn: t.end_lsn as u32,
                    track_is_data: t.track_is_data,
                });
            }

            match accurip.lookup(&ar_tracks, cddb).await {
                Ok(v) => {
                    accurip_status = v.status;
                    accurip_result = Some(v);
                }
                Err(e) => {
                    warnings.push(format!("accurip lookup failed: {e:?}"));
                    accurip_status = AccuDbStatus::Error;
                }
            }
        } else {
            warnings.push("accurip lookup skipped: discid/cddb unavailable".to_string());
        }
    }

    MetadataFlowResult {
        discid,
        musicbrainz: musicbrainz_meta,
        cover_arts,
        accurip_status,
        accurip: accurip_result,
        warnings,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOutputInput {
    pub track_number: u32,
    pub track_meta: HashMap<String, String>,
    pub pcm: PcmTrackData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackOutputFlowInput {
    pub settings: Settings,
    pub output_root: PathBuf,
    pub album_meta: HashMap<String, String>,
    pub tracks: Vec<TrackOutputInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOutputFile {
    pub track_number: u32,
    pub output_format: OutputFormat,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOutputFlowResult {
    pub written_files: Vec<TrackOutputFile>,
}

#[derive(Debug)]
pub enum TrackOutputFlowError {
    UnsupportedOutputFormat(OutputFormat),
    Naming(String),
    Io(std::io::Error),
    Tagging {
        output_format: OutputFormat,
        path: PathBuf,
        message: String,
    },
    Encode {
        output_format: OutputFormat,
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for TrackOutputFlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedOutputFormat(fmt_kind) => {
                write!(f, "output format {fmt_kind:?} is not yet implemented")
            }
            Self::Naming(msg) => write!(f, "naming error: {msg}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Tagging {
                output_format,
                path,
                message,
            } => write!(
                f,
                "tagging error for {output_format:?} at {}: {message}",
                path.display()
            ),
            Self::Encode {
                output_format,
                path,
                message,
            } => write!(
                f,
                "encode error for {output_format:?} at {}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for TrackOutputFlowError {}

impl From<std::io::Error> for TrackOutputFlowError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

fn output_format_descriptor(fmt_kind: OutputFormat) -> Option<(&'static str, &'static str)> {
    match fmt_kind {
        OutputFormat::Wav => Some(("WAV", "wav")),
        OutputFormat::Flac => Some(("FLAC", "flac")),
        _ => None,
    }
}

fn canonical_flac_vorbis_key(input: &str) -> String {
    match input {
        "track" | "tracknumber" => "TRACKNUMBER".to_string(),
        "disc" | "discnumber" => "DISCNUMBER".to_string(),
        "totaldiscs" | "disctotal" => "DISCTOTAL".to_string(),
        "album_artist" => "ALBUMARTIST".to_string(),
        "musicbrainz_albumid" => "MUSICBRAINZ_ALBUMID".to_string(),
        "musicbrainz_discid" => "MUSICBRAINZ_DISCID".to_string(),
        _ => input.to_ascii_uppercase(),
    }
}

fn build_flac_comment_map(
    settings: &Settings,
    album_meta: &HashMap<String, String>,
    track: &TrackOutputInput,
) -> HashMap<String, String> {
    let mut merged = HashMap::new();

    for (key, value) in album_meta {
        if value.is_empty() {
            continue;
        }
        merged.insert(canonical_flac_vorbis_key(key), value.clone());
    }

    for (key, value) in &track.track_meta {
        if value.is_empty() {
            continue;
        }
        merged.insert(canonical_flac_vorbis_key(key), value.clone());
    }

    merged
        .entry("TRACKNUMBER".to_string())
        .or_insert_with(|| track.track_number.to_string());

    if settings.discnumber > 0 {
        merged
            .entry("DISCNUMBER".to_string())
            .or_insert_with(|| settings.discnumber.to_string());
    }
    if settings.totaldiscs > 0 {
        merged
            .entry("DISCTOTAL".to_string())
            .or_insert_with(|| settings.totaldiscs.to_string());
    }

    merged
}

fn embed_flac_vorbis_comments(
    path: &Path,
    comments: &HashMap<String, String>,
) -> Result<(), TrackOutputFlowError> {
    let mut tag = metaflac::Tag::read_from_path(path).map_err(|e| TrackOutputFlowError::Tagging {
        output_format: OutputFormat::Flac,
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    for (key, value) in comments {
        tag.set_vorbis(key, vec![value]);
    }

    tag.save().map_err(|e| TrackOutputFlowError::Tagging {
        output_format: OutputFormat::Flac,
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

pub fn write_track_outputs(input: TrackOutputFlowInput) -> Result<TrackOutputFlowResult, TrackOutputFlowError> {
    let naming_ctx = NamingContext {
        sanitize_method: input.settings.sanitize_method,
        nb_tracks: input.tracks.len(),
    };

    let mut written_files = Vec::new();
    for fmt_kind in &input.settings.outputs {
        let (format_suffix, extension) = output_format_descriptor(*fmt_kind)
            .ok_or(TrackOutputFlowError::UnsupportedOutputFormat(*fmt_kind))?;

        for track in &input.tracks {
            let relative_path_str = build_track_relative_path(
                &naming_ctx,
                &input.album_meta,
                &track.track_meta,
                &input.settings.folder_name_scheme,
                &input.settings.track_name_scheme,
                format_suffix,
                extension,
            )
            .map_err(TrackOutputFlowError::Naming)?;

            let relative_path = PathBuf::from(relative_path_str);
            let absolute_path = input.output_root.join(&relative_path);

            if let Some(parent) = absolute_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            match fmt_kind {
                OutputFormat::Wav => write_wav_file(&absolute_path, &track.pcm).map_err(|e| {
                    TrackOutputFlowError::Encode {
                        output_format: *fmt_kind,
                        path: absolute_path.clone(),
                        message: e.to_string(),
                    }
                })?,
                OutputFormat::Flac => {
                    write_flac_file(&absolute_path, &track.pcm).map_err(|e| {
                        TrackOutputFlowError::Encode {
                            output_format: *fmt_kind,
                            path: absolute_path.clone(),
                            message: e.to_string(),
                        }
                    })?;

                    let comments = build_flac_comment_map(&input.settings, &input.album_meta, track);
                    embed_flac_vorbis_comments(&absolute_path, &comments)?;
                }
                _ => {
                    return Err(TrackOutputFlowError::UnsupportedOutputFormat(*fmt_kind));
                }
            }

            written_files.push(TrackOutputFile {
                track_number: track.track_number,
                output_format: *fmt_kind,
                relative_path,
                absolute_path,
            });
        }
    }

    Ok(TrackOutputFlowResult { written_files })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::audio::{PcmSpec, PcmTrackData};
    use crate::CoverArtLookupSize;
    use crate::metadata::accurip::{AccuRipDiscIds, AccuRipTrackMatches};

    #[derive(Clone)]
    struct MbMock {
        called: Arc<Mutex<usize>>,
        result: Result<MusicBrainzReleaseMeta, MusicBrainzError>,
    }

    #[async_trait]
    impl MusicBrainzLookup for MbMock {
        async fn lookup_release(
            &self,
            _discid: &str,
            _release_selection: Option<&ReleaseSelection>,
            _discnumber: i32,
            _nb_cd_tracks: usize,
        ) -> Result<MusicBrainzReleaseMeta, MusicBrainzError> {
            *self.called.lock().expect("lock") += 1;
            self.result.clone()
        }
    }

    #[derive(Clone)]
    struct CoverMock {
        called: Arc<Mutex<usize>>,
        release_ids: Arc<Mutex<Vec<Option<String>>>>,
    }

    #[async_trait]
    impl CoverArtLookup for CoverMock {
        async fn fill_release_coverart(
            &self,
            cover_arts: &mut Vec<CoverArtImage>,
            release_id: Option<&str>,
            _disable_coverart_db: bool,
            _lookup_size: CoverArtLookupSize,
            _info_only: bool,
        ) -> Result<(), CoverArtError> {
            *self.called.lock().expect("lock") += 1;
            self.release_ids
                .lock()
                .expect("lock")
                .push(release_id.map(ToString::to_string));
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
        result: Result<AccuRipLookupResult, AccuRipError>,
    }

    #[async_trait]
    impl AccuRipLookup for ArMock {
        async fn lookup(
            &self,
            _tracks: &[AccuRipTrackInput],
            _cddb_id: u32,
        ) -> Result<AccuRipLookupResult, AccuRipError> {
            *self.called.lock().expect("lock") += 1;
            self.result.clone()
        }
    }

    fn test_tracks() -> Vec<AppTrack> {
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

    fn mb_ok() -> MusicBrainzReleaseMeta {
        MusicBrainzReleaseMeta {
            musicbrainz_albumid: "rel-1".to_string(),
            releasecomment: None,
            date: Some("2024-01-01".to_string()),
            album: "Album".to_string(),
            barcode: None,
            packaging: None,
            country: None,
            releasestatus: None,
            catalognumber: None,
            label: None,
            album_artist: Some("Artist".to_string()),
            discname: None,
            format: Some("CD".to_string()),
            discnumber: Some(1),
            totaldiscs: 1,
            tracks: Vec::new(),
        }
    }

    fn ar_ok() -> AccuRipLookupResult {
        AccuRipLookupResult {
            status: AccuDbStatus::Found,
            request_url: "http://ar".to_string(),
            disc_ids: AccuRipDiscIds {
                audio_tracks: 2,
                id_type_1: 1,
                id_type_2: 2,
            },
            track_matches: vec![
                AccuRipTrackMatches {
                    entries: Vec::new(),
                    max_confidence: 0,
                },
                AccuRipTrackMatches {
                    entries: Vec::new(),
                    max_confidence: 0,
                },
            ],
        }
    }

    fn unique_temp_output_root() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cyanrip-rs-output-dispatch-{now}"))
    }

    fn sample_pcm() -> PcmTrackData {
        PcmTrackData {
            spec: PcmSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: vec![
                0, 10, -10, 300, -300, 1200, -1200, 50, -50, 75, -75, 90, -90, 110, -110, 130,
                -130,
            ],
        }
    }

    fn track_meta(track: &str, title: &str) -> HashMap<String, String> {
        [
            ("track".to_string(), track.to_string()),
            ("title".to_string(), title.to_string()),
        ]
        .into_iter()
        .collect()
    }

    fn first_vorbis_value(tag: &metaflac::Tag, key: &str) -> Option<String> {
        tag.get_vorbis(key)
            .and_then(|values| values.into_iter().next())
            .map(ToString::to_string)
    }

    #[tokio::test]
    async fn orchestrates_full_flow_in_expected_order() {
        let mb_calls = Arc::new(Mutex::new(0usize));
        let cover_calls = Arc::new(Mutex::new(0usize));
        let ar_calls = Arc::new(Mutex::new(0usize));
        let release_ids = Arc::new(Mutex::new(Vec::new()));

        let mb = MbMock {
            called: mb_calls.clone(),
            result: Ok(mb_ok()),
        };
        let cover = CoverMock {
            called: cover_calls.clone(),
            release_ids: release_ids.clone(),
        };
        let ar = ArMock {
            called: ar_calls.clone(),
            result: Ok(ar_ok()),
        };

        let input = MetadataFlowInput {
            settings: Settings::default(),
            tracks: test_tracks(),
            info_only: false,
            initial_cover_arts: Vec::new(),
        };

        let out = orchestrate_metadata_flow(input, &mb, &cover, &ar).await;

        assert!(out.discid.is_some());
        assert!(out.musicbrainz.is_some());
        assert_eq!(out.accurip_status, AccuDbStatus::Found);
        assert!(out.accurip.is_some());
        assert_eq!(out.cover_arts.len(), 1);
        assert!(out.warnings.is_empty());

        assert_eq!(*mb_calls.lock().expect("lock"), 1);
        assert_eq!(*cover_calls.lock().expect("lock"), 1);
        assert_eq!(*ar_calls.lock().expect("lock"), 1);

        let ids = release_ids.lock().expect("lock").clone();
        assert_eq!(ids, vec![Some("rel-1".to_string())]);
    }

    #[tokio::test]
    async fn respects_disable_flags() {
        let mb = MbMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Ok(mb_ok()),
        };
        let cover = CoverMock {
            called: Arc::new(Mutex::new(0usize)),
            release_ids: Arc::new(Mutex::new(Vec::new())),
        };
        let ar = ArMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Ok(ar_ok()),
        };

        let mut settings = Settings::default();
        settings.disable_mb = true;
        settings.disable_accurip = true;

        let input = MetadataFlowInput {
            settings,
            tracks: test_tracks(),
            info_only: false,
            initial_cover_arts: Vec::new(),
        };

        let out = orchestrate_metadata_flow(input, &mb, &cover, &ar).await;
        assert!(out.musicbrainz.is_none());
        assert_eq!(out.accurip_status, AccuDbStatus::Disabled);
        assert!(out.accurip.is_none());
        assert_eq!(*mb.called.lock().expect("lock"), 0);
        assert_eq!(*ar.called.lock().expect("lock"), 0);
        assert_eq!(*cover.called.lock().expect("lock"), 1);
    }

    #[tokio::test]
    async fn falls_back_when_musicbrainz_fails() {
        let cover_ids = Arc::new(Mutex::new(Vec::new()));
        let mb = MbMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Err(MusicBrainzError::NotFound),
        };
        let cover = CoverMock {
            called: Arc::new(Mutex::new(0usize)),
            release_ids: cover_ids.clone(),
        };
        let ar = ArMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Ok(ar_ok()),
        };

        let input = MetadataFlowInput {
            settings: Settings::default(),
            tracks: test_tracks(),
            info_only: false,
            initial_cover_arts: Vec::new(),
        };

        let out = orchestrate_metadata_flow(input, &mb, &cover, &ar).await;
        assert!(out.musicbrainz.is_none());
        assert!(out.warnings.iter().any(|w| w.contains("musicbrainz lookup failed")));
        assert_eq!(cover_ids.lock().expect("lock").clone(), vec![None]);
        assert_eq!(out.accurip_status, AccuDbStatus::Found);
    }

    #[tokio::test]
    async fn handles_discid_failure_and_skips_network_dependent_steps() {
        let mb = MbMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Ok(mb_ok()),
        };
        let cover = CoverMock {
            called: Arc::new(Mutex::new(0usize)),
            release_ids: Arc::new(Mutex::new(Vec::new())),
        };
        let ar = ArMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Ok(ar_ok()),
        };

        let input = MetadataFlowInput {
            settings: Settings::default(),
            tracks: vec![AppTrack {
                number: 1,
                start_lsn: 0,
                end_lsn: 1,
                track_is_data: true,
            }],
            info_only: false,
            initial_cover_arts: Vec::new(),
        };

        let out = orchestrate_metadata_flow(input, &mb, &cover, &ar).await;

        assert!(out.discid.is_none());
        assert!(out.musicbrainz.is_none());
        assert!(out.warnings.iter().any(|w| w.contains("discid computation failed")));
        assert!(out.warnings.iter().any(|w| w.contains("musicbrainz lookup skipped")));
        assert!(out.warnings.iter().any(|w| w.contains("accurip lookup skipped")));
        assert_eq!(out.accurip_status, AccuDbStatus::Error);
        assert_eq!(*mb.called.lock().expect("lock"), 0);
        assert_eq!(*ar.called.lock().expect("lock"), 0);
        assert_eq!(*cover.called.lock().expect("lock"), 1);
    }

    #[test]
    fn writes_per_track_outputs_for_wav_and_flac() {
        let output_root = unique_temp_output_root();

        let mut settings = Settings::default();
        settings.outputs = vec![OutputFormat::Wav, OutputFormat::Flac];
        settings.folder_name_scheme = "{album} [{format}]".to_string();
        settings.track_name_scheme = "{track} - {title}".to_string();
        settings.discnumber = 1;
        settings.totaldiscs = 2;

        let album_meta: HashMap<String, String> = [
            ("album".to_string(), "Example Album".to_string()),
            ("album_artist".to_string(), "Example Artist".to_string()),
            ("date".to_string(), "2024-01-01".to_string()),
        ]
        .into_iter()
        .collect();

        let tracks = vec![
            TrackOutputInput {
                track_number: 1,
                track_meta: [
                    ("track".to_string(), "01".to_string()),
                    ("title".to_string(), "Intro".to_string()),
                    ("artist".to_string(), "Track Artist".to_string()),
                ]
                .into_iter()
                .collect(),
                pcm: sample_pcm(),
            },
            TrackOutputInput {
                track_number: 2,
                track_meta: track_meta("02", "Outro"),
                pcm: sample_pcm(),
            },
        ];

        let result = write_track_outputs(TrackOutputFlowInput {
            settings,
            output_root: output_root.clone(),
            album_meta,
            tracks,
        })
        .expect("dispatch should write wav and flac outputs");

        assert_eq!(result.written_files.len(), 4);

        let expected_paths = vec![
            output_root.join("Example Album [WAV]/01 - Intro.wav"),
            output_root.join("Example Album [WAV]/02 - Outro.wav"),
            output_root.join("Example Album [FLAC]/01 - Intro.flac"),
            output_root.join("Example Album [FLAC]/02 - Outro.flac"),
        ];

        for p in expected_paths {
            assert!(p.exists(), "expected output path to exist: {}", p.display());
        }

        let flac_tag =
            metaflac::Tag::read_from_path(output_root.join("Example Album [FLAC]/01 - Intro.flac"))
                .expect("flac tags should be readable");
        assert_eq!(first_vorbis_value(&flac_tag, "ALBUM").as_deref(), Some("Example Album"));
        assert_eq!(
            first_vorbis_value(&flac_tag, "ALBUMARTIST").as_deref(),
            Some("Example Artist")
        );
        assert_eq!(
            first_vorbis_value(&flac_tag, "ARTIST").as_deref(),
            Some("Track Artist")
        );
        assert_eq!(first_vorbis_value(&flac_tag, "TITLE").as_deref(), Some("Intro"));
        assert_eq!(first_vorbis_value(&flac_tag, "TRACKNUMBER").as_deref(), Some("01"));
        assert_eq!(first_vorbis_value(&flac_tag, "DISCNUMBER").as_deref(), Some("1"));
        assert_eq!(first_vorbis_value(&flac_tag, "DISCTOTAL").as_deref(), Some("2"));

        let cleanup = std::fs::remove_dir_all(&output_root);
        assert!(cleanup.is_ok(), "temporary output root should be removable");
    }

    #[test]
    fn rejects_unsupported_output_formats_in_dispatch() {
        let output_root = unique_temp_output_root();

        let mut settings = Settings::default();
        settings.outputs = vec![OutputFormat::Mp3];

        let album_meta: HashMap<String, String> = [("album".to_string(), "Example Album".to_string())]
            .into_iter()
            .collect();

        let tracks = vec![TrackOutputInput {
            track_number: 1,
            track_meta: track_meta("01", "Intro"),
            pcm: sample_pcm(),
        }];

        let err = write_track_outputs(TrackOutputFlowInput {
            settings,
            output_root,
            album_meta,
            tracks,
        })
        .expect_err("unsupported format should error");

        assert!(matches!(
            err,
            TrackOutputFlowError::UnsupportedOutputFormat(OutputFormat::Mp3)
        ));
    }
}
