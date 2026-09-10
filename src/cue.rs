use std::collections::BTreeMap;

pub fn frames_to_cue(frames: u32) -> String {
    let min = frames / (75 * 60);
    let sec = (frames - (min * 75 * 60)) / 75;
    let left = frames - (min * 75 * 60) - (sec * 75);
    format!("{min:02}:{sec:02}:{left:02}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueFileType {
    Mp3,
    Binary,
    Wave,
}

impl CueFileType {
    pub fn as_cue_token(self) -> &'static str {
        match self {
            Self::Mp3 => "MP3",
            Self::Binary => "BINARY",
            Self::Wave => "WAVE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CueTrack {
    pub number: u32,
    pub index: u32,
    pub is_data: bool,
    pub preemphasis: bool,
    pub file_path: String,
    pub cue_path: Option<String>,
    pub file_type: CueFileType,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub songwriter: Option<String>,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub isrc: Option<String>,
    pub pregap_lsn: Option<u32>,
    pub start_lsn: u32,
    pub start_lsn_sig: u32,
    pub dropped_pregap_start: Option<u32>,
    pub merged_pregap_end: Option<u32>,
    pub previous_start_lsn_sig: Option<u32>,
    pub postgap_frames: Option<u32>,
    pub flag_dcp: bool,
    pub flag_4ch: bool,
    pub flag_scms: bool,
}

#[derive(Debug, Clone)]
pub struct CueDoc {
    pub meta: BTreeMap<String, String>,
    pub tracks: Vec<CueTrack>,
    pub deemphasis: bool,
    pub force_deemphasis: bool,
}

fn write_meta_line_if_present(
    out: &mut String,
    meta: &BTreeMap<String, String>,
    key: &str,
    fmt: &str,
) {
    if let Some(v) = meta.get(key) {
        out.push_str(&fmt.replace("{}", v));
    }
}

fn relative_file_name(path: &str, cue_path: Option<&str>) -> String {
    let Some(cue_path) = cue_path else {
        return path.to_string();
    };

    let plen = cue_path.rfind('/').map(|i| i + 1).unwrap_or(0);
    if plen > 0 && path.len() >= plen && path[..plen] == cue_path[..plen] {
        path[plen..].to_string()
    } else {
        path.to_string()
    }
}

fn write_track_tags(out: &mut String, t: &CueTrack) {
    if let Some(title) = &t.title {
        out.push_str(&format!("    TITLE \"{}\"\n", title));
    }
    if let Some(artist) = &t.performer {
        out.push_str(&format!("    PERFORMER \"{}\"\n", artist));
    }
    if let Some(songwriter) = &t.songwriter {
        out.push_str(&format!("    SONGWRITER \"{}\"\n", songwriter));
    }
    if let Some(composer) = &t.composer {
        out.push_str(&format!("    COMPOSER \"{}\"\n", composer));
    }
    if let Some(arranger) = &t.arranger {
        out.push_str(&format!("    ARRANGER \"{}\"\n", arranger));
    }
}

fn write_appended_pregap(t: &CueTrack) -> bool {
    t.pregap_lsn.is_some()
        && t.pregap_lsn != Some(t.start_lsn)
        && t.previous_start_lsn_sig.is_some()
        && t.dropped_pregap_start.is_none()
        && t.merged_pregap_end.is_none()
}

pub fn render_cue(doc: &CueDoc) -> String {
    let mut out = String::new();

    write_meta_line_if_present(
        &mut out,
        &doc.meta,
        "musicbrainz_discid",
        "REM MUSICBRAINZ_DISCID \"{}\"\n",
    );
    write_meta_line_if_present(&mut out, &doc.meta, "cddb", "REM DISCID \"{}\"\n");

    for (k, v) in &doc.meta {
        if [
            "musicbrainz_discid",
            "cddb",
            "disc_mcn",
            "album_artist",
            "album",
            "title",
        ]
        .contains(&k.as_str())
        {
            continue;
        }
        out.push_str(&format!("REM {} \"{}\"\n", k.to_ascii_uppercase(), v));
    }

    write_meta_line_if_present(&mut out, &doc.meta, "disc_mcn", "CATALOG {}\n");
    write_meta_line_if_present(&mut out, &doc.meta, "album_artist", "PERFORMER \"{}\"\n");
    write_meta_line_if_present(&mut out, &doc.meta, "album", "TITLE \"{}\"\n");

    for t in &doc.tracks {
        let appended = write_appended_pregap(t);

        if appended {
            out.push_str(&format!("  TRACK {:02} AUDIO\n", t.index));
            write_track_tags(&mut out, t);

            if let (Some(pregap), Some(prev)) = (t.pregap_lsn, t.previous_start_lsn_sig) {
                out.push_str(&format!("    INDEX 00 {}\n", frames_to_cue(pregap - prev)));
            }
        }

        let name = relative_file_name(&t.file_path, t.cue_path.as_deref());
        out.push_str(&format!(
            "FILE \"{}\" {}\n",
            name,
            t.file_type.as_cue_token()
        ));

        if !appended {
            out.push_str(&format!(
                "  TRACK {:02} {}\n",
                t.number,
                if t.is_data { "MODE1/2352" } else { "AUDIO" }
            ));
        }

        if !t.is_data && !appended {
            write_track_tags(&mut out, t);
            if let Some(isrc) = &t.isrc {
                out.push_str(&format!("    ISRC {}\n", isrc));
            }
        }

        let (time_00, time_01) = if let Some(drop_start) = t.dropped_pregap_start {
            (
                frames_to_cue(t.start_lsn_sig - drop_start),
                frames_to_cue(0),
            )
        } else if let Some(merged_end) = t.merged_pregap_end {
            (
                frames_to_cue(0),
                frames_to_cue(merged_end - t.start_lsn_sig),
            )
        } else {
            (String::new(), frames_to_cue(0))
        };

        let mut flags = Vec::new();
        if t.preemphasis && !doc.deemphasis && !doc.force_deemphasis {
            flags.push("PRE");
        }
        if t.flag_dcp {
            flags.push("DCP");
        }
        if t.flag_4ch {
            flags.push("4CH");
        }
        if t.flag_scms {
            flags.push("SCMS");
        }
        if !flags.is_empty() {
            out.push_str(&format!("    FLAGS {}\n", flags.join(" ")));
        }

        if let Some(drop_start) = t.dropped_pregap_start
            && drop_start != t.start_lsn
        {
            out.push_str(&format!("    PREGAP {}\n", time_00));
            out.push_str(&format!("    INDEX 01 {}\n", time_01));
            if let Some(postgap_frames) = t.postgap_frames {
                out.push_str(&format!("    POSTGAP {}\n", frames_to_cue(postgap_frames)));
            }
            continue;
        }

        if t.merged_pregap_end.is_some() {
            out.push_str(&format!("    INDEX 00 {}\n", time_00));
            out.push_str(&format!("    INDEX 01 {}\n", time_01));
        } else {
            out.push_str(&format!("    INDEX 01 {}\n", time_01));
        }

        if let Some(postgap_frames) = t.postgap_frames {
            out.push_str(&format!("    POSTGAP {}\n", frames_to_cue(postgap_frames)));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn read_fixture(path: &str) -> String {
        fs::read_to_string(path).expect("fixture should be readable")
    }

    #[test]
    fn cue_time_conversion() {
        assert_eq!(frames_to_cue(0), "00:00:00");
        assert_eq!(frames_to_cue(75), "00:01:00");
        assert_eq!(frames_to_cue(4500), "01:00:00");
    }

    #[test]
    fn renders_basic_two_audio_tracks_fixture() {
        let mut meta = BTreeMap::new();
        meta.insert("musicbrainz_discid".into(), "kWw6x.CUEFIXTURE".into());
        meta.insert("cddb".into(), "a1230bcd".into());
        meta.insert("disc_mcn".into(), "0123456789012".into());
        meta.insert("album_artist".into(), "Example Artist".into());
        meta.insert("album".into(), "Example Album".into());

        let tracks = vec![
            CueTrack {
                number: 1,
                index: 1,
                is_data: false,
                preemphasis: false,
                file_path: "01 - Intro.flac".into(),
                cue_path: None,
                file_type: CueFileType::Wave,
                title: Some("Intro".into()),
                performer: Some("Example Artist".into()),
                songwriter: None,
                composer: None,
                arranger: None,
                isrc: None,
                pregap_lsn: None,
                start_lsn: 0,
                start_lsn_sig: 0,
                dropped_pregap_start: None,
                merged_pregap_end: None,
                previous_start_lsn_sig: None,
                postgap_frames: None,
                flag_dcp: false,
                flag_4ch: false,
                flag_scms: false,
            },
            CueTrack {
                number: 2,
                index: 2,
                is_data: false,
                preemphasis: false,
                file_path: "02 - Outro.flac".into(),
                cue_path: None,
                file_type: CueFileType::Wave,
                title: Some("Outro".into()),
                performer: Some("Example Artist".into()),
                songwriter: None,
                composer: None,
                arranger: None,
                isrc: None,
                pregap_lsn: None,
                start_lsn: 0,
                start_lsn_sig: 0,
                dropped_pregap_start: None,
                merged_pregap_end: None,
                previous_start_lsn_sig: None,
                postgap_frames: None,
                flag_dcp: false,
                flag_4ch: false,
                flag_scms: false,
            },
        ];

        let rendered = render_cue(&CueDoc {
            meta,
            tracks,
            deemphasis: true,
            force_deemphasis: false,
        });

        let expected = read_fixture("tests/fixtures/cue/basic_audio_two_tracks.cue");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn renders_mixed_audio_data_fixture() {
        let mut meta = BTreeMap::new();
        meta.insert("cddb".into(), "deadbeef".into());
        meta.insert("album_artist".into(), "Example Artist".into());
        meta.insert("album".into(), "Hybrid Disc".into());

        let tracks = vec![
            CueTrack {
                number: 1,
                index: 1,
                is_data: false,
                preemphasis: false,
                file_path: "01 - Audio Track.flac".into(),
                cue_path: None,
                file_type: CueFileType::Wave,
                title: Some("Audio Track".into()),
                performer: Some("Example Artist".into()),
                songwriter: None,
                composer: None,
                arranger: None,
                isrc: None,
                pregap_lsn: None,
                start_lsn: 0,
                start_lsn_sig: 0,
                dropped_pregap_start: None,
                merged_pregap_end: None,
                previous_start_lsn_sig: None,
                postgap_frames: None,
                flag_dcp: false,
                flag_4ch: false,
                flag_scms: false,
            },
            CueTrack {
                number: 2,
                index: 2,
                is_data: true,
                preemphasis: false,
                file_path: "track02.bin".into(),
                cue_path: None,
                file_type: CueFileType::Binary,
                title: None,
                performer: None,
                songwriter: None,
                composer: None,
                arranger: None,
                isrc: None,
                pregap_lsn: None,
                start_lsn: 0,
                start_lsn_sig: 0,
                dropped_pregap_start: None,
                merged_pregap_end: None,
                previous_start_lsn_sig: None,
                postgap_frames: None,
                flag_dcp: false,
                flag_4ch: false,
                flag_scms: false,
            },
        ];

        let rendered = render_cue(&CueDoc {
            meta,
            tracks,
            deemphasis: true,
            force_deemphasis: false,
        });

        let expected = read_fixture("tests/fixtures/cue/mixed_audio_data.cue");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn strips_cue_directory_prefix_from_file_path() {
        let mut meta = BTreeMap::new();
        meta.insert("album".into(), "X".into());

        let tracks = vec![CueTrack {
            number: 1,
            index: 1,
            is_data: false,
            preemphasis: false,
            file_path: "album/01 - Intro.flac".into(),
            cue_path: Some("album/disc.cue".into()),
            file_type: CueFileType::Wave,
            title: Some("Intro".into()),
            performer: None,
            songwriter: None,
            composer: None,
            arranger: None,
            isrc: None,
            pregap_lsn: None,
            start_lsn: 0,
            start_lsn_sig: 0,
            dropped_pregap_start: None,
            merged_pregap_end: None,
            previous_start_lsn_sig: None,
            postgap_frames: None,
            flag_dcp: false,
            flag_4ch: false,
            flag_scms: false,
        }];

        let rendered = render_cue(&CueDoc {
            meta,
            tracks,
            deemphasis: true,
            force_deemphasis: false,
        });

        assert!(rendered.contains("FILE \"01 - Intro.flac\" WAVE"));
    }

    #[test]
    fn emits_preemphasis_and_pregap_lines() {
        let mut meta = BTreeMap::new();
        meta.insert("album".into(), "X".into());

        let tracks = vec![CueTrack {
            number: 1,
            index: 1,
            is_data: false,
            preemphasis: true,
            file_path: "t1.flac".into(),
            cue_path: None,
            file_type: CueFileType::Wave,
            title: Some("T1".into()),
            performer: None,
            songwriter: None,
            composer: None,
            arranger: None,
            isrc: None,
            pregap_lsn: None,
            start_lsn: 200,
            start_lsn_sig: 200,
            dropped_pregap_start: Some(50),
            merged_pregap_end: None,
            previous_start_lsn_sig: None,
            postgap_frames: None,
            flag_dcp: false,
            flag_4ch: false,
            flag_scms: false,
        }];

        let rendered = render_cue(&CueDoc {
            meta,
            tracks,
            deemphasis: false,
            force_deemphasis: false,
        });

        assert!(rendered.contains("    FLAGS PRE\n"));
        assert!(rendered.contains("    PREGAP 00:02:00\n"));
        assert!(rendered.contains("    INDEX 01 00:00:00\n"));
    }

    #[test]
    fn emits_extended_track_directives() {
        let mut meta = BTreeMap::new();
        meta.insert("album".into(), "X".into());

        let tracks = vec![CueTrack {
            number: 1,
            index: 1,
            is_data: false,
            preemphasis: true,
            file_path: "t1.flac".into(),
            cue_path: None,
            file_type: CueFileType::Wave,
            title: Some("T1".into()),
            performer: Some("Artist".into()),
            songwriter: Some("Writer".into()),
            composer: Some("Composer".into()),
            arranger: Some("Arranger".into()),
            isrc: Some("USAAA9912345".into()),
            pregap_lsn: None,
            start_lsn: 0,
            start_lsn_sig: 0,
            dropped_pregap_start: None,
            merged_pregap_end: None,
            previous_start_lsn_sig: None,
            postgap_frames: Some(150),
            flag_dcp: true,
            flag_4ch: true,
            flag_scms: true,
        }];

        let rendered = render_cue(&CueDoc {
            meta,
            tracks,
            deemphasis: false,
            force_deemphasis: false,
        });

        assert!(rendered.contains("    SONGWRITER \"Writer\"\n"));
        assert!(rendered.contains("    COMPOSER \"Composer\"\n"));
        assert!(rendered.contains("    ARRANGER \"Arranger\"\n"));
        assert!(rendered.contains("    FLAGS PRE DCP 4CH SCMS\n"));
        assert!(rendered.contains("    POSTGAP 00:02:00\n"));
    }
}
