use async_trait::async_trait;

use crate::metadata::accurip::{
    AccuDbStatus, AccuRipError, AccuRipLookupResult, AccuRipService, AccuRipTrackInput,
};
use crate::metadata::coverart::{CoverArtError, CoverArtImage, CoverArtService};
use crate::metadata::discid::{DiscTrack, DiscidInfo, compute_discid};
use crate::metadata::musicbrainz::{MusicBrainzError, MusicBrainzReleaseMeta, MusicBrainzService};
use crate::{ReleaseSelection, Settings};

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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
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
}
