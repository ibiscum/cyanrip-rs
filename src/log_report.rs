#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArDbStatus {
    Error,
    NotFound,
    Found,
    Mismatch,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParanoiaStatus {
    Read,
    Verify,
    FixupEdge,
    FixupAtom,
    Scratch,
    Repair,
    Skip,
    Drift,
    Backoff,
    Overlap,
    FixupDropped,
    FixupDuped,
    ReadErr,
    CacheErr,
    Wrote,
    Finished,
}

impl ParanoiaStatus {
    fn as_label(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::Verify => "VERIFY",
            Self::FixupEdge => "FIXUP_EDGE",
            Self::FixupAtom => "FIXUP_ATOM",
            Self::Scratch => "SCRATCH",
            Self::Repair => "REPAIR",
            Self::Skip => "SKIP",
            Self::Drift => "DRIFT",
            Self::Backoff => "BACKOFF",
            Self::Overlap => "OVERLAP",
            Self::FixupDropped => "FIXUP_DROPPED",
            Self::FixupDuped => "FIXUP_DUPED",
            Self::ReadErr => "READERR",
            Self::CacheErr => "CACHEERR",
            Self::Wrote => "WROTE",
            Self::Finished => "FINISHED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartReportInput {
    pub version: String,
    pub vcstag: String,
    pub drive_used: Option<String>,
    pub system_device: String,
    pub device_model: Option<String>,
    pub offset: i32,
    pub over_under_read_frames: i32,
    pub overread_leadinout: bool,
    pub speed: i32,
    pub drive_speed_changeable: bool,
    pub c2_supported: bool,
    pub paranoia_level: i32,
    pub max_paranoia_level: i32,
    pub frame_retries: i32,
    pub hdcd_decoding: bool,
    pub album_art: Vec<String>,
    pub outputs: Vec<String>,
    pub disc_number: Option<String>,
    pub total_discs: Option<String>,
    pub disc_tracks: i32,
    pub tracks_to_rip: Option<Vec<i32>>,
    pub musicbrainz_discid: Option<String>,
    pub release_id: Option<String>,
    pub cddb_id: Option<String>,
    pub disc_mcn: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub accuraterip_status: ArDbStatus,
    pub total_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishReportInput {
    pub tracks_ripped_accurately: Option<(i32, i32)>,
    pub tracks_ripped_partially_accurately: Option<(i32, i32)>,
    pub paranoia_status_counts: Vec<(ParanoiaStatus, u64)>,
    pub ripping_errors: i32,
    pub finished_at: String,
}

pub fn render_start_report(input: &StartReportInput) -> String {
    let mut out = String::new();

    out.push_str(&format!("cyanrip {} ({})\n", input.version, input.vcstag));
    if let Some(drive) = &input.drive_used {
        out.push_str(&format!("Drive used:     {}\n", drive));
    } else {
        out.push_str("Drive used:     error retrieving drive info\n");
    }
    out.push_str(&format!("System device:  {}\n", input.system_device));

    if let Some(model) = &input.device_model {
        out.push_str(&format!("Device model:   {}\n", model));
    }

    out.push_str(&format!(
        "Offset:         {}{} {}\n",
        if input.offset >= 0 { '+' } else { '-' },
        input.offset.abs(),
        if input.offset.abs() == 1 {
            "sample"
        } else {
            "samples"
        }
    ));

    out.push_str(&format!(
        "{}{}{} {}\n",
        if input.over_under_read_frames < 0 {
            "Underread:      "
        } else {
            "Overread:       "
        },
        if input.over_under_read_frames >= 0 {
            '+'
        } else {
            '-'
        },
        input.over_under_read_frames.abs(),
        if input.over_under_read_frames.abs() == 1 {
            "frame"
        } else {
            "frames"
        }
    ));

    out.push_str(&format!(
        "{}{}\n",
        if input.over_under_read_frames < 0 {
            "Underread mode: "
        } else {
            "Overread mode:  "
        },
        if input.overread_leadinout {
            "read in lead-in/lead-out"
        } else {
            "fill with silence in lead-in/lead-out"
        }
    ));

    if input.speed != 0 && input.drive_speed_changeable {
        out.push_str(&format!("Speed:          {}x\n", input.speed));
    } else {
        out.push_str(&format!(
            "Speed:          default ({})\n",
            if input.drive_speed_changeable {
                "changeable"
            } else {
                "unchangeable"
            }
        ));
    }

    out.push_str(&format!(
        "C2 errors:      {} by drive\n",
        if input.c2_supported {
            "supported"
        } else {
            "unsupported"
        }
    ));

    if input.paranoia_level == input.max_paranoia_level {
        out.push_str("Paranoia level: max\n");
    } else if input.paranoia_level == 0 {
        out.push_str("Paranoia level: none\n");
    } else {
        out.push_str(&format!("Paranoia level: {}\n", input.paranoia_level));
    }

    out.push_str(&format!("Frame retries:  {}\n", input.frame_retries));
    out.push_str(&format!(
        "HDCD decoding:  {}\n",
        if input.hdcd_decoding {
            "enabled"
        } else {
            "disabled"
        }
    ));

    if input.album_art.is_empty() {
        out.push_str("Album Art:      none\n");
    } else {
        out.push_str(&format!("Album Art:      {}\n", input.album_art.join(", ")));
    }

    out.push_str(&format!("Outputs:        {}\n", input.outputs.join(", ")));

    if let Some(disc) = &input.disc_number {
        out.push_str(&format!("Disc number:    {}\n", disc));
    }
    if let Some(total) = &input.total_discs {
        out.push_str(&format!("Total discs:    {}\n", total));
    }

    out.push_str(&format!("Disc tracks:    {}\n", input.disc_tracks));

    match &input.tracks_to_rip {
        None => out.push_str("Tracks to rip:  all\n"),
        Some(indices) if indices.is_empty() => out.push_str("Tracks to rip:  none\n"),
        Some(indices) => {
            let joined = indices
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Tracks to rip:  {}\n", joined));
        }
    }

    if let Some(v) = &input.musicbrainz_discid {
        out.push_str(&format!("DiscID:         {}\n", v));
    }
    if let Some(v) = &input.release_id {
        out.push_str(&format!("Release ID:     {}\n", v));
    }
    if let Some(v) = &input.cddb_id {
        out.push_str(&format!("CDDB ID:        {}\n", v));
    }
    if let Some(v) = &input.disc_mcn {
        out.push_str(&format!("Disc MCN:       {}\n", v));
    }
    if let Some(v) = &input.album {
        out.push_str(&format!("Album:          {}\n", v));
    }
    if let Some(v) = &input.album_artist {
        out.push_str(&format!("Album artist:   {}\n", v));
    }

    out.push_str(&format!(
        "AccurateRip:    {}\n",
        match input.accuraterip_status {
            ArDbStatus::Error => "error",
            ArDbStatus::NotFound => "not found",
            ArDbStatus::Found => "found",
            ArDbStatus::Mismatch => "mismatch",
            ArDbStatus::Disabled => "disabled",
        }
    ));

    out.push_str(&format!("Total time:     {}\n", input.total_time));
    out.push('\n');

    out
}

pub fn render_finish_report(input: &FinishReportInput) -> String {
    let mut out = String::new();

    if let Some((ok, total)) = input.tracks_ripped_accurately {
        out.push_str(&format!("Tracks ripped accurately: {}/{}\n", ok, total));
        if let Some((partial, partial_total)) = input.tracks_ripped_partially_accurately {
            out.push_str(&format!(
                "Tracks ripped partially accurately: {}/{}\n",
                partial, partial_total
            ));
        }
        out.push('\n');
    }

    out.push_str("Paranoia status counts:\n");

    let mut has_status = false;
    let pad_to = "FIXUP_DROPPED".len();
    for (status, count) in &input.paranoia_status_counts {
        if *count == 0 {
            continue;
        }
        has_status = true;
        let label = status.as_label();
        let padding = pad_to.saturating_sub(label.len());
        out.push_str("  ");
        out.push_str(label);
        out.push_str(": ");
        for _ in 0..padding {
            out.push(' ');
        }
        out.push_str(&format!("{}\n", count));
    }

    if has_status {
        out.push('\n');
    } else {
        out.push_str("  none\n\n");
    }

    out.push_str(&format!("Ripping errors: {}\n", input.ripping_errors));
    out.push_str(&format!("Ripping finished at {}\n", input.finished_at));

    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn sample_start() -> StartReportInput {
        StartReportInput {
            version: "0.1.0".to_string(),
            vcstag: "abc123".to_string(),
            drive_used: Some("PLEXTOR PX-891SAF (revision 1.00)".to_string()),
            system_device: "/dev/sr0".to_string(),
            device_model: Some("PLEXTOR PX-891SAF".to_string()),
            offset: 6,
            over_under_read_frames: 1,
            overread_leadinout: false,
            speed: 0,
            drive_speed_changeable: true,
            c2_supported: false,
            paranoia_level: 3,
            max_paranoia_level: 3,
            frame_retries: 10,
            hdcd_decoding: false,
            album_art: vec!["Front (From: coverartarchive)".to_string()],
            outputs: vec!["flac".to_string(), "wav".to_string()],
            disc_number: Some("1".to_string()),
            total_discs: Some("2".to_string()),
            disc_tracks: 12,
            tracks_to_rip: Some(vec![1, 2, 4]),
            musicbrainz_discid: Some("mb-discid-123".to_string()),
            release_id: Some("release-xyz".to_string()),
            cddb_id: Some("cddb-789".to_string()),
            disc_mcn: Some("0123456789012".to_string()),
            album: Some("Example Album".to_string()),
            album_artist: Some("Example Artist".to_string()),
            accuraterip_status: ArDbStatus::Found,
            total_time: "00:42:13".to_string(),
        }
    }

    #[test]
    fn start_report_matches_fixture_snapshot() {
        let expected = fs::read_to_string("tests/fixtures/log/report_start_expected.txt")
            .expect("fixture should exist");
        assert_eq!(render_start_report(&sample_start()), expected);
    }

    #[test]
    fn start_report_tracks_to_rip_all_and_none() {
        let mut all = sample_start();
        all.tracks_to_rip = None;
        assert!(render_start_report(&all).contains("Tracks to rip:  all\n"));

        let mut none = sample_start();
        none.tracks_to_rip = Some(Vec::new());
        assert!(render_start_report(&none).contains("Tracks to rip:  none\n"));
    }

    #[test]
    fn finish_report_matches_fixture_snapshot() {
        let expected = fs::read_to_string("tests/fixtures/log/report_finish_expected.txt")
            .expect("fixture should exist");

        let input = FinishReportInput {
            tracks_ripped_accurately: Some((10, 12)),
            tracks_ripped_partially_accurately: Some((1, 2)),
            paranoia_status_counts: vec![
                (ParanoiaStatus::Read, 512),
                (ParanoiaStatus::Verify, 511),
                (ParanoiaStatus::FixupDropped, 2),
            ],
            ripping_errors: 0,
            finished_at: "2026-08-18T20:44:00".to_string(),
        };

        assert_eq!(render_finish_report(&input), expected);
    }

    #[test]
    fn finish_report_no_status_lines_emits_none() {
        let input = FinishReportInput {
            tracks_ripped_accurately: None,
            tracks_ripped_partially_accurately: None,
            paranoia_status_counts: vec![(ParanoiaStatus::Read, 0), (ParanoiaStatus::Verify, 0)],
            ripping_errors: 3,
            finished_at: "2026-08-18T20:50:00".to_string(),
        };

        let rendered = render_finish_report(&input);
        assert!(rendered.contains("Paranoia status counts:\n  none\n\n"));
        assert!(rendered.contains("Ripping errors: 3\n"));
    }
}
