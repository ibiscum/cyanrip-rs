use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha1::{Digest, Sha1};

const TOC_TRACK_LIMIT: usize = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscTrack {
    pub number: u8,
    pub start_lsn: i32,
    pub end_lsn: i32,
    pub track_is_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscidInfo {
    pub musicbrainz_discid: String,
    pub cddb: String,
    pub mb_submission_url: String,
    pub last_audio_track_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscidError {
    NoTracks,
    NoAudioTracks,
    InvalidTrackData,
}

pub fn compute_discid(tracks: &[DiscTrack]) -> Result<DiscidInfo, DiscidError> {
    if tracks.is_empty() {
        return Err(DiscidError::NoTracks);
    }

    if tracks.iter().any(|t| t.start_lsn < 0 || t.end_lsn < 0 || t.end_lsn < t.start_lsn) {
        return Err(DiscidError::InvalidTrackData);
    }

    let last_audio_track_index = tracks
        .iter()
        .rposition(|t| !t.track_is_data)
        .ok_or(DiscidError::NoAudioTracks)?;

    let first_track_number = tracks[0].number;
    let last_audio_track_number = tracks[last_audio_track_index].number;
    let last = tracks[last_audio_track_index].end_lsn as u32 + 151;

    let mut sha_input = String::new();
    sha_input.push_str(&format!("{first_track_number:02X}"));
    sha_input.push_str(&format!("{last_audio_track_number:02X}"));
    sha_input.push_str(&format!("{last:08X}"));

    for i in 0..TOC_TRACK_LIMIT {
        let offset = if i <= last_audio_track_index {
            tracks[i].start_lsn as u32 + 150
        } else {
            0
        };
        sha_input.push_str(&format!("{offset:08X}"));
    }

    let digest = Sha1::digest(sha_input.as_bytes());
    let mut discid = STANDARD.encode(digest);
    discid = discid.replace('/', "_").replace('+', ".").replace('=', "-");

    let mut cddb = 0u32;
    for track in &tracks[..=last_audio_track_index] {
        let mut m = (track.start_lsn as u32 + 150) / 75;
        while m > 0 {
            cddb += m % 10;
            m /= 10;
        }
    }

    cddb = ((cddb % 0xff) << 24)
        | ((last / 75 - (tracks[0].start_lsn as u32 + 150) / 75) << 8)
        | (last_audio_track_number as u32);

    let mut mb_submission_url = format!(
        "https://musicbrainz.org/cdtoc/attach?toc={first_track_number}+{last_audio_track_number}+{last}"
    );
    for track in &tracks[..=last_audio_track_index] {
        let offset = track.start_lsn as u32 + 150;
        mb_submission_url.push_str(&format!("+{offset}"));
    }
    mb_submission_url.push_str(&format!("&tracks={}&id={discid}", last_audio_track_index + 1));

    Ok(DiscidInfo {
        musicbrainz_discid: discid,
        cddb: format!("{cddb:08X}"),
        mb_submission_url,
        last_audio_track_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_discid_cddb_and_toc_case_with_trailing_data_track() {
        let tracks = vec![
            DiscTrack {
                number: 1,
                start_lsn: 0,
                end_lsn: 14_999,
                track_is_data: false,
            },
            DiscTrack {
                number: 2,
                start_lsn: 15_000,
                end_lsn: 29_999,
                track_is_data: false,
            },
            DiscTrack {
                number: 3,
                start_lsn: 30_000,
                end_lsn: 44_999,
                track_is_data: true,
            },
        ];

        let out = compute_discid(&tracks).expect("vector should compute");

        assert_eq!(out.musicbrainz_discid, "OMDNUUEF6OVAhAJVAHuIJDSdzdM-");
        assert_eq!(out.cddb, "06019002");
        assert_eq!(
            out.mb_submission_url,
            "https://musicbrainz.org/cdtoc/attach?toc=1+2+30150+150+15150&tracks=2&id=OMDNUUEF6OVAhAJVAHuIJDSdzdM-"
        );
    }

    #[test]
    fn computes_discid_cddb_and_toc_case_with_three_audio_tracks() {
        let tracks = vec![
            DiscTrack {
                number: 1,
                start_lsn: 183,
                end_lsn: 15_000,
                track_is_data: false,
            },
            DiscTrack {
                number: 2,
                start_lsn: 15_183,
                end_lsn: 30_000,
                track_is_data: false,
            },
            DiscTrack {
                number: 3,
                start_lsn: 30_200,
                end_lsn: 45_000,
                track_is_data: false,
            },
        ];

        let out = compute_discid(&tracks).expect("vector should compute");

        assert_eq!(out.musicbrainz_discid, "enjqiWIv9qV0S1_bscl.qi2QxTA-");
        assert_eq!(out.cddb, "12025603");
        assert_eq!(
            out.mb_submission_url,
            "https://musicbrainz.org/cdtoc/attach?toc=1+3+45151+333+15333+30350&tracks=3&id=enjqiWIv9qV0S1_bscl.qi2QxTA-"
        );
    }

    #[test]
    fn rejects_missing_and_invalid_input() {
        let empty = compute_discid(&[]);
        assert_eq!(empty, Err(DiscidError::NoTracks));

        let only_data = vec![DiscTrack {
            number: 1,
            start_lsn: 0,
            end_lsn: 1_000,
            track_is_data: true,
        }];
        assert_eq!(compute_discid(&only_data), Err(DiscidError::NoAudioTracks));

        let invalid = vec![DiscTrack {
            number: 1,
            start_lsn: 2_000,
            end_lsn: 1_999,
            track_is_data: false,
        }];
        assert_eq!(compute_discid(&invalid), Err(DiscidError::InvalidTrackData));
    }
}
