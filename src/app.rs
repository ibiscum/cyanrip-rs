use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
#[cfg(all(target_os = "linux", feature = "cdda"))]
use std::time::Duration;

use crate::audio::flac::write_flac_file;
use crate::audio::process::{TrackProcessingOptions, process_track_pcm};
use crate::audio::wav::write_wav_file;
use crate::audio::{PcmSpec, PcmTrackData, ProcessedPcmTrackData};
use crate::cdda::paranoia::{RetryPolicy, RipEvent, RipState};
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use crate::cdda::reader::CddaReadError;
use crate::cdda::reader::run_track_with_paranoia_heuristics_interruptible;
use crate::cdda::reader::{
    CDDA_FRAME_BYTES, CddaFrameReader, FaultInjectedImageReader, ParanoiaHeuristicConfig,
};
use crate::cue::{CueDoc, CueFileType, CueTrack, render_cue};
use crate::metadata::accurip::{
    AccuDbStatus, AccuRipError, AccuRipLookupResult, AccuRipService, AccuRipTrackInput,
};
use crate::metadata::coverart::{
    CoverArtError, CoverArtImage, CoverArtService, string_is_url,
};
use crate::metadata::discid::{DiscTrack, DiscidInfo, compute_discid};
use crate::metadata::musicbrainz::{
    MusicBrainzError, MusicBrainzReleaseMeta, MusicBrainzService, ReleaseSummary,
};
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use crate::metadata::musicbrainz::ReqwestMusicBrainzHttpClient;
use crate::naming::{
    NamingContext, build_cover_relative_path, build_log_relative_path, build_track_relative_path,
    detect_track_path_collisions, resolve_output_path,
};
use crate::{
    CoverSpecTarget, DriverKind, OutputFormat, ReleaseSelection, Settings, open_dev_kind,
    parse_cover_specs,
};

const DEFAULT_SYNTHETIC_FRAME_COUNT: usize = 32;
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
const FIND_OFFSET_INITIAL_RADIUS_FRAMES: usize = 6;

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use tokio::runtime::Builder as TokioRuntimeBuilder;
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use crate::metadata::accurip::{AccuRipDbEntry, find_accurip_confidence};

#[cfg(all(target_os = "linux", feature = "cdda"))]
fn paranoia_heuristics_for_level(paranoia_level: i32) -> ParanoiaHeuristicConfig {
    crate::cdda::linux_drive::heuristics_for_paranoia_level(paranoia_level)
}

#[cfg(not(all(target_os = "linux", feature = "cdda")))]
fn paranoia_heuristics_for_level(_paranoia_level: i32) -> ParanoiaHeuristicConfig {
    ParanoiaHeuristicConfig::default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunWorkflowError {
    UnsupportedOutputFormat(OutputFormat),
    NotYetImplemented(&'static str),
    Runtime(String),
}

impl std::fmt::Display for RunWorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedOutputFormat(fmt_kind) => {
                write!(f, "output format {fmt_kind:?} is not yet implemented")
            }
            Self::NotYetImplemented(msg) => write!(f, "{msg}"),
            Self::Runtime(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RunWorkflowError {}

fn render_info_only_report(settings: &Settings, drive_used: Option<&str>) -> String {
    let mut lines = Vec::new();
    lines.push(format!("cyanrip-rs {}", env!("CARGO_PKG_VERSION")));
    if let Some(drive) = drive_used {
        lines.push(format!("Drive used:     {drive}"));
    }
    lines.push(format!(
        "System device:  {}",
        settings.dev_path.as_deref().unwrap_or("<default>")
    ));
    let offset = settings.offset;
    let offset_sign = if offset >= 0 { '+' } else { '-' };
    let offset_abs = offset.unsigned_abs();
    let offset_word = if offset_abs == 1 { "sample" } else { "samples" };
    lines.push(format!("Offset:         {offset_sign}{offset_abs} {offset_word}"));
    let ouf = settings.over_under_read_frames;
    let ouf_label = if ouf < 0 { "Underread:      " } else { "Overread:       " };
    let ouf_sign = if ouf >= 0 { '+' } else { '-' };
    let ouf_abs = ouf.unsigned_abs();
    let ouf_word = if ouf_abs == 1 { "frame" } else { "frames" };
    lines.push(format!("{ouf_label}{ouf_sign}{ouf_abs} {ouf_word}"));
    let mode_label = if ouf < 0 { "Underread mode: " } else { "Overread mode:  " };
    let mode_value = if settings.overread_leadinout {
        "read in lead-in/lead-out"
    } else {
        "fill with silence in lead-in/lead-out"
    };
    lines.push(format!("{mode_label}{mode_value}"));
    lines.push("Speed:          default (unchangeable)".to_string());
    let paranoia_str = if settings.paranoia_level == 0 {
        "none".to_string()
    } else if settings.paranoia_level >= 3 {
        "max".to_string()
    } else {
        settings.paranoia_level.to_string()
    };
    lines.push(format!("Paranoia level: {paranoia_str}"));
    lines.push(format!("Frame retries:  {}", settings.max_retries));
    lines.push(format!(
        "HDCD decoding:  {}",
        if settings.decode_hdcd { "enabled" } else { "disabled" }
    ));
    let output_names: Vec<&str> = settings
        .outputs
        .iter()
        .map(|o| match o {
            OutputFormat::Flac => "flac",
            OutputFormat::Wav => "wav",
            OutputFormat::Mp3 => "mp3",
            OutputFormat::Tta => "tta",
            OutputFormat::Opus => "opus",
            OutputFormat::Aac => "aac",
            OutputFormat::AacMp4 => "aac_mp4",
            OutputFormat::Wavpack => "wavpack",
            OutputFormat::Vorbis => "vorbis",
            OutputFormat::Alac => "alac",
            OutputFormat::AlacMp4 => "alac_mp4",
            OutputFormat::OpusMp4 => "opus_mp4",
            OutputFormat::Pcm => "pcm",
        })
        .collect();
    lines.push(format!(
        "Outputs:        {}",
        if output_names.is_empty() {
            "none".to_string()
        } else {
            output_names.join(", ")
        }
    ));
    lines.push(format!(
        "AccurateRip:    {}",
        if settings.disable_accurip { "disabled" } else { "enabled" }
    ));
    lines.join("\n")
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn validate_requested_track_indices_against_toc(
    toc: &[InfoTocEntry],
    requested_indices: &[i32],
) -> Result<(), RunWorkflowError> {
    if requested_indices.is_empty() {
        return Ok(());
    }

    let available: Vec<i32> = toc.iter().map(|t| i32::from(t.number)).collect();
    for idx in requested_indices {
        if *idx <= 0 || !available.contains(idx) {
            return Err(RunWorkflowError::Runtime(format!(
                "Invalid rip index {}, list has {} tracks!",
                idx,
                toc.len()
            )));
        }
    }

    Ok(())
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn format_musicbrainz_release_summary_for_info_mode(idx: usize, release: &ReleaseSummary) -> String {
    let mut suffix = String::new();
    if let Some(country) = release.country.as_deref().filter(|c| !c.trim().is_empty()) {
        suffix.push_str(&format!(" ({country})"));
    }
    if let Some(date) = release.date.as_deref().filter(|d| !d.trim().is_empty()) {
        suffix.push_str(&format!(" ({date})"));
    }
    if release.num_cds > 1 {
        suffix.push_str(&format!(" ({} CDs)", release.num_cds));
    }

    format!(
        "    {} (ID: {}): {}{}",
        idx.saturating_add(1),
        release.id,
        release.album,
        suffix
    )
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn format_musicbrainz_multiple_releases_message(discid: &str, releases: &[ReleaseSummary]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Multiple releases found in database for DiscID {discid}:\n"
    ));
    for (idx, release) in releases.iter().enumerate() {
        out.push_str(&format!(
            "{}\n",
            format_musicbrainz_release_summary_for_info_mode(idx, release)
        ));
    }
    out.push_str(
        "\nPlease specify which release to use by adding the -R argument with an index or ID.",
    );
    out
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn format_msf_from_frames(frames: i32) -> String {
    let frames_non_negative = frames.max(0);
    let total_seconds = frames_non_negative / 75;
    let ff = frames_non_negative % 75;
    let mm = total_seconds / 60;
    let ss = total_seconds % 60;
    format!("{mm:02}:{ss:02}.{ff:02}")
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InfoTocEntry {
    number: u8,
    start_lsn: i32,
    end_lsn: i32,
    track_is_data: bool,
    pregap_lsn: Option<i32>,
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn render_info_only_report_with_toc(
    settings: &Settings,
    drive_used: Option<&str>,
    toc: &[InfoTocEntry],
    discid: Option<(&str, &str, &str)>,
    musicbrainz: Option<&MusicBrainzReleaseMeta>,
) -> String {
    let mut out = render_info_only_report(settings, drive_used);
    let selected_tracks = selected_track_numbers(settings);
    let visible_tracks = filter_info_tracks(toc, &selected_tracks);

    if !toc.is_empty() {
        let total_frames: i32 = toc
            .iter()
            .map(|t| t.end_lsn.saturating_sub(t.start_lsn).saturating_add(1))
            .sum();
        let tracks_to_rip = if settings.rip_indices.is_empty() {
            "all".to_string()
        } else {
            settings
                .rip_indices
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!("\nDisc tracks:    {}\n", toc.len()));
        out.push_str(&format!("Tracks to rip:  {tracks_to_rip}\n"));
        if let Some((discid_str, cddb_str, mb_url)) = discid {
            out.push_str(&format!("\nMusicBrainz URL:\n{mb_url}\n"));
            out.push_str(&format!("DiscID:         {discid_str}\n"));
            if let Some(release) = musicbrainz {
                out.push_str(&format!(
                    "Release ID:     {}\n",
                    release.musicbrainz_albumid
                ));
            }
            out.push_str(&format!("CDDB ID:        {cddb_str}\n"));
        }
        if let Some(release) = musicbrainz {
            out.push_str(&format!("Album:          {}\n", release.album));
            if let Some(album_artist) = release.album_artist.as_deref() {
                out.push_str(&format!("Album artist:   {album_artist}\n"));
            }
            if let Some(discnumber) = release.discnumber {
                out.push_str(&format!("Disc number:    {discnumber}\n"));
            }
            out.push_str(&format!("Total discs:    {}\n", release.totaldiscs));
        }
        out.push_str(&format!("Total time:     {}\n", format_msf_from_frames(total_frames)));

        out.push_str("\nTracks:\n");
        for track in visible_tracks {
            let frames = track.end_lsn.saturating_sub(track.start_lsn).saturating_add(1);
            out.push_str(&format!("Track {} info:\n", track.number));
            out.push_str("  Preemphasis:   none detected\n");
            out.push_str("\n  Properties:\n");
            if track.track_is_data {
                let data_bytes = frames as u64 * 2352;
                let mib = data_bytes as f64 / (1024.0 * 1024.0);
                out.push_str(&format!("    Data bytes:  {data_bytes} ({mib:.2} Mib)\n"));
                out.push_str(&format!("    Frames:      {frames}\n"));
            } else {
                // 588 stereo samples per CDDA frame (2352 bytes / 4 bytes per stereo sample)
                let samples = frames as u64 * 588;
                out.push_str(&format!("    Duration:    {}\n", format_msf_from_frames(frames)));
                out.push_str(&format!("    Samples:     {samples}\n"));
                out.push_str(&format!("    Frames:      {frames}\n"));
                out.push_str("    Sample peak: 0.000000\n");
            }
            if let Some(pregap) = track.pregap_lsn {
                let pregap_frames = track.start_lsn.saturating_sub(pregap);
                out.push_str(&format!(
                    "    Pregap LSN:  {pregap} (duration: {})\n",
                    format_msf_from_frames(pregap_frames)
                ));
            } else {
                out.push_str("    Pregap LSN:  none\n");
            }
            out.push_str(&format!("    Start LSN:   {}\n", track.start_lsn));
            out.push_str(&format!("    End LSN:     {}\n", track.end_lsn));
            out.push_str(&format!(
                "  Accurip:       {}\n",
                if settings.disable_accurip { "disabled" } else { "enabled" }
            ));

            if let (Some(release), Some((discid_str, cddb_str, _))) = (musicbrainz, discid)
                && let Some(track_meta) = release
                    .tracks
                    .get(track.number.saturating_sub(1) as usize)
            {
                out.push_str("\n  Metadata:\n");
                if let Some(mbid) = track_meta.mbid.as_deref() {
                    out.push_str(&format!("    mbid:                {mbid}\n"));
                }
                out.push_str(&format!("    title:               {}\n", track_meta.title));
                if let Some(artist) = track_meta.artist.as_deref() {
                    out.push_str(&format!("    artist:              {artist}\n"));
                }
                out.push_str(&format!("    track:               {}\n", track.number));
                out.push_str(&format!("    tracktotal:          {}\n", toc.len()));
                out.push_str("    disc_mcn:            0000000000000\n");
                out.push_str(&format!("    musicbrainz_discid:  {discid_str}\n"));
                out.push_str(&format!("    cddb:                {cddb_str}\n"));
                if let Some(media) = release.format.as_deref() {
                    out.push_str(&format!("    media:               {media}\n"));
                }
                out.push_str("    comment:             cyanrip 0.9.4-rc2\n");
                if let Some(date) = release.date.as_deref().filter(|d| !d.trim().is_empty()) {
                    out.push_str(&format!("    date:                {date}\n"));
                }
                out.push_str(&format!(
                    "    musicbrainz_albumid: {}\n",
                    release.musicbrainz_albumid
                ));
                out.push_str(&format!("    album:               {}\n", release.album));
                if let Some(barcode) = release.barcode.as_deref() {
                    out.push_str(&format!("    barcode:             {barcode}\n"));
                }
                if let Some(packaging) = release.packaging.as_deref() {
                    out.push_str(&format!("    packaging:           {packaging}\n"));
                }
                if let Some(country) = release.country.as_deref() {
                    out.push_str(&format!("    country:             {country}\n"));
                }
                if let Some(releasestatus) = release.releasestatus.as_deref() {
                    out.push_str(&format!("    releasestatus:       {releasestatus}\n"));
                }
                if let Some(catalognumber) = release.catalognumber.as_deref() {
                    out.push_str(&format!("    catalognumber:       {catalognumber}\n"));
                }
                if let Some(label) = release.label.as_deref() {
                    out.push_str(&format!("    label:               {label}\n"));
                }
                if let Some(album_artist) = release.album_artist.as_deref() {
                    out.push_str(&format!("    album_artist:        {album_artist}\n"));
                }
                out.push_str(&format!("    totaldiscs:          {}\n", release.totaldiscs));
                if let Some(disc) = release.discnumber {
                    out.push_str(&format!("    disc:                {disc}\n"));
                }
                if let Some(format) = release.format.as_deref() {
                    out.push_str(&format!("    format:              {format}\n"));
                }
            }
            out.push('\n');
        }
    }

    out
}

#[cfg(any(test, all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn filter_info_tracks<'a>(toc: &'a [InfoTocEntry], selected_tracks: &[u32]) -> Vec<&'a InfoTocEntry> {
    if selected_tracks.is_empty() {
        return toc.iter().collect();
    }

    toc.iter()
        .filter(|t| selected_tracks.contains(&(t.number as u32)))
        .collect()
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn run_info_only_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    use crate::cdda::linux_drive::{read_drive_hwinfo, read_drive_toc_tracks};

    let hw = read_drive_hwinfo(settings.dev_path.as_deref());
    let drive_used: Option<String> = hw.map(|h| {
        format!("{} {} (revision {})", h.vendor, h.model, h.revision)
    });

    let toc = read_drive_toc_tracks(settings.dev_path.as_deref())
        .map_err(|e| RunWorkflowError::Runtime(format!("TOC read failed: {e:?}")))?;

    let toc_entries: Vec<InfoTocEntry> = toc
        .iter()
        .map(|t| InfoTocEntry {
            number: t.number,
            start_lsn: t.start_lsn,
            end_lsn: t.end_lsn,
            track_is_data: t.track_is_data,
            pregap_lsn: t.pregap_lsn,
        })
        .collect();

    validate_requested_track_indices_against_toc(&toc_entries, &settings.rip_indices)?;

    let discid_parts: Option<(String, String, String)> = if !toc_entries.is_empty() {
        let disc_tracks: Vec<DiscTrack> = toc_entries
            .iter()
            .map(|t| DiscTrack {
                number: t.number,
                start_lsn: t.start_lsn,
                end_lsn: t.end_lsn,
                track_is_data: t.track_is_data,
            })
            .collect();
        compute_discid(&disc_tracks)
            .ok()
            .map(|d| (d.musicbrainz_discid, d.cddb, d.mb_submission_url))
    } else {
        None
    };

    let mut selected_release: Option<MusicBrainzReleaseMeta> = None;

    if !settings.disable_mb && let Some((discid, _, _)) = discid_parts.as_ref() {
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| RunWorkflowError::Runtime(format!("tokio runtime init failed: {e}")))?;

        let service = MusicBrainzService::new(
            ReqwestMusicBrainzHttpClient::default(),
            "https://musicbrainz.org",
            "cyanrip-rs/0.1",
        );

        match runtime.block_on(service.lookup_release(
            discid,
            settings.release.as_ref(),
            settings.discnumber,
            toc_entries.len(),
        )) {
            Ok(release) => selected_release = Some(release),
            Err(MusicBrainzError::NotFound) => {}
            Err(MusicBrainzError::MultipleReleases(candidates)) => {
                return Err(RunWorkflowError::Runtime(
                    format_musicbrainz_multiple_releases_message(discid, &candidates),
                ));
            }
            Err(e) => {
                return Err(RunWorkflowError::Runtime(format!(
                    "musicbrainz lookup failed: {e:?}"
                )));
            }
        }
    }

    let report = render_info_only_report_with_toc(
        settings,
        drive_used.as_deref(),
        &toc_entries,
        discid_parts
            .as_ref()
            .map(|(id, cddb, url)| (id.as_str(), cddb.as_str(), url.as_str())),
        selected_release.as_ref(),
    );
    Ok(report)
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn run_info_only_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    Ok(render_info_only_report(settings, None))
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn render_find_offset_report_header(settings: &Settings) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("cyanrip-rs find-offset mode".to_string());
    lines.push(format!(
        "Device: {}",
        settings.dev_path.as_deref().unwrap_or("<default>")
    ));
    lines.push("AccurateRip: enabled (required)".to_string());
    lines.push("MusicBrainz: disabled".to_string());
    lines.push("CoverArt DB: disabled".to_string());
    lines.push("Eject on success: disabled".to_string());
    lines.push("Offset baseline (samples): 0".to_string());
    lines
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn accurip_v1_checksum(frame: &[u8]) -> u32 {
    let mut sum = 0u32;
    for (i, word) in frame.chunks_exact(4).enumerate() {
        let value = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        sum = sum.wrapping_add(value.wrapping_mul((i as u32).wrapping_add(1)));
    }
    sum
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn accurip_v1_checksum_pcm(pcm: &PcmTrackData, is_first_track: bool, is_last_track: bool) -> u32 {
    // Real AccurateRip v1 checksums trim the first/last 5 frames of the
    // disc's first/last track; reuse the verified fun512::ChecksumCtx logic
    // instead of a naive whole-track weighted sum.
    let nb_samples = (pcm.interleaved_i16_samples.len() / 2) as u32;
    let mut ctx = crate::fun512::ChecksumCtx::new(nb_samples, is_first_track, is_last_track);
    let mut buf = Vec::with_capacity(pcm.interleaved_i16_samples.len().saturating_mul(2));
    for sample in &pcm.interleaved_i16_samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    ctx.process_bytes(&buf);
    ctx.finalize().accurip_checksum_v1
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn track_accurip_confidence_for_pcm(
    track_number: u32,
    pcm: &PcmTrackData,
    metadata_flow: Option<&MetadataFlowResult>,
) -> Option<i32> {
    let mf = metadata_flow?;
    if mf.accurip_status != AccuDbStatus::Found {
        return None;
    }

    let ar = mf.accurip.as_ref()?;
    let idx = track_number.saturating_sub(1) as usize;
    let matches = ar.track_matches.get(idx)?;
    if matches.entries.is_empty() {
        return None;
    }

    let is_first_track = track_number == 1;
    let is_last_track = track_number as usize == ar.track_matches.len();
    let checksum = accurip_v1_checksum_pcm(pcm, is_first_track, is_last_track);
    track_accurip_confidence_for_checksum(track_number, checksum, metadata_flow)
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn track_accurip_confidence_for_checksum(
    track_number: u32,
    checksum: u32,
    metadata_flow: Option<&MetadataFlowResult>,
) -> Option<i32> {
    let mf = metadata_flow?;
    if mf.accurip_status != AccuDbStatus::Found {
        return None;
    }

    let ar = mf.accurip.as_ref()?;
    let idx = track_number.saturating_sub(1) as usize;
    let matches = ar.track_matches.get(idx)?;
    if matches.entries.is_empty() {
        return None;
    }

    Some(find_accurip_confidence(
        AccuDbStatus::Found,
        &matches.entries,
        checksum,
        false,
    ))
}

fn paranoia_run_did_not_converge(state: RipState, events: &[RipEvent]) -> bool {
    if state != RipState::TrackComplete {
        return true;
    }
    events.contains(&RipEvent::RetryLimitReached)
}

#[derive(Debug, Clone)]
struct TrackAcquisitionResult {
    pcm: PcmTrackData,
    #[allow(dead_code)]
    accurip_confidence_from_paranoia_frames: Option<i32>,
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn search_for_offset_in_window(
    window: &[u8],
    entries: &[AccuRipDbEntry],
    max_confidence: i32,
    bytes_radius: usize,
    dir: i32,
    guess: i32,
) -> Option<i32> {
    if window.len() < bytes_radius.saturating_mul(2).saturating_add(CDDA_FRAME_BYTES) {
        return None;
    }

    let center = bytes_radius;
    let check_offset = |offset: i32| -> Option<i32> {
        let byte_off = (offset.unsigned_abs() as usize).saturating_mul(4);
        if byte_off > bytes_radius {
            return None;
        }
        let start = if offset < 0 {
            center.checked_sub(byte_off)?
        } else {
            center.checked_add(byte_off)?
        };
        let end = start.checked_add(CDDA_FRAME_BYTES)?;
        let frame = window.get(start..end)?;
        let checksum = accurip_v1_checksum(frame);
        if checksum == 0 {
            return None;
        }

        let conf = find_accurip_confidence(AccuDbStatus::Found, entries, checksum, true);
        if conf == max_confidence {
            Some(offset)
        } else {
            None
        }
    };

    if guess != 0 && let Some(found) = check_offset(guess) {
        return Some(found);
    }

    let start_byte_off = if dir < 0 { 4 } else { 0 };
    let mut byte_off = start_byte_off;
    while byte_off < bytes_radius {
        let offset = dir.saturating_mul((byte_off / 4) as i32);
        if offset != guess && let Some(found) = check_offset(offset) {
            return Some(found);
        }
        byte_off = byte_off.saturating_add(4);
    }

    None
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn apply_offset_candidate(
    lines: &mut Vec<String>,
    track_number: u32,
    candidate_offset: i32,
    offset_found_samples: &mut i32,
    confidence: &mut i32,
    has_more_tracks: bool,
) {
    let suffix = if has_more_tracks {
        ", trying to confirm with another track"
    } else {
        ""
    };

    if *confidence == 0 {
        *offset_found_samples = candidate_offset;
        *confidence = 1;
        lines.push(format!(
            "Offset of {:+} found in track {}{}",
            candidate_offset, track_number, suffix
        ));
    } else if *offset_found_samples == candidate_offset {
        *confidence = confidence.saturating_add(1);
        lines.push(format!(
            "Offset of {:+} confirmed (confidence: {}) in track {}{}",
            candidate_offset, *confidence, track_number, suffix
        ));
    } else {
        lines.push(format!(
            "New offset of {:+} found at track {}, scrapping old offset of {:+}{}",
            candidate_offset, track_number, *offset_found_samples, suffix
        ));
        *offset_found_samples = candidate_offset;
        *confidence = 1;
    }
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn read_drive_window(
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
) -> Result<Vec<u8>, RunWorkflowError> {
    use crate::cdda::linux_drive::open_linux_physical_drive;

    let mut reader = open_linux_physical_drive(device_path)
        .map_err(|e| RunWorkflowError::Runtime(format!("physical drive open failed: {e:?}")))?;
    reader
        .seek_frame(start_lsn)
        .map_err(|e| RunWorkflowError::Runtime(format!("physical seek failed: {e:?}")))?;

    let mut out = Vec::with_capacity(frame_count.saturating_mul(CDDA_FRAME_BYTES));
    for _ in 0..frame_count {
        let frame = reader
            .read_frame()
            .map_err(|e| RunWorkflowError::Runtime(format!("physical read failed: {e:?}")))?;
        out.extend_from_slice(&frame);
    }
    Ok(out)
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn run_find_offset_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    use crate::cdda::linux_drive::{read_drive_hwinfo, read_drive_toc_tracks};

    let mut lines = vec![
        "Searching for drive offset, enabling AccuRip and disabling MusicBrainz and Cover art fetching..."
            .to_string(),
    ];
    let device = settings.dev_path.as_deref().unwrap_or("/dev/cdrom");
    lines.push(format!("Checking {device} for cdrom..."));

    if let Some(hw) = read_drive_hwinfo(settings.dev_path.as_deref()) {
        lines.push(format!(
            "                CDROM sensed: {} {} {} CD-ROM",
            hw.vendor, hw.model, hw.revision
        ));
    }

    lines.push(String::new());
    lines.push("Opening drive...".to_string());

    let toc = read_drive_toc_tracks(settings.dev_path.as_deref())
        .map_err(|e| RunWorkflowError::Runtime(format!("TOC read failed: {e:?}")))?;
    if toc.is_empty() {
        lines.push("No tracks detected on drive, cannot detect drive offset!".to_string());
        return Ok(lines.join("\n"));
    }

    let disc_tracks: Vec<DiscTrack> = toc
        .iter()
        .map(|t| DiscTrack {
            number: t.number,
            start_lsn: t.start_lsn,
            end_lsn: t.end_lsn,
            track_is_data: t.track_is_data,
        })
        .collect();
    let ar_tracks: Vec<AccuRipTrackInput> = toc
        .iter()
        .map(|t| AccuRipTrackInput {
            number: t.number as u32,
            start_lsn: t.start_lsn as u32,
            end_lsn: t.end_lsn as u32,
            track_is_data: t.track_is_data,
        })
        .collect();

    let discid = compute_discid(&disc_tracks)
        .map_err(|e| RunWorkflowError::Runtime(format!("discid computation failed: {e:?}")))?;
    let cddb_id = u32::from_str_radix(&discid.cddb, 16)
        .map_err(|e| RunWorkflowError::Runtime(format!("cddb parse failed: {e}")))?;

    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| RunWorkflowError::Runtime(format!("tokio runtime init failed: {e}")))?;
    let service = AccuRipService::default();
    let lookup = runtime
        .block_on(service.lookup(&ar_tracks, cddb_id))
        .map_err(|e| RunWorkflowError::Runtime(format!("accurip lookup failed: {e:?}")))?;

    if lookup.status != AccuDbStatus::Found {
        lines.push("No track had AccuRip entry in the AccurateRip database, cannot detect drive offset!".to_string());
        return Ok(lines.join("\n"));
    }

    let mut radius = FIND_OFFSET_INITIAL_RADIUS_FRAMES;
    let mut offset_found_confidence = 0i32;
    let mut offset_found_samples = 0i32;
    let mut had_any_ar = false;
    let mut had_any_eligible_track = false;

    loop {
        let mut had_ar_this_radius = false;
        let mut did_check_this_radius = false;

        for (idx, track) in ar_tracks.iter().enumerate() {
            let Some(matches) = lookup.track_matches.get(idx) else {
                continue;
            };
            if matches.entries.is_empty() {
                continue;
            }
            had_ar_this_radius = true;
            had_any_ar = true;

            let track_len = track.end_lsn.saturating_sub(track.start_lsn) as usize;
            if track_len < (450 + radius) {
                continue;
            }
            did_check_this_radius = true;
            had_any_eligible_track = true;

            lines.push(format!("Loading data for track {}...", idx + 1));

            let start_lsn = (track.start_lsn as i32)
                .saturating_add(450)
                .saturating_sub(radius as i32)
                .max(0);
            let window = read_drive_window(
                settings.dev_path.as_deref(),
                start_lsn,
                2 * radius + 1,
            )?;
            lines.push("Data loaded, searching for offsets...".to_string());
            let bytes_radius = radius.saturating_mul(CDDA_FRAME_BYTES);
            let dir = if offset_found_confidence > 0 && offset_found_samples < 0 {
                -1
            } else {
                1
            };

            let first_guess = if offset_found_confidence > 0 {
                offset_found_samples
            } else {
                0
            };
            let found = search_for_offset_in_window(
                &window,
                &matches.entries,
                matches.max_confidence,
                bytes_radius,
                dir,
                first_guess,
            )
            .or_else(|| {
                search_for_offset_in_window(
                    &window,
                    &matches.entries,
                    matches.max_confidence,
                    bytes_radius,
                    -dir,
                    0,
                )
            });

            if let Some(offset) = found {
                let has_more_tracks = idx + 1 < ar_tracks.len();
                apply_offset_candidate(
                    &mut lines,
                    (idx + 1) as u32,
                    offset,
                    &mut offset_found_samples,
                    &mut offset_found_confidence,
                    has_more_tracks,
                );
            } else {
                let suffix = if idx + 1 < ar_tracks.len() {
                    ", trying another track"
                } else {
                    ""
                };
                lines.push(format!("Nothing found for track {}{}", idx + 1, suffix));
            }
        }

        if offset_found_confidence > 0 {
            break;
        }

        if !had_ar_this_radius || !did_check_this_radius {
            break;
        }

        let next_radius = radius.saturating_mul(2);
        if next_radius <= radius {
            break;
        }

        lines.push(format!(
            "Was not able to find drive offset with a radius of {} frames, trying again with a larger radius...",
            radius
        ));
        radius = next_radius;
    }

    if offset_found_confidence > 0 {
        lines.push(format!(
            "Drive offset of {:+} found (confidence: {})!",
            offset_found_samples, offset_found_confidence
        ));
    } else if !had_any_ar {
        lines.push("No track had AccuRip entry in the AccurateRip database, cannot detect drive offset!".to_string());
    } else if !had_any_eligible_track {
        lines.push("No track was long enough to search for drive offset!".to_string());
    } else {
        lines.push("Was not able to find drive offset!".to_string());
    }

    let _ = &discid;
    Ok(lines.join("\n"))
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn run_find_offset_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    let mut lines = render_find_offset_report_header(settings);
    lines.push(
        "Status: unavailable in this build (requires linux + cdda + backend-libcdio-sys and an inserted audio CD)"
            .to_string(),
    );
    Ok(lines.join("\n"))
}

fn parse_album_metadata_map(raw: Option<&str>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(raw) = raw else {
        return out;
    };

    for part in raw.split(':') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some((k, v)) = p.split_once('=') {
            let key = k.trim();
            let value = v.trim();
            if !key.is_empty() && !value.is_empty() {
                out.insert(key.to_string(), value.to_string());
            }
        }
    }

    out
}

fn parse_track_meta_entry(entry: &str) -> Option<(u32, BTreeMap<String, String>)> {
    let (idx_raw, rest) = entry.split_once('=')?;
    let idx = idx_raw.trim().parse::<u32>().ok()?;
    if idx == 0 {
        return None;
    }

    let mut map = BTreeMap::new();
    for pair in rest.split(':') {
        let p = pair.trim();
        if p.is_empty() {
            continue;
        }
        if let Some((k, v)) = p.split_once('=') {
            let key = k.trim();
            let value = v.trim();
            if !key.is_empty() && !value.is_empty() {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }

    Some((idx, map))
}

fn track_has_preemphasis(track_meta: &HashMap<String, String>) -> bool {
    match track_meta.get("preemphasis") {
        Some(v) => {
            let n = v.trim().to_ascii_lowercase();
            n == "1" || n == "true" || n == "yes" || n == "on"
        }
        None => false,
    }
}

fn parse_boolish(raw: &str) -> Option<bool> {
    let n = raw.trim().to_ascii_lowercase();
    match n.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_u32_field(fields: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    fields.get(key).and_then(|v| v.trim().parse::<u32>().ok())
}

fn parse_bool_field(fields: &BTreeMap<String, String>, key: &str) -> Option<bool> {
    fields.get(key).and_then(|v| parse_boolish(v))
}

fn cue_file_type_from_field(fields: &BTreeMap<String, String>) -> Option<CueFileType> {
    let raw = fields.get("cue_file_type").or_else(|| fields.get("file_type"))?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "wave" | "wav" => Some(CueFileType::Wave),
        "binary" | "bin" => Some(CueFileType::Binary),
        "mp3" => Some(CueFileType::Mp3),
        _ => None,
    }
}

fn track_directive_text(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields
        .get(key)
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn infer_cover_extension_from_source(source: &str) -> Option<String> {
    let clean = source
        .split_once('?')
        .map(|(left, _)| left)
        .unwrap_or(source);

    Path::new(clean)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.trim().to_ascii_lowercase())
        .filter(|e| !e.is_empty())
}

fn initial_cover_arts_from_settings(
    settings: &Settings,
    info_only: bool,
) -> Result<Vec<CoverArtImage>, RunWorkflowError> {
    let parsed = parse_cover_specs(&settings.cover_specs).map_err(RunWorkflowError::Runtime)?;
    let mut out = Vec::new();

    for spec in parsed {
        let CoverSpecTarget::Album(title) = spec.target else {
            continue;
        };

        let source_url = spec.source;
        let is_url = string_is_url(&source_url);
        let data = if info_only || is_url {
            None
        } else {
            Some(fs::read(&source_url).map_err(|e| {
                RunWorkflowError::Runtime(format!(
                    "failed to read cover art source {source_url}: {e}"
                ))
            })?)
        };

        out.push(CoverArtImage {
            title,
            source: Some("CLI".to_string()),
            source_url: source_url.clone(),
            extension: infer_cover_extension_from_source(&source_url),
            data,
            content_type: None,
        });
    }

    Ok(out)
}

fn default_media_value(settings: &Settings) -> &'static str {
    if settings.decode_hdcd {
        "HDCD"
    } else {
        "CD"
    }
}

#[allow(dead_code)]
fn render_cue_only_preview(settings: &Settings) -> String {
    let meta = parse_album_metadata_map(settings.album_metadata.as_deref());
    let mut parsed_tracks = Vec::new();

    for entry in &settings.track_metadata {
        if let Some((idx, fields)) = parse_track_meta_entry(entry) {
            parsed_tracks.push((idx, fields));
        }
    }

    parsed_tracks.sort_by_key(|(idx, _)| *idx);

    let cue_tracks: Vec<CueTrack> = parsed_tracks
        .into_iter()
        .map(|(idx, fields)| {
            let title = fields.get("title").cloned();
            let performer = fields.get("artist").cloned();
            let base_name = title.clone().unwrap_or_else(|| format!("Track {idx:02}"));
            let file_path = format!("{idx:02} - {base_name}.flac");

            let start_lsn = parse_u32_field(&fields, "start_lsn").unwrap_or(0);
            let mut preemphasis = parse_bool_field(&fields, "preemphasis").unwrap_or(false);
            if let Some(v) = parse_bool_field(&fields, "flags_pre") {
                preemphasis = v;
            }
            let flag_dcp = parse_bool_field(&fields, "flag_dcp")
                .or_else(|| parse_bool_field(&fields, "dcp"))
                .unwrap_or(false);
            let flag_4ch = parse_bool_field(&fields, "flag_4ch")
                .or_else(|| parse_bool_field(&fields, "4ch"))
                .unwrap_or(false);
            let flag_scms = parse_bool_field(&fields, "flag_scms")
                .or_else(|| parse_bool_field(&fields, "scms"))
                .unwrap_or(false);

            let mut is_data = parse_bool_field(&fields, "data")
                .or_else(|| parse_bool_field(&fields, "is_data"))
                .unwrap_or(false);
            if parse_bool_field(&fields, "audio") == Some(true) {
                is_data = false;
            }

            let default_file_type = if is_data {
                CueFileType::Binary
            } else {
                CueFileType::Wave
            };

            let mut pregap_lsn = parse_u32_field(&fields, "pregap_lsn");
            let mut start_lsn_sig = parse_u32_field(&fields, "start_lsn_sig").unwrap_or(start_lsn);
            let mut dropped_pregap_start = parse_u32_field(&fields, "dropped_pregap_start");
            let mut merged_pregap_end = parse_u32_field(&fields, "merged_pregap_end");
            let mut previous_start_lsn_sig = parse_u32_field(&fields, "previous_start_lsn_sig");

            if let Some(pregap_start) = parse_u32_field(&fields, "pregap_start_lsn") {
                pregap_lsn = Some(pregap_start);
                if fields.contains_key("pregap_mode") {
                    let mode = fields
                        .get("pregap_mode")
                        .map(|v| v.trim().to_ascii_lowercase())
                        .unwrap_or_default();
                    match mode.as_str() {
                        "drop" => {
                            dropped_pregap_start = Some(pregap_start);
                            start_lsn_sig = start_lsn;
                        }
                        "merge" | "default" => {
                            merged_pregap_end = Some(start_lsn);
                            start_lsn_sig = pregap_start;
                        }
                        "track" | "append" => {
                            if previous_start_lsn_sig.is_none() {
                                previous_start_lsn_sig = Some(start_lsn.saturating_sub(1));
                            }
                            start_lsn_sig = start_lsn;
                        }
                        _ => {}
                    }
                }
            }

            CueTrack {
                number: idx,
                index: idx,
                is_data,
                preemphasis,
                file_path,
                cue_path: None,
                file_type: cue_file_type_from_field(&fields).unwrap_or(default_file_type),
                title,
                performer,
                songwriter: track_directive_text(&fields, "songwriter"),
                composer: track_directive_text(&fields, "composer"),
                arranger: track_directive_text(&fields, "arranger"),
                isrc: fields.get("isrc").cloned(),
                pregap_lsn,
                start_lsn,
                start_lsn_sig,
                dropped_pregap_start,
                merged_pregap_end,
                previous_start_lsn_sig,
                postgap_frames: parse_u32_field(&fields, "postgap_frames")
                    .or_else(|| parse_u32_field(&fields, "postgap")),
                flag_dcp,
                flag_4ch,
                flag_scms,
            }
        })
        .collect();

    let cue = render_cue(&CueDoc {
        meta,
        tracks: cue_tracks,
        deemphasis: settings.deemphasis,
        force_deemphasis: settings.force_deemphasis,
    });

    format!("cyanrip-rs cue-only preview\n{cue}")
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn cue_meta_from_runtime(
    settings: &Settings,
    discid: Option<&DiscidInfo>,
    release: Option<&MusicBrainzReleaseMeta>,
) -> BTreeMap<String, String> {
    let mut meta = BTreeMap::new();
    let user_meta = parse_album_metadata_map(settings.album_metadata.as_deref());

    if let Some(d) = discid {
        meta.insert("musicbrainz_discid".to_string(), d.musicbrainz_discid.clone());
        meta.insert("cddb".to_string(), d.cddb.clone());
    }

    meta.insert("media".to_string(), default_media_value(settings).to_string());
    meta.insert("comment".to_string(), "cyanrip 0.9.4-rc2".to_string());

    if let Some(r) = release {
        if let Some(date) = r.date.as_ref().filter(|d| !d.trim().is_empty()) {
            meta.insert("date".to_string(), date.clone());
        }
        meta.insert(
            "musicbrainz_albumid".to_string(),
            r.musicbrainz_albumid.clone(),
        );
        if let Some(barcode) = r.barcode.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("barcode".to_string(), barcode.clone());
        }
        if let Some(releasestatus) = r.releasestatus.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("releasestatus".to_string(), releasestatus.clone());
        }
        if let Some(catalognumber) = r.catalognumber.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("catalognumber".to_string(), catalognumber.clone());
        }
        if let Some(label) = r.label.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("label".to_string(), label.clone());
        }
        meta.insert("totaldiscs".to_string(), r.totaldiscs.to_string());
        if let Some(discnumber) = r.discnumber {
            meta.insert("disc".to_string(), discnumber.to_string());
        }
        if let Some(format) = r.format.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("format".to_string(), format.clone());
        }
        meta.insert("album".to_string(), r.album.clone());
        if let Some(album_artist) = r.album_artist.as_ref().filter(|v| !v.trim().is_empty()) {
            meta.insert("album_artist".to_string(), album_artist.clone());
        }
    }

    if let Some(album) = user_meta.get("album") {
        meta.insert("album".to_string(), album.clone());
    }
    if let Some(album_artist) = user_meta.get("album_artist") {
        meta.insert("album_artist".to_string(), album_artist.clone());
    }
    if let Some(date) = user_meta.get("date") {
        meta.insert("date".to_string(), date.clone());
    }

    meta
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn cue_tracks_from_runtime(
    settings: &Settings,
    toc: &[InfoTocEntry],
    release: Option<&MusicBrainzReleaseMeta>,
) -> Vec<CueTrack> {
    let (format_suffix, extension) = settings
        .outputs
        .first()
        .copied()
        .and_then(output_format_descriptor)
        .unwrap_or(("FLAC", "flac"));

    let mut album_meta: HashMap<String, String> =
        parse_album_metadata_map(settings.album_metadata.as_deref())
            .into_iter()
            .collect();
    if let Some(r) = release {
        album_meta
            .entry("album".to_string())
            .or_insert_with(|| r.album.clone());
        if let Some(album_artist) = r.album_artist.as_ref().filter(|v| !v.trim().is_empty()) {
            album_meta
                .entry("album_artist".to_string())
                .or_insert_with(|| album_artist.clone());
        }
        if let Some(date) = r.date.as_ref().filter(|v| !v.trim().is_empty()) {
            album_meta
                .entry("date".to_string())
                .or_insert_with(|| date.clone());
        }
        if let Some(releasecomment) = r
            .releasecomment
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            album_meta
                .entry("releasecomment".to_string())
                .or_insert_with(|| releasecomment.clone());
        }
    }

    let mut explicit_track_meta: HashMap<u32, BTreeMap<String, String>> = HashMap::new();
    for entry in &settings.track_metadata {
        if let Some((idx, fields)) = parse_track_meta_entry(entry) {
            explicit_track_meta.insert(idx, fields);
        }
    }

    let naming_ctx = NamingContext {
        sanitize_method: settings.sanitize_method,
        nb_tracks: toc.len(),
    };

    let cue_rel_path = crate::naming::build_cue_relative_path(
        &naming_ctx,
        &album_meta,
        &settings.folder_name_scheme,
        &settings.cue_name_scheme,
        format_suffix,
    )
    .unwrap_or_else(|_| "disc.cue".to_string());

    toc.iter()
        .enumerate()
        .map(|(idx, entry)| {
            let mut title = release
                .and_then(|r| r.tracks.get(entry.number.saturating_sub(1) as usize))
                .map(|t| t.title.clone());
            let mut performer = release
                .and_then(|r| r.tracks.get(entry.number.saturating_sub(1) as usize))
                .and_then(|t| t.artist.clone());

            let start_lsn = entry.start_lsn.max(0) as u32;
            let mut start_lsn_sig = start_lsn;
            let mut pregap_lsn = entry
                .pregap_lsn
                .and_then(|p| if p >= 0 { Some(p as u32) } else { None })
                .filter(|p| *p < start_lsn);
            let mut previous_start_lsn_sig = if idx > 0 {
                Some(toc[idx - 1].start_lsn.max(0) as u32)
            } else {
                None
            };

            let action = settings
                .pregap_action
                .get((entry.number as usize).saturating_sub(1))
                .copied()
                .unwrap_or(crate::PregapAction::Default);

            let mut dropped_pregap_start = None;
            let mut merged_pregap_end = None;

            if let Some(pregap_start) = pregap_lsn {
                match action {
                    crate::PregapAction::Drop => {
                        dropped_pregap_start = Some(pregap_start);
                        start_lsn_sig = start_lsn;
                    }
                    crate::PregapAction::Merge | crate::PregapAction::Default => {
                        merged_pregap_end = Some(start_lsn);
                        start_lsn_sig = pregap_start;
                    }
                    crate::PregapAction::Track => {
                        start_lsn_sig = start_lsn;
                    }
                }
            }

            let mut preemphasis = false;
            let mut isrc = None;
            let mut songwriter = None;
            let mut composer = None;
            let mut arranger = None;
            let mut postgap_frames = None;
            let mut flag_dcp = false;
            let mut flag_4ch = false;
            let mut flag_scms = false;
            let mut file_type = if entry.track_is_data {
                CueFileType::Binary
            } else {
                CueFileType::Wave
            };

            if let Some(fields) = explicit_track_meta.get(&(entry.number as u32)) {
                if let Some(t) = fields.get("title").filter(|v| !v.trim().is_empty()) {
                    title = Some(t.clone());
                }
                if let Some(a) = fields.get("artist").filter(|v| !v.trim().is_empty()) {
                    performer = Some(a.clone());
                }
                if let Some(v) = parse_bool_field(fields, "preemphasis") {
                    preemphasis = v;
                }
                if let Some(v) = parse_bool_field(fields, "flags_pre") {
                    preemphasis = v;
                }
                if let Some(v) = fields.get("isrc").filter(|v| !v.trim().is_empty()) {
                    isrc = Some(v.clone());
                }
                songwriter = track_directive_text(fields, "songwriter");
                composer = track_directive_text(fields, "composer");
                arranger = track_directive_text(fields, "arranger");
                if let Some(v) = parse_u32_field(fields, "start_lsn_sig") {
                    start_lsn_sig = v;
                }
                if let Some(v) = parse_u32_field(fields, "pregap_lsn") {
                    pregap_lsn = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "pregap_start_lsn") {
                    pregap_lsn = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "previous_start_lsn_sig") {
                    previous_start_lsn_sig = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "dropped_pregap_start") {
                    dropped_pregap_start = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "merged_pregap_end") {
                    merged_pregap_end = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "postgap_frames") {
                    postgap_frames = Some(v);
                }
                if let Some(v) = parse_u32_field(fields, "postgap") {
                    postgap_frames = Some(v);
                }
                if let Some(v) = parse_bool_field(fields, "flag_dcp")
                    .or_else(|| parse_bool_field(fields, "dcp"))
                {
                    flag_dcp = v;
                }
                if let Some(v) = parse_bool_field(fields, "flag_4ch")
                    .or_else(|| parse_bool_field(fields, "4ch"))
                {
                    flag_4ch = v;
                }
                if let Some(v) = parse_bool_field(fields, "flag_scms")
                    .or_else(|| parse_bool_field(fields, "scms"))
                {
                    flag_scms = v;
                }
                if let Some(mode) = fields.get("pregap_mode") {
                    match mode.trim().to_ascii_lowercase().as_str() {
                        "drop" => {
                            if let Some(p) = pregap_lsn {
                                dropped_pregap_start = Some(p);
                                merged_pregap_end = None;
                                start_lsn_sig = start_lsn;
                            }
                        }
                        "merge" | "default" => {
                            if let Some(p) = pregap_lsn {
                                dropped_pregap_start = None;
                                merged_pregap_end = Some(start_lsn);
                                start_lsn_sig = p;
                            }
                        }
                        "track" | "append" => {
                            dropped_pregap_start = None;
                            merged_pregap_end = None;
                            start_lsn_sig = start_lsn;
                            if previous_start_lsn_sig.is_none() {
                                previous_start_lsn_sig = Some(start_lsn.saturating_sub(1));
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(v) = cue_file_type_from_field(fields) {
                    file_type = v;
                }
            }

            let mut track_meta = HashMap::new();
            track_meta.insert("track".to_string(), entry.number.to_string());
            if let Some(t) = title.as_ref().filter(|t| !t.trim().is_empty()) {
                track_meta.insert("title".to_string(), t.clone());
            }
            if let Some(a) = performer.as_ref().filter(|a| !a.trim().is_empty()) {
                track_meta.insert("artist".to_string(), a.clone());
            }

            let rel_path = build_track_relative_path(
                &naming_ctx,
                &album_meta,
                &track_meta,
                &settings.folder_name_scheme,
                &settings.track_name_scheme,
                format_suffix,
                extension,
            )
            .unwrap_or_else(|_| {
                format!(
                    "{:02} - {}.{}",
                    entry.number,
                    title.clone().unwrap_or_else(|| format!("Track {:02}", entry.number)),
                    extension
                )
            });

            let cue_path = Some(cue_rel_path.clone());

            CueTrack {
                number: entry.number as u32,
                index: entry.number as u32,
                is_data: entry.track_is_data,
                preemphasis,
                file_path: rel_path,
                cue_path,
                file_type,
                title,
                performer,
                songwriter,
                composer,
                arranger,
                isrc,
                pregap_lsn,
                start_lsn,
                start_lsn_sig,
                dropped_pregap_start,
                merged_pregap_end,
                previous_start_lsn_sig,
                postgap_frames,
                flag_dcp,
                flag_4ch,
                flag_scms,
            }
        })
        .collect()
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn run_cue_only_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    use crate::cdda::linux_drive::{read_drive_hwinfo, read_drive_toc_tracks};

    let hw = read_drive_hwinfo(settings.dev_path.as_deref());
    let drive_used: Option<String> = hw.map(|h| {
        format!("{} {} (revision {})", h.vendor, h.model, h.revision)
    });

    let toc = read_drive_toc_tracks(settings.dev_path.as_deref())
        .map_err(|e| RunWorkflowError::Runtime(format!("TOC read failed: {e:?}")))?;

    let toc_entries: Vec<InfoTocEntry> = toc
        .iter()
        .map(|t| InfoTocEntry {
            number: t.number,
            start_lsn: t.start_lsn,
            end_lsn: t.end_lsn,
            track_is_data: t.track_is_data,
            pregap_lsn: t.pregap_lsn,
        })
        .collect();

    let discid = if !toc_entries.is_empty() {
        let disc_tracks: Vec<DiscTrack> = toc_entries
            .iter()
            .map(|t| DiscTrack {
                number: t.number,
                start_lsn: t.start_lsn,
                end_lsn: t.end_lsn,
                track_is_data: t.track_is_data,
            })
            .collect();
        Some(
            compute_discid(&disc_tracks)
                .map_err(|e| RunWorkflowError::Runtime(format!("discid computation failed: {e:?}")))?,
        )
    } else {
        None
    };

    let mut selected_release: Option<MusicBrainzReleaseMeta> = None;
    if !settings.disable_mb && let Some(d) = discid.as_ref() {
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| RunWorkflowError::Runtime(format!("tokio runtime init failed: {e}")))?;

        let service = MusicBrainzService::new(
            ReqwestMusicBrainzHttpClient::default(),
            "https://musicbrainz.org",
            "cyanrip-rs/0.1",
        );

        match runtime.block_on(service.lookup_release(
            &d.musicbrainz_discid,
            settings.release.as_ref(),
            settings.discnumber,
            toc_entries.len(),
        )) {
            Ok(release) => selected_release = Some(release),
            Err(MusicBrainzError::NotFound) => {}
            Err(MusicBrainzError::MultipleReleases(candidates)) => {
                return Err(RunWorkflowError::Runtime(
                    format_musicbrainz_multiple_releases_message(&d.musicbrainz_discid, &candidates),
                ));
            }
            Err(e) => {
                return Err(RunWorkflowError::Runtime(format!(
                    "musicbrainz lookup failed: {e:?}"
                )));
            }
        }
    }

    let mut report_settings = settings.clone();
    if report_settings.outputs.is_empty() {
        report_settings.outputs.push(OutputFormat::Flac);
    }

    let report = render_info_only_report_with_toc(
        &report_settings,
        drive_used.as_deref(),
        &toc_entries,
        discid
            .as_ref()
            .map(|d| (d.musicbrainz_discid.as_str(), d.cddb.as_str(), d.mb_submission_url.as_str())),
        selected_release.as_ref(),
    );

    let cue = render_cue(&CueDoc {
        meta: cue_meta_from_runtime(settings, discid.as_ref(), selected_release.as_ref()),
        tracks: cue_tracks_from_runtime(settings, &toc_entries, selected_release.as_ref()),
        deemphasis: settings.deemphasis,
        force_deemphasis: settings.force_deemphasis,
    });

    let mut out = String::new();
    if let Some(release) = selected_release.as_ref() {
        if let Some(album_artist) = release.album_artist.as_deref().filter(|v| !v.trim().is_empty()) {
            out.push_str(&format!(
                "Found MusicBrainz release: {} - {}\n",
                release.album, album_artist
            ));
        }
    }
    out.push_str(&report);
    out.push_str("\n\n");
    out.push_str(&cue);
    Ok(out)
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn run_cue_only_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    Ok(render_cue_only_preview(settings))
}

fn env_var_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let n = v.trim().to_ascii_lowercase();
            n == "1" || n == "true" || n == "yes" || n == "on"
        }
        Err(_) => false,
    }
}

fn default_runtime_output_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(target_os = "linux")]
fn read_proc_status_value_kib(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if !line.starts_with(key) {
            continue;
        }
        let mut parts = line.split_whitespace();
        let _label = parts.next()?;
        let value = parts.next()?.parse::<u64>().ok()?;
        return Some(value);
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_proc_status_value_kib(_key: &str) -> Option<u64> {
    None
}

fn current_rss_kib() -> Option<u64> {
    read_proc_status_value_kib("VmRSS:")
}

fn peak_rss_kib() -> Option<u64> {
    read_proc_status_value_kib("VmHWM:")
}

fn format_kib_as_mib(kib: u64) -> String {
    format!("{:.2} MiB", kib as f64 / 1024.0)
}

fn format_bytes_as_mib(bytes: usize) -> String {
    format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TrackBenchmark {
    track_number: u32,
    elapsed_ms: u128,
    pcm_bytes: usize,
    rss_kib_after: Option<u64>,
}

fn configured_output_root(settings: &Settings) -> PathBuf {
    if let Some(cli_root) = settings.output_root.as_deref()
        && !cli_root.trim().is_empty()
    {
        return PathBuf::from(cli_root);
    }

    match std::env::var("CYANRIP_RS_OUTPUT_ROOT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => default_runtime_output_root(),
    }
}

fn synthetic_track_pcm() -> PcmTrackData {
    let mut samples = Vec::with_capacity(48_000 * 2);
    for i in 0..48_000 {
        let phase = (i % 128) as i16 - 64;
        let val = phase * 120;
        samples.push(val);
        samples.push(-val);
    }

    PcmTrackData {
        spec: PcmSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: samples,
    }
}

fn build_synthetic_frames(frame_count: usize) -> Vec<Vec<u8>> {
    let mut frames = Vec::with_capacity(frame_count);
    for i in 0..frame_count {
        let mut frame = vec![0u8; CDDA_FRAME_BYTES];
        for (j, b) in frame.iter_mut().enumerate() {
            *b = ((i * 31 + j * 17) & 0xFF) as u8;
        }
        frames.push(frame);
    }
    frames
}

fn configured_frame_count() -> usize {
    match std::env::var("CYANRIP_RS_FRAME_COUNT") {
        Ok(v) => v
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_SYNTHETIC_FRAME_COUNT),
        Err(_) => DEFAULT_SYNTHETIC_FRAME_COUNT,
    }
}

fn acquire_track_pcm_from_reader<R: CddaFrameReader>(
    reader: &mut R,
    start_lsn: i32,
    frame_count: usize,
) -> Result<PcmTrackData, RunWorkflowError> {
    reader
        .seek_frame(start_lsn)
        .map_err(|e| RunWorkflowError::Runtime(format!("frame seek failed: {e:?}")))?;

    let mut samples = Vec::new();
    for _ in 0..frame_count {
        let frame = reader
            .read_frame()
            .map_err(|e| RunWorkflowError::Runtime(format!("frame read failed: {e:?}")))?;

        let mut off = 0usize;
        while off + 1 < frame.len() {
            let v = i16::from_le_bytes([frame[off], frame[off + 1]]);
            samples.push(v);
            off += 2;
        }
    }

    Ok(PcmTrackData {
        spec: PcmSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: samples,
    })
}

fn acquire_track_pcm_from_image_reader(frame_count: usize) -> Result<PcmTrackData, RunWorkflowError> {
    let frames = build_synthetic_frames(frame_count);
    let mut reader = FaultInjectedImageReader::new(frames);
    acquire_track_pcm_from_reader(&mut reader, 0, frame_count)
}

fn synthetic_track_pcm_from_image_reader(frame_count: usize) -> Result<PcmTrackData, RunWorkflowError> {
    acquire_track_pcm_from_image_reader(frame_count)
}

fn synthetic_track_pcm_for_source() -> Result<PcmTrackData, RunWorkflowError> {
    let source = std::env::var("CYANRIP_RS_SYNTHETIC_SOURCE").unwrap_or_else(|_| "tone".to_string());
    if source.eq_ignore_ascii_case("image-reader") {
        return synthetic_track_pcm_from_image_reader(configured_frame_count());
    }

    Ok(synthetic_track_pcm())
}

fn render_synthetic_full_rip(settings: &Settings) -> Result<String, RunWorkflowError> {
    let output_root = configured_output_root(settings);
    let cli_cover_arts = initial_cover_arts_from_settings(settings, false)?;

    let mut album_meta: HashMap<String, String> = parse_album_metadata_map(settings.album_metadata.as_deref())
        .into_iter()
        .collect();

    album_meta
        .entry("album".to_string())
        .or_insert_with(|| "Synthetic Album".to_string());
    album_meta
        .entry("album_artist".to_string())
        .or_insert_with(|| "Synthetic Artist".to_string());
    album_meta
        .entry("media".to_string())
        .or_insert_with(|| default_media_value(settings).to_string());

    let mut track_plan: Vec<(u32, HashMap<String, String>)> = Vec::new();
    for entry in &settings.track_metadata {
        if let Some((idx, fields)) = parse_track_meta_entry(entry) {
            let mut track_meta: HashMap<String, String> = fields.into_iter().collect();
            track_meta
                .entry("track".to_string())
                .or_insert_with(|| format!("{idx:02}"));
            track_meta
                .entry("title".to_string())
                .or_insert_with(|| format!("Synthetic Track {idx:02}"));
            track_plan.push((idx, track_meta));
        }
    }

    if track_plan.is_empty() {
        let mut track_meta = HashMap::new();
        track_meta.insert("track".to_string(), "01".to_string());
        track_meta.insert("title".to_string(), "Synthetic Track 01".to_string());
        track_plan.push((1, track_meta));
    }

    let naming_track_count = track_plan.len();
    warn_track_path_collisions_for_formats(settings, &album_meta, &track_plan, naming_track_count)
        .map_err(|e| RunWorkflowError::Runtime(format!("synthetic full-rip failed: {e}")))?;

    let mut written_files = Vec::new();
    for (track_number, track_meta) in &track_plan {
        let pcm = synthetic_track_pcm_for_source()?;
        let result = write_track_outputs_with_naming_tracks(
            TrackOutputFlowInput {
                settings: settings.clone(),
                output_root: output_root.clone(),
                album_meta: album_meta.clone(),
                cover_arts: cli_cover_arts.clone(),
                tracks: vec![TrackOutputInput {
                    track_number: *track_number,
                    track_meta: track_meta.clone(),
                    pcm,
                }],
            },
            naming_track_count,
            false,
            None,
        )
        .map_err(|e| RunWorkflowError::Runtime(format!("synthetic full-rip failed: {e}")))?;
        written_files.extend(result.written_files);
    }

    let mut out = String::new();
    out.push_str("cyanrip-rs synthetic full-rip mode\n");
    out.push_str(&format!("Output root: {}\n", output_root.display()));
    out.push_str(&format!("Written files: {}\n", written_files.len()));
    for file in &written_files {
        out.push_str(&format!("FILE {}\n", file.absolute_path.display()));
    }

    write_runtime_log_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &out,
    )?;

    let synthetic_track_meta_map: HashMap<u32, HashMap<String, String>> = track_plan
        .into_iter()
        .collect();
    write_runtime_cue_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &written_files,
        &synthetic_track_meta_map,
    )?;

    write_runtime_cover_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &cli_cover_arts,
    )?;

    Ok(out)
}

fn write_runtime_log_files(
    settings: &Settings,
    output_root: &Path,
    album_meta: &HashMap<String, String>,
    naming_track_count: usize,
    content: &str,
) -> Result<(), RunWorkflowError> {
    let naming_ctx = NamingContext {
        sanitize_method: settings.sanitize_method,
        nb_tracks: naming_track_count.max(1),
    };

    for fmt_kind in &settings.outputs {
        let (format_suffix, _extension) = output_format_descriptor(*fmt_kind)
            .ok_or(RunWorkflowError::UnsupportedOutputFormat(*fmt_kind))?;

        let relative_path = build_log_relative_path(
            &naming_ctx,
            album_meta,
            &settings.folder_name_scheme,
            &settings.log_name_scheme,
            format_suffix,
        )
        .map_err(|e| {
            RunWorkflowError::Runtime(format!("failed to resolve log output path for {fmt_kind:?}: {e}"))
        })?;

        let absolute_path = resolve_output_path(output_root, &relative_path, true)
            .map_err(|e| {
                RunWorkflowError::Runtime(format!(
                    "failed to resolve log output path {}: {e}",
                    output_root.join(&relative_path).display()
                ))
            })?;

        fs::write(&absolute_path, content).map_err(|e| {
            RunWorkflowError::Runtime(format!(
                "failed to write log file {}: {e}",
                absolute_path.display()
            ))
        })?;
    }

    Ok(())
}

fn cue_doc_meta_from_album_meta(album_meta: &HashMap<String, String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in album_meta {
        if value.trim().is_empty() {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    out
}

fn cue_track_from_written_file(
    file: &TrackOutputFile,
    cue_relative_path: &str,
    track_meta: &HashMap<String, String>,
) -> CueTrack {
    CueTrack {
        number: file.track_number,
        index: file.track_number,
        is_data: false,
        preemphasis: track_has_preemphasis(track_meta),
        file_path: file.relative_path.to_string_lossy().to_string(),
        cue_path: Some(cue_relative_path.to_string()),
        file_type: CueFileType::Wave,
        title: track_meta.get("title").cloned(),
        performer: track_meta.get("artist").cloned(),
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
    }
}

fn write_runtime_cue_files(
    settings: &Settings,
    output_root: &Path,
    album_meta: &HashMap<String, String>,
    naming_track_count: usize,
    written_files: &[TrackOutputFile],
    track_meta_map: &HashMap<u32, HashMap<String, String>>,
) -> Result<(), RunWorkflowError> {
    let naming_ctx = NamingContext {
        sanitize_method: settings.sanitize_method,
        nb_tracks: naming_track_count.max(1),
    };

    for fmt_kind in &settings.outputs {
        let (format_suffix, _extension) = output_format_descriptor(*fmt_kind)
            .ok_or(RunWorkflowError::UnsupportedOutputFormat(*fmt_kind))?;

        let cue_relative_path = crate::naming::build_cue_relative_path(
            &naming_ctx,
            album_meta,
            &settings.folder_name_scheme,
            &settings.cue_name_scheme,
            format_suffix,
        )
        .map_err(|e| {
            RunWorkflowError::Runtime(format!("failed to resolve cue output path for {fmt_kind:?}: {e}"))
        })?;

        let cue_absolute_path = resolve_output_path(output_root, &cue_relative_path, true).map_err(|e| {
            RunWorkflowError::Runtime(format!(
                "failed to resolve cue output path {}: {e}",
                output_root.join(&cue_relative_path).display()
            ))
        })?;

        let mut tracks_for_format: Vec<&TrackOutputFile> = written_files
            .iter()
            .filter(|f| f.output_format == *fmt_kind)
            .collect();
        tracks_for_format.sort_by_key(|f| f.track_number);

        if tracks_for_format.is_empty() {
            continue;
        }

        let cue_tracks: Vec<CueTrack> = tracks_for_format
            .into_iter()
            .map(|file| {
                let track_meta = track_meta_map
                    .get(&file.track_number)
                    .cloned()
                    .unwrap_or_else(|| {
                        let mut fallback = HashMap::new();
                        fallback.insert("track".to_string(), format!("{:02}", file.track_number));
                        fallback.insert("title".to_string(), format!("Track {:02}", file.track_number));
                        fallback
                    });
                cue_track_from_written_file(file, &cue_relative_path, &track_meta)
            })
            .collect();

        let cue_doc = CueDoc {
            meta: cue_doc_meta_from_album_meta(album_meta),
            tracks: cue_tracks,
            deemphasis: settings.deemphasis,
            force_deemphasis: settings.force_deemphasis,
        };
        let cue_text = render_cue(&cue_doc);

        fs::write(&cue_absolute_path, cue_text).map_err(|e| {
            RunWorkflowError::Runtime(format!(
                "failed to write cue file {}: {e}",
                cue_absolute_path.display()
            ))
        })?;
    }

    Ok(())
}

fn write_runtime_cover_files(
    settings: &Settings,
    output_root: &Path,
    album_meta: &HashMap<String, String>,
    naming_track_count: usize,
    cover_arts: &[CoverArtImage],
) -> Result<(), RunWorkflowError> {
    if cover_arts.is_empty() {
        return Ok(());
    }

    let naming_ctx = NamingContext {
        sanitize_method: settings.sanitize_method,
        nb_tracks: naming_track_count.max(1),
    };

    for fmt_kind in &settings.outputs {
        let (format_suffix, _extension) = output_format_descriptor(*fmt_kind)
            .ok_or(RunWorkflowError::UnsupportedOutputFormat(*fmt_kind))?;

        for art in cover_arts {
            if !art.title.eq_ignore_ascii_case("front") && !art.title.eq_ignore_ascii_case("back") {
                continue;
            }
            let Some(data) = art.data.as_ref() else {
                continue;
            };

            let relative_path = build_cover_relative_path(
                &naming_ctx,
                album_meta,
                &settings.folder_name_scheme,
                format_suffix,
                &art.title,
                art.extension.as_deref(),
            )
            .map_err(|e| {
                RunWorkflowError::Runtime(format!(
                    "failed to resolve cover output path for {fmt_kind:?}: {e}"
                ))
            })?;

            let absolute_path = resolve_output_path(output_root, &relative_path, true).map_err(|e| {
                RunWorkflowError::Runtime(format!(
                    "failed to resolve cover output path {}: {e}",
                    output_root.join(&relative_path).display()
                ))
            })?;

            fs::write(&absolute_path, data).map_err(|e| {
                RunWorkflowError::Runtime(format!(
                    "failed to write cover file {}: {e}",
                    absolute_path.display()
                ))
            })?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FullRipSource {
    Image,
    Physical,
}

fn should_attempt_eject_on_success(settings: &Settings, source: FullRipSource) -> bool {
    settings.eject_on_success_rip && source == FullRipSource::Physical
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn maybe_eject_after_success(settings: &Settings, source: FullRipSource) {
    if !should_attempt_eject_on_success(settings, source) {
        return;
    }

    use crate::cdda::linux_drive::eject_linux_drive_if_supported;

    let device_path = settings.dev_path.as_deref().or(Some("/dev/cdrom"));
    let _ = eject_linux_drive_if_supported(device_path);
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn maybe_eject_after_success(settings: &Settings, source: FullRipSource) {
    let _ = should_attempt_eject_on_success(settings, source);
}

fn full_rip_source_from_settings(settings: &Settings) -> FullRipSource {
    match settings.dev_path.as_deref() {
        Some(path) => match open_dev_kind(path) {
            DriverKind::BinCue | DriverKind::Cue | DriverKind::Nrg | DriverKind::CdrDao => {
                FullRipSource::Image
            }
            DriverKind::Unknown => FullRipSource::Physical,
        },
        None => {
            #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
            {
                FullRipSource::Physical
            }
            #[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
            {
                FullRipSource::Image
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "cdda"))]
fn acquire_track_pcm_from_physical_reader(
    settings: &Settings,
    frame_count: usize,
    start_lsn: i32,
    track_number: u32,
    _metadata_flow: Option<&MetadataFlowResult>,
) -> Result<TrackAcquisitionResult, RunWorkflowError> {
    use crate::cdda::linux_drive::{open_linux_physical_drive, run_paranoia_on_linux_drive_with_defaults_for_level};

    let device_path = settings.dev_path.as_deref().or(Some("/dev/cdrom"));

    if settings.paranoia_level > 0 {
        println!(
            "Ripping (paranoia level {}): track {}, {} frame(s)...",
            settings.paranoia_level, track_number, frame_count
        );

        let mut retry_policy = if settings.ripping_retries > 0 {
            RetryPolicy::new(
                settings.ripping_retries as u32,
                settings.max_retries.max(1) as u32,
            )
        } else {
            RetryPolicy::disabled()
        };

        let run = run_paranoia_on_linux_drive_with_defaults_for_level(
            device_path,
            settings.paranoia_level,
            start_lsn,
            frame_count,
            settings.max_retries.max(0) as u32,
            &mut retry_policy,
            |_pass, pass_frames| {
                let mut acc = 0u32;
                for frame in pass_frames {
                    for b in frame {
                        acc = acc.wrapping_add(*b as u32);
                    }
                }
                acc
            },
            {
                let start = Instant::now();
                let mut last_update = start.checked_sub(Duration::from_secs(1)).unwrap_or(start);
                move |done: usize, total: usize| {
                    let now = Instant::now();
                    let should_update = done == 1
                        || done >= total
                        || now.duration_since(last_update) >= Duration::from_millis(400);
                    if !should_update {
                        return;
                    }
                    let progress = (done as f64 / total.max(1) as f64) * 100.0;
                    let elapsed = now.duration_since(start).as_secs_f64().max(0.001);
                    let eta_min = if done >= total {
                        0.0
                    } else {
                        let rate_fps = done as f64 / elapsed;
                        let remaining_frames = total.saturating_sub(done) as f64;
                        (remaining_frames / rate_fps) / 60.0
                    };
                    print!(
                        "\rRipping (paranoia): track {}, progress - {:.2}%, ETA - {}   ",
                        track_number, progress, format_eta_min_sec(eta_min)
                    );
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    last_update = now;
                }
            },
        )
        .map_err(|e| RunWorkflowError::Runtime(format!("physical paranoia run failed: {e:?}")))?;

        println!();

        if run.state == RipState::Failed {
            return Err(RunWorkflowError::Runtime(format!(
                "physical paranoia run did not complete track: {:?}",
                run.state
            )));
        }

        // The paranoia engine already performs jitter/error correction while
        // reading, so its frames are the final PCM source directly -- there is
        // no separate "direct read" step to fall back to (unlike a precheck).
        let Some(frames) = run.final_frames.as_ref() else {
            return Err(RunWorkflowError::Runtime(format!(
                "physical paranoia run produced no finalized frames for track {}: state {:?}, retry-limit-reached {}",
                track_number,
                run.state,
                run.events.contains(&RipEvent::RetryLimitReached)
            )));
        };

        if paranoia_run_did_not_converge(run.state, &run.events) {
            log::warn!(
                "paranoia read for track {} did not fully converge (state {:?}); using best-effort corrected frames",
                track_number, run.state
            );
        }

        println!(
            "Paranoia read complete for track {} (passes: {}, state: {:?})",
            track_number,
            run.passes,
            run.state,
        );

        return Ok(TrackAcquisitionResult {
            pcm: pcm_from_cdda_frames(frames),
            // AccurateRip confidence requires the drive-offset-corrected pcm,
            // which is only available to the caller after cropping; see
            // apply_drive_offset_crop and track_accurip_confidence_for_pcm.
            accurip_confidence_from_paranoia_frames: None,
        });
    }

    println!(
        "Paranoia disabled; starting direct drive read for track {}...",
        track_number
    );

    let mut reader = open_linux_physical_drive(device_path).map_err(|e| {
        RunWorkflowError::Runtime(format!("physical drive open failed: {e:?}"))
    })?;
    reader
        .seek_frame(start_lsn)
        .map_err(|e| RunWorkflowError::Runtime(format!("frame seek failed: {e:?}")))?;

    let mut samples = Vec::new();
    let start = Instant::now();
    let mut last_update = start.checked_sub(Duration::from_secs(1)).unwrap_or(start);

    for frame_idx in 0..frame_count {
        let frame = reader
            .read_frame()
            .map_err(|e| RunWorkflowError::Runtime(format!("frame read failed: {e:?}")))?;

        let mut off = 0usize;
        while off + 1 < frame.len() {
            let v = i16::from_le_bytes([frame[off], frame[off + 1]]);
            samples.push(v);
            off += 2;
        }

        let done = frame_idx.saturating_add(1);
        let now = Instant::now();
        let should_update = done == 1
            || done == frame_count
            || now.duration_since(last_update) >= Duration::from_millis(400);

        if should_update {
            let progress = (done as f64 / frame_count.max(1) as f64) * 100.0;
            let elapsed = now.duration_since(start).as_secs_f64().max(0.001);
            let eta_min = if done >= frame_count {
                0.0
            } else {
                let rate_fps = done as f64 / elapsed;
                let remaining_frames = frame_count.saturating_sub(done) as f64;
                (remaining_frames / rate_fps) / 60.0
            };

            print!(
                "\rRipping          : track {}, progress - {:.2}%, ETA - {}   ", track_number, progress, format_eta_min_sec(eta_min));
                let _ = std::io::Write::flush(&mut std::io::stdout());
            last_update = now;
        }
    }

    println!();

    Ok(TrackAcquisitionResult {
        pcm: PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: samples,
        },
        accurip_confidence_from_paranoia_frames: None,
    })
}

#[cfg(not(all(target_os = "linux", feature = "cdda")))]
fn acquire_track_pcm_from_physical_reader(
    _settings: &Settings,
    _frame_count: usize,
    _start_lsn: i32,
    _track_number: u32,
    _metadata_flow: Option<&MetadataFlowResult>,
) -> Result<TrackAcquisitionResult, RunWorkflowError> {
    Err(RunWorkflowError::Runtime(
        "physical drive reader requires linux + cdda feature support".to_string(),
    ))
}

fn selected_track_numbers(settings: &Settings) -> Vec<u32> {
    settings
        .rip_indices
        .iter()
        .filter(|n| **n > 0)
        .map(|n| *n as u32)
        .collect()
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn resolve_requested_audio_tracks(
    requested_track_numbers: &[u32],
    available_audio_tracks: &[u32],
    total_tracks_on_disc: usize,
) -> Result<Vec<u32>, RunWorkflowError> {
    if requested_track_numbers.is_empty() {
        return Ok(available_audio_tracks.to_vec());
    }

    if let Some(invalid) = requested_track_numbers
        .iter()
        .find(|n| !available_audio_tracks.contains(n))
    {
        return Err(RunWorkflowError::Runtime(format!(
            "Invalid rip index {}, list has {} tracks!",
            invalid, total_tracks_on_disc
        )));
    }

    Ok(requested_track_numbers.to_vec())
}

fn apply_offset_frame_adjustment(boundary: TrackBoundary, settings: &Settings) -> TrackBoundary {
    // Mirror upstream coarse frame shifting used by setup_track_lsn.
    let mut first_frame = boundary.start_lsn;
    let mut last_frame = boundary
        .start_lsn
        .saturating_add(boundary.frame_count.saturating_sub(1) as i32);
    let extra_frames = settings.over_under_read_frames;
    let sign: i32 = if extra_frames < 0 {
        -1
    } else if extra_frames > 0 {
        1
    } else {
        0
    };

    let shift = (extra_frames.unsigned_abs() as i32).saturating_sub(1).max(0);
    first_frame = first_frame.saturating_add(sign.saturating_mul(shift));
    last_frame = last_frame.saturating_add(sign.saturating_mul(shift));

    if sign < 0 {
        first_frame = first_frame.saturating_sub(1);
    }
    if sign > 0 {
        last_frame = last_frame.saturating_add(1);
    }

    let frame_count = if last_frame >= first_frame {
        last_frame.saturating_sub(first_frame).saturating_add(1) as usize
    } else {
        0
    };

    TrackBoundary {
        track_number: boundary.track_number,
        start_lsn: first_frame,
        frame_count,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrackReadPlan {
    track_number: u32,
    start_lsn: i32,
    frame_count: usize,
    read_start_lsn: i32,
    read_frame_count: usize,
    silence_before_frames: usize,
    silence_after_frames: usize,
    // Pre-offset-adjustment TOC boundary, needed to crop the offset-shifted
    // (frame-rounded) read window down to the disc's true sample-accurate track length.
    original_start_lsn: i32,
    original_frame_count: usize,
}

fn plan_track_read(
    boundary: TrackBoundary,
    settings: &Settings,
    disc_start_lsn: i32,
    disc_end_lsn: i32,
) -> TrackReadPlan {
    let adjusted = apply_offset_frame_adjustment(boundary, settings);
    let mut first = adjusted.start_lsn;
    let mut last = adjusted
        .start_lsn
        .saturating_add(adjusted.frame_count.saturating_sub(1) as i32);

    let (silence_before_frames, silence_after_frames) = if settings.overread_leadinout {
        (0usize, 0usize)
    } else {
        let before = disc_start_lsn.saturating_sub(first).max(0) as usize;
        let after = last.saturating_sub(disc_end_lsn).max(0) as usize;
        first = first.saturating_add(before as i32);
        last = last.saturating_sub(after as i32);
        (before, after)
    };

    let read_frame_count = if last >= first {
        last.saturating_sub(first).saturating_add(1) as usize
    } else {
        0
    };
    let frame_count = read_frame_count
        .saturating_add(silence_before_frames)
        .saturating_add(silence_after_frames);

    TrackReadPlan {
        track_number: adjusted.track_number,
        start_lsn: adjusted.start_lsn,
        frame_count,
        read_start_lsn: first,
        read_frame_count,
        silence_before_frames,
        silence_after_frames,
        original_start_lsn: boundary.start_lsn,
        original_frame_count: boundary.frame_count,
    }
}

fn build_track_read_plans(
    settings: &Settings,
    boundaries: &[TrackBoundary],
    disc_start_lsn: i32,
    disc_end_lsn: i32,
) -> Vec<TrackReadPlan> {
    boundaries
        .iter()
        .copied()
        .map(|b| plan_track_read(b, settings, disc_start_lsn, disc_end_lsn))
        .collect()
}

fn add_silence_padding(
    mut pcm: PcmTrackData,
    silence_before_frames: usize,
    silence_after_frames: usize,
) -> PcmTrackData {
    let samples_per_frame = CDDA_FRAME_BYTES / std::mem::size_of::<i16>();
    let before_samples = silence_before_frames.saturating_mul(samples_per_frame);
    let after_samples = silence_after_frames.saturating_mul(samples_per_frame);
    if before_samples == 0 && after_samples == 0 {
        return pcm;
    }

    let mut out = Vec::with_capacity(
        before_samples
            .saturating_add(pcm.interleaved_i16_samples.len())
            .saturating_add(after_samples),
    );
    out.extend(std::iter::repeat_n(0i16, before_samples));
    out.extend_from_slice(&pcm.interleaved_i16_samples);
    out.extend(std::iter::repeat_n(0i16, after_samples));
    pcm.interleaved_i16_samples = out;
    pcm
}

/// Applies the fine (sub-frame) sample-accurate drive-offset correction that
/// `apply_offset_frame_adjustment` cannot express: that function only grows the
/// read window by whole CD frames, so callers must crop the result back down
/// to the disc's true track length at the exact sample offset. Without this
/// step, ripped audio is a whole frame longer/misaligned and never matches
/// AccurateRip checksums (or upstream cyanrip's/EAC's output) for any offset
/// that isn't a multiple of 588 samples.
fn apply_drive_offset_crop(mut pcm: PcmTrackData, plan: &TrackReadPlan, offset_samples: i32) -> PcmTrackData {
    const I16_PER_CD_FRAME: i64 = (CDDA_FRAME_BYTES / 2) as i64;

    let crop_start = (plan.original_start_lsn as i64 - plan.start_lsn as i64)
        .saturating_mul(I16_PER_CD_FRAME)
        .saturating_add((offset_samples as i64).saturating_mul(2));
    let crop_len = (plan.original_frame_count as i64).saturating_mul(I16_PER_CD_FRAME);

    let total = pcm.interleaved_i16_samples.len() as i64;
    let start = crop_start.clamp(0, total);
    let end = start.saturating_add(crop_len).clamp(0, total);

    let mut cropped: Vec<i16> = if start < end {
        pcm.interleaved_i16_samples[start as usize..end as usize].to_vec()
    } else {
        Vec::new()
    };
    // Defensive: keep the output at the disc's true track length even if the
    // computed offset window ran past the available (padded) sample buffer.
    if (cropped.len() as i64) < crop_len {
        cropped.resize(crop_len.max(0) as usize, 0);
    }
    pcm.interleaved_i16_samples = cropped;
    pcm
}

fn pcm_from_cdda_frames(frames: &[Vec<u8>]) -> PcmTrackData {
    let mut samples = Vec::new();
    for frame in frames {
        let mut off = 0usize;
        while off + 1 < frame.len() {
            let v = i16::from_le_bytes([frame[off], frame[off + 1]]);
            samples.push(v);
            off += 2;
        }
    }

    PcmTrackData {
        spec: PcmSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
        },
        interleaved_i16_samples: samples,
    }
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn map_toc_preflight_error(err: &CddaReadError) -> RunWorkflowError {
    let msg = match err {
        CddaReadError::ReadFailed(msg) | CddaReadError::SeekFailed(msg) => msg,
    };
    let lower = msg.to_ascii_lowercase();
    if lower.contains("no medium") || lower.contains("invalid toc values") {
        return RunWorkflowError::Runtime(
            "Drive has no readable audio medium inserted".to_string(),
        );
    }
    RunWorkflowError::Runtime(format!("TOC read failed: {err:?}"))
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn resolve_physical_track_boundaries(
    settings: &Settings,
    requested_track_numbers: &[u32],
) -> Result<(Vec<TrackBoundary>, i32, i32), RunWorkflowError> {
    use crate::cdda::linux_drive::read_drive_toc_tracks;

    let device_path = settings.dev_path.as_deref().or(Some("/dev/cdrom"));

    let toc = read_drive_toc_tracks(device_path).map_err(|e| map_toc_preflight_error(&e))?;

    let available_audio_tracks: Vec<u32> = toc
        .iter()
        .filter(|t| !t.track_is_data)
        .map(|t| t.number as u32)
        .collect();

    let wanted = resolve_requested_audio_tracks(
        requested_track_numbers,
        &available_audio_tracks,
        toc.len(),
    )?;

    if wanted.is_empty() {
        return Err(RunWorkflowError::Runtime(
            "no matching audio tracks selected for ripping".to_string(),
        ));
    }

    let mut boundaries = Vec::new();
    let mut disc_end_lsn = 0i32;
    for track in &toc {
        let track_number = track.number as u32;
        disc_end_lsn = disc_end_lsn.max(track.end_lsn);
        if track.track_is_data || !wanted.contains(&track_number) {
            continue;
        }

        let base_frames = track
            .end_lsn
            .saturating_sub(track.start_lsn)
            .saturating_add(1)
            .max(0) as usize;

        boundaries.push(TrackBoundary {
            track_number,
            start_lsn: track.start_lsn,
            frame_count: base_frames,
        });
    }

    if boundaries.is_empty() {
        return Err(RunWorkflowError::Runtime(
            "selected tracks were not available as audio tracks".to_string(),
        ));
    }

    Ok((boundaries, 0, disc_end_lsn))
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn resolve_physical_track_boundaries(
    _settings: &Settings,
    _requested_track_numbers: &[u32],
) -> Result<(Vec<TrackBoundary>, i32, i32), RunWorkflowError> {
    Err(RunWorkflowError::Runtime(
        "physical TOC resolution requires linux + cdda + backend-libcdio-sys".to_string(),
    ))
}

fn track_meta_map_from_settings(settings: &Settings) -> HashMap<u32, HashMap<String, String>> {
    let mut out: HashMap<u32, HashMap<String, String>> = HashMap::new();
    for entry in &settings.track_metadata {
        if let Some((idx, fields)) = parse_track_meta_entry(entry) {
            out.insert(idx, fields.into_iter().collect());
        }
    }
    out
}

fn track_meta_for_number(
    track_number: u32,
    track_meta_map: &HashMap<u32, HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut out = track_meta_map
        .get(&track_number)
        .cloned()
        .unwrap_or_default();
    out.entry("track".to_string())
        .or_insert_with(|| format!("{track_number:02}"));
    out.entry("title".to_string())
        .or_insert_with(|| format!("Track {track_number:02}"));
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrackBoundary {
    track_number: u32,
    start_lsn: i32,
    frame_count: usize,
}

fn image_disc_range_from_boundaries(boundaries: &[TrackBoundary]) -> (i32, i32) {
    let start = boundaries.iter().map(|b| b.start_lsn.max(0)).min().unwrap_or(0);
    let end = boundaries
        .iter()
        .map(|b| {
            b.start_lsn
                .saturating_add(b.frame_count.saturating_sub(1) as i32)
        })
        .max()
        .unwrap_or(0)
        .max(start);
    (start, end)
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn app_tracks_from_image_boundaries(boundaries: &[TrackBoundary]) -> Vec<AppTrack> {
    boundaries
        .iter()
        .map(|b| AppTrack {
            number: b.track_number as u8,
            start_lsn: b.start_lsn,
            end_lsn: b.start_lsn.saturating_add(b.frame_count.saturating_sub(1) as i32),
            track_is_data: false,
        })
        .collect()
}

fn parse_i32_meta(map: &HashMap<String, String>, key: &str) -> Option<i32> {
    map.get(key).and_then(|v| v.trim().parse::<i32>().ok())
}

fn format_duration_from_frames(frame_count: usize) -> String {
    let minutes = frame_count / (75 * 60);
    let seconds = (frame_count / 75) % 60;
    let centis = ((frame_count % 75) * 100) / 75;
    format!("{minutes:02}:{seconds:02}.{centis:02}")
}

/// Formats a fractional-minutes ETA (e.g. 1.75) as whole "M:SS".
fn format_eta_min_sec(eta_min: f64) -> String {
    let total_secs = (eta_min.max(0.0) * 60.0).round() as u64;
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

fn samples_from_frames(frame_count: usize) -> usize {
    frame_count.saturating_mul(588)
}

fn parse_usize_meta(map: &HashMap<String, String>, key: &str) -> Option<usize> {
    map.get(key)
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
}

fn parse_image_toc_env() -> HashMap<u32, (i32, usize)> {
    let raw = match std::env::var("CYANRIP_RS_IMAGE_TOC") {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let mut out = HashMap::new();
    for part in raw.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }

        let Some((track_s, range_s)) = p.split_once(':') else {
            continue;
        };
        let Ok(track_number) = track_s.trim().parse::<u32>() else {
            continue;
        };
        if track_number == 0 {
            continue;
        }

        let Some((start_s, end_s)) = range_s.split_once('-') else {
            continue;
        };
        let Ok(start_lsn) = start_s.trim().parse::<i32>() else {
            continue;
        };
        let Ok(end_lsn) = end_s.trim().parse::<i32>() else {
            continue;
        };
        if start_lsn < 0 || end_lsn < start_lsn {
            continue;
        }

        let frame_count = (end_lsn - start_lsn + 1) as usize;
        out.insert(track_number, (start_lsn, frame_count));
    }

    out
}

fn parse_cue_index_01_lsn(line: &str) -> Option<i32> {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    if !parts[0].eq_ignore_ascii_case("INDEX") || parts[1] != "01" {
        return None;
    }

    let mut ts = parts[2].split(':');
    let mm = ts.next()?.trim().parse::<i32>().ok()?;
    let ss = ts.next()?.trim().parse::<i32>().ok()?;
    let ff = ts.next()?.trim().parse::<i32>().ok()?;
    if ts.next().is_some() {
        return None;
    }
    if mm < 0 || !(0..60).contains(&ss) || !(0..75).contains(&ff) {
        return None;
    }

    Some(mm.saturating_mul(60).saturating_mul(75) + ss.saturating_mul(75) + ff)
}

fn parse_track_number(line: &str) -> Option<u32> {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 || !parts[0].eq_ignore_ascii_case("TRACK") {
        return None;
    }
    parts[1].parse::<u32>().ok().filter(|n| *n > 0)
}

fn parse_cue_track_starts(cue_text: &str) -> Vec<(u32, i32)> {
    let mut starts = Vec::new();
    let mut current_track: Option<u32> = None;

    for line in cue_text.lines() {
        if let Some(track_number) = parse_track_number(line) {
            current_track = Some(track_number);
            continue;
        }

        if let Some(start_lsn) = parse_cue_index_01_lsn(line)
            && let Some(track_number) = current_track
            && starts.iter().all(|(n, _)| *n != track_number)
        {
            starts.push((track_number, start_lsn));
        }
    }

    starts.sort_by_key(|(n, _)| *n);
    starts
}

fn parse_image_toc_from_cue_file(
    cue_path: &Path,
    default_frame_count: usize,
) -> HashMap<u32, (i32, usize)> {
    let text = match fs::read_to_string(cue_path) {
        Ok(t) => t,
        Err(_) => return HashMap::new(),
    };

    let starts = parse_cue_track_starts(&text);
    if starts.is_empty() {
        return HashMap::new();
    }

    let mut out = HashMap::new();
    for idx in 0..starts.len() {
        let (track_number, start_lsn) = starts[idx];
        let frame_count = if let Some((_, next_start_lsn)) = starts.get(idx + 1) {
            if *next_start_lsn > start_lsn {
                (*next_start_lsn - start_lsn) as usize
            } else {
                default_frame_count
            }
        } else {
            default_frame_count
        };
        out.insert(track_number, (start_lsn, frame_count));
    }

    out
}

fn image_toc_overrides_from_settings(
    settings: &Settings,
    default_frame_count: usize,
) -> HashMap<u32, (i32, usize)> {
    let mut out = HashMap::new();

    if let Some(dev_path) = settings.dev_path.as_deref()
        && open_dev_kind(dev_path) == DriverKind::Cue
    {
        let cue_overrides = parse_image_toc_from_cue_file(Path::new(dev_path), default_frame_count);
        out.extend(cue_overrides);
    }

    for (track_number, range) in parse_image_toc_env() {
        out.insert(track_number, range);
    }

    out
}

fn resolve_track_boundaries(
    track_numbers: &[u32],
    track_meta_map: &HashMap<u32, HashMap<String, String>>,
    default_frame_count: usize,
    image_toc_overrides: Option<&HashMap<u32, (i32, usize)>>,
) -> Vec<TrackBoundary> {
    track_numbers
        .iter()
        .map(|track_number| {
            let default_start = ((*track_number as usize)
                .saturating_sub(1)
                .saturating_mul(default_frame_count)) as i32;

            if let Some((start_lsn, frame_count)) = image_toc_overrides
                .and_then(|m| m.get(track_number).copied())
            {
                return TrackBoundary {
                    track_number: *track_number,
                    start_lsn,
                    frame_count,
                };
            }

            let meta = track_meta_map.get(track_number);
            let start_lsn = meta
                .and_then(|m| parse_i32_meta(m, "start_lsn"))
                .filter(|n| *n >= 0)
                .unwrap_or(default_start);

            let frame_count = if let Some(m) = meta {
                if let Some(frames) = parse_usize_meta(m, "frames") {
                    frames
                } else if let Some(end_lsn) = parse_i32_meta(m, "end_lsn") {
                    if end_lsn >= start_lsn {
                        (end_lsn - start_lsn + 1) as usize
                    } else {
                        default_frame_count
                    }
                } else {
                    default_frame_count
                }
            } else {
                default_frame_count
            };

            TrackBoundary {
                track_number: *track_number,
                start_lsn,
                frame_count,
            }
        })
        .collect()
}

fn acquire_tracks_pcm_from_image_reader(
    settings: &Settings,
    plans: &[TrackReadPlan],
) -> Result<Vec<(u32, TrackAcquisitionResult)>, RunWorkflowError> {
    let max_frame_end = plans
        .iter()
        .map(|b| (b.read_start_lsn.max(0) as usize).saturating_add(b.read_frame_count))
        .max()
        .unwrap_or(DEFAULT_SYNTHETIC_FRAME_COUNT);
    let total_frames = max_frame_end.max(DEFAULT_SYNTHETIC_FRAME_COUNT);
    let frames = build_synthetic_frames(total_frames);
    let mut reader = FaultInjectedImageReader::new(frames);

    let mut out = Vec::with_capacity(plans.len());
    for plan in plans {
        let start_lsn = plan.read_start_lsn;
        let frame_count = plan.read_frame_count;
        let mut paranoia_fallback_pcm: Option<PcmTrackData> = None;

        if settings.paranoia_level > 0 && frame_count > 0 {
            let mut retry_policy = if settings.ripping_retries > 0 {
                RetryPolicy::new(
                    settings.ripping_retries as u32,
                    settings.max_retries.max(1) as u32,
                )
            } else {
                RetryPolicy::disabled()
            };

            let heuristics = paranoia_heuristics_for_level(settings.paranoia_level);
            let run = run_track_with_paranoia_heuristics_interruptible(
                &mut reader,
                start_lsn,
                frame_count,
                settings.max_retries.max(0) as u32,
                &mut retry_policy,
                heuristics,
                || false,
                |_pass, pass_frames| {
                    let mut acc = 0u32;
                    for frame in pass_frames {
                        for b in frame {
                            acc = acc.wrapping_add(*b as u32);
                        }
                    }
                    acc
                },
                |_done, _total| {},
            )
            .map_err(|e| {
                RunWorkflowError::Runtime(format!("image paranoia run failed: {e:?}"))
            })?;

            if paranoia_run_did_not_converge(run.state, &run.events) {
                if let Some(frames) = run.final_frames.as_ref() {
                    paranoia_fallback_pcm = Some(pcm_from_cdda_frames(frames));
                } else {
                    return Err(RunWorkflowError::Runtime(format!(
                        "image paranoia run did not complete track: {:?}",
                        run.state
                    )));
                }
            }
        }

        let pcm = if let Some(pcm) = paranoia_fallback_pcm {
            pcm
        } else if frame_count > 0 {
            acquire_track_pcm_from_reader(&mut reader, start_lsn, frame_count)?
        } else {
            PcmTrackData {
                spec: PcmSpec {
                    channels: 2,
                    sample_rate: 44_100,
                    bits_per_sample: 16,
                },
                interleaved_i16_samples: Vec::new(),
            }
        };
        let pcm = add_silence_padding(pcm, plan.silence_before_frames, plan.silence_after_frames);
        let pcm = apply_drive_offset_crop(pcm, plan, settings.offset);
        out.push((
            plan.track_number,
            TrackAcquisitionResult {
                pcm,
                accurip_confidence_from_paranoia_frames: None,
            },
        ));
    }

    Ok(out)
}

fn acquire_tracks_pcm_from_physical_reader(
    settings: &Settings,
    plans: &[TrackReadPlan],
    metadata_flow: Option<&MetadataFlowResult>,
) -> Result<Vec<(u32, TrackAcquisitionResult)>, RunWorkflowError> {
    let mut out = Vec::with_capacity(plans.len());
    for plan in plans {
        let start_lsn = plan.read_start_lsn;
        let frame_count = plan.read_frame_count;

        let acquired = if frame_count > 0 {
            acquire_track_pcm_from_physical_reader(
                settings,
                frame_count,
                start_lsn,
                plan.track_number,
                metadata_flow,
            )?
        } else {
            TrackAcquisitionResult {
                pcm: PcmTrackData {
                    spec: PcmSpec {
                        channels: 2,
                        sample_rate: 44_100,
                        bits_per_sample: 16,
                    },
                    interleaved_i16_samples: Vec::new(),
                },
                accurip_confidence_from_paranoia_frames: None,
            }
        };
        let mut acquired = acquired;
        acquired.pcm = add_silence_padding(
            acquired.pcm,
            plan.silence_before_frames,
            plan.silence_after_frames,
        );
        acquired.pcm = apply_drive_offset_crop(acquired.pcm, plan, settings.offset);
        // The confidence above was computed on the raw, pre-crop read; the
        // corrected pcm must be re-checked against AccuRip by the caller.
        acquired.accurip_confidence_from_paranoia_frames = None;
        out.push((plan.track_number, acquired));
    }
    Ok(out)
}

fn run_full_rip_from_selected_source(settings: &Settings) -> Result<String, RunWorkflowError> {
    let source = full_rip_source_from_settings(settings);
    let requested_track_numbers = selected_track_numbers(settings);
    let default_frame_count = configured_frame_count();
    let track_meta_map = track_meta_map_from_settings(settings);
    let cli_cover_arts = initial_cover_arts_from_settings(settings, false)?;
    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    let mut track_meta_map = track_meta_map;
    let image_toc_overrides = match source {
        FullRipSource::Image => {
            let map = image_toc_overrides_from_settings(settings, default_frame_count);
            if map.is_empty() {
                None
            } else {
                Some(map)
            }
        }
        FullRipSource::Physical => None,
    };
    let (boundaries, disc_start_lsn, disc_end_lsn) = match source {
        FullRipSource::Image => {
            let image_track_numbers = if requested_track_numbers.is_empty() {
                vec![1]
            } else {
                requested_track_numbers.clone()
            };
            let boundaries = resolve_track_boundaries(
                &image_track_numbers,
                &track_meta_map,
                default_frame_count,
                image_toc_overrides.as_ref(),
            );
            let (disc_start_lsn, disc_end_lsn) = image_disc_range_from_boundaries(&boundaries);
            (boundaries, disc_start_lsn, disc_end_lsn)
        }
        FullRipSource::Physical => {
            resolve_physical_track_boundaries(settings, &requested_track_numbers)?
        }
    };
    let read_plans = build_track_read_plans(settings, &boundaries, disc_start_lsn, disc_end_lsn);

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    let mut metadata_flow: Option<MetadataFlowResult> = None;

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    {
        use crate::metadata::coverart::ReqwestCoverArtHttpClient;

        let app_tracks: Vec<AppTrack> = match source {
            FullRipSource::Physical => {
                use crate::cdda::linux_drive::read_drive_toc_tracks;
                let device_path = settings.dev_path.as_deref().or(Some("/dev/cdrom"));
                read_drive_toc_tracks(device_path)
                    .map(|toc| {
                        toc.iter()
                            .map(|t| AppTrack {
                                number: t.number,
                                start_lsn: t.start_lsn,
                                end_lsn: t.end_lsn,
                                track_is_data: t.track_is_data,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            FullRipSource::Image => {
                // Images have no independent TOC source, so MusicBrainz
                // track-count matching is best-effort against only the
                // tracks selected for this rip.
                app_tracks_from_image_boundaries(&boundaries)
            }
        };

        if !app_tracks.is_empty() {
            let runtime = TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| RunWorkflowError::Runtime(format!("tokio runtime init failed: {e}")))?;

            let mb_service = MusicBrainzService::new(
                ReqwestMusicBrainzHttpClient::default(),
                "https://musicbrainz.org",
                "cyanrip-rs/0.1",
            );
            let cover_service = CoverArtService::new(
                ReqwestCoverArtHttpClient::default(),
                "http://coverartarchive.org/release",
                "cyanrip-rs/0.1",
            );
            let ar_service = AccuRipService::default();
            let initial_cover_arts = initial_cover_arts_from_settings(settings, false)?;

            let mf = runtime.block_on(orchestrate_metadata_flow(
                MetadataFlowInput {
                    settings: settings.clone(),
                    tracks: app_tracks,
                    info_only: false,
                    initial_cover_arts: initial_cover_arts,
                },
                &mb_service,
                &cover_service,
                &ar_service,
            ));

            if let Some(candidates) = mf.musicbrainz_release_choices.as_ref() {
                let discid_str = mf
                    .discid
                    .as_ref()
                    .map(|d| d.musicbrainz_discid.as_str())
                    .unwrap_or("");
                return Err(RunWorkflowError::Runtime(
                    format_musicbrainz_multiple_releases_message(discid_str, candidates),
                ));
            }

            for warning in &mf.warnings {
                log::warn!("{warning}");
            }

            if let Some(release) = mf.musicbrainz.as_ref()
                && let Some(album_artist) = release.album_artist.as_deref()
            {
                println!(
                    "Found MusicBrainz release: {} - {}",
                    release.album, album_artist
                );
            }

            println!(
                "Preparing rip metadata and file naming for {} selected track(s)...",
                read_plans.len()
            );

            for b in &read_plans {
                let idx = b.track_number.saturating_sub(1) as usize;
                if let Some(release) = mf.musicbrainz.as_ref()
                    && let Some(tmeta) = release.tracks.get(idx)
                {
                    let ent = track_meta_map.entry(b.track_number).or_default();
                    ent.entry("title".to_string())
                        .or_insert_with(|| tmeta.title.clone());
                    if let Some(artist) = tmeta.artist.as_deref() {
                        ent.entry("artist".to_string())
                            .or_insert_with(|| artist.to_string());
                    }
                    ent.entry("track".to_string())
                        .or_insert_with(|| format!("{:02}", b.track_number));
                    ent.entry("tracktotal".to_string())
                        .or_insert_with(|| release.tracks.len().to_string());
                }
            }

            metadata_flow = Some(mf);
        }
    }

    let output_root = configured_output_root(settings);

    let mut album_meta: HashMap<String, String> = parse_album_metadata_map(settings.album_metadata.as_deref())
        .into_iter()
        .collect();

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    if let Some(mf) = metadata_flow.as_ref() {
        if let Some(d) = mf.discid.as_ref() {
            album_meta
                .entry("musicbrainz_discid".to_string())
                .or_insert_with(|| d.musicbrainz_discid.clone());
            album_meta
                .entry("cddb".to_string())
                .or_insert_with(|| d.cddb.clone());
        }
        if let Some(release) = mf.musicbrainz.as_ref() {
            album_meta
                .entry("album".to_string())
                .or_insert_with(|| release.album.clone());
            if let Some(album_artist) = release.album_artist.as_deref() {
                album_meta
                    .entry("album_artist".to_string())
                    .or_insert_with(|| album_artist.to_string());
            }
            album_meta
                .entry("musicbrainz_albumid".to_string())
                .or_insert_with(|| release.musicbrainz_albumid.clone());
            if let Some(date) = release.date.as_deref() {
                album_meta
                    .entry("date".to_string())
                    .or_insert_with(|| date.to_string());
            }
        }
    }

    album_meta
        .entry("album".to_string())
        .or_insert_with(|| "Runtime Album".to_string());
    album_meta
        .entry("album_artist".to_string())
        .or_insert_with(|| "Runtime Artist".to_string());
    album_meta
        .entry("media".to_string())
        .or_insert_with(|| default_media_value(settings).to_string());

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    let cover_arts_for_write = if let Some(mf) = metadata_flow.as_ref() {
        mf.cover_arts.clone()
    } else {
        cli_cover_arts.clone()
    };

    #[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
    let cover_arts_for_write = cli_cover_arts.clone();

    let track_plan: Vec<(u32, HashMap<String, String>)> = read_plans
        .iter()
        .map(|b| {
            (
                b.track_number,
                track_meta_for_number(b.track_number, &track_meta_map),
            )
        })
        .collect();
    let naming_track_count = track_plan.len();

    println!(
        "Checking output path collisions for {} track(s) across {} format(s)...",
        naming_track_count,
        settings.outputs.len()
    );

    warn_track_path_collisions_for_formats(settings, &album_meta, &track_plan, naming_track_count)
        .map_err(|e| RunWorkflowError::Runtime(format!("full-rip writer flow failed: {e}")))?;

    println!("Starting track extraction and encoding...");

    let mut written_files = Vec::new();
    let mut benchmarks = Vec::new();
    // Encoding/writing a track's PCM (CPU-bound) is dispatched to a background
    // thread per track so the next track's disc read can start immediately
    // instead of waiting for the previous track's encode to finish.
    let mut encode_handles: Vec<std::thread::JoinHandle<Result<TrackOutputFlowResult, String>>> =
        Vec::new();
    for boundary in &read_plans {
        let track_started = Instant::now();
        #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
        let mut track_attempt = 0u32;
        #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
        let max_track_attempts = settings.max_retries.max(1) as u32;
        let pcm = loop {
            #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
            {
                track_attempt = track_attempt.saturating_add(1);
                println!(
                    "Track {} read attempt {} of {}...",
                    boundary.track_number, track_attempt, max_track_attempts
                );
            }
            let acquired = match source {
                FullRipSource::Image => acquire_tracks_pcm_from_image_reader(
                    settings,
                    std::slice::from_ref(boundary),
                )?
                .into_iter()
                .next()
                .map(|(_, acquired)| acquired)
                .ok_or_else(|| {
                    RunWorkflowError::Runtime(format!(
                        "image track acquisition returned no PCM for track {}",
                        boundary.track_number
                    ))
                })?,
                FullRipSource::Physical => acquire_tracks_pcm_from_physical_reader(
                    settings,
                    std::slice::from_ref(boundary),
                    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
                    metadata_flow.as_ref(),
                    #[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
                    None,
                )?
                .into_iter()
                .next()
                .map(|(_, acquired)| acquired)
                .ok_or_else(|| {
                    RunWorkflowError::Runtime(format!(
                        "physical track acquisition returned no PCM for track {}",
                        boundary.track_number
                    ))
                })?,
            };

            #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
            {
                let conf = acquired
                    .accurip_confidence_from_paranoia_frames
                    .or_else(|| {
                        track_accurip_confidence_for_pcm(
                            boundary.track_number,
                            &acquired.pcm,
                            metadata_flow.as_ref(),
                        )
                    });
                match conf {
                    Some(v) if v > 0 => println!(
                        "AccurateRip verified for track {} on attempt {} with confidence {}.",
                        boundary.track_number, track_attempt, v
                    ),
                    Some(0) => println!(
                        "AccurateRip status found but confidence is 0 for track {} on attempt {}.",
                        boundary.track_number, track_attempt
                    ),
                    None => println!(
                        "AccurateRip verification unavailable for track {} on attempt {}.",
                        boundary.track_number, track_attempt
                    ),
                    _ => {}
                }
                if conf == Some(-1) {
                    if track_attempt < max_track_attempts {
                        println!(
                            "AccurateRip mismatch on track {} (attempt {} of {}), retrying track read...",
                            boundary.track_number,
                            track_attempt,
                            max_track_attempts
                        );
                        continue;
                    }
                    log::error!(
                        "AccurateRip mismatch persisted on track {} after {} attempt(s); failing exact-rip enforcement",
                        boundary.track_number,
                        track_attempt
                    );
                    return Err(RunWorkflowError::Runtime(format!(
                        "AccurateRip mismatch persisted on track {} after {} attempt(s)",
                        boundary.track_number, track_attempt
                    )));
                }
            }

            break acquired.pcm;
        };
        let pcm_bytes = pcm
            .interleaved_i16_samples
            .len()
            .saturating_mul(std::mem::size_of::<i16>());

        // Benchmark covers read + AccurateRip verification only; encode/write now
        // happens concurrently with subsequent tracks' reads (see encode_handles).
        benchmarks.push(TrackBenchmark {
            track_number: boundary.track_number,
            elapsed_ms: track_started.elapsed().as_millis(),
            pcm_bytes,
            rss_kib_after: current_rss_kib(),
        });

        let track_meta = track_meta_for_number(boundary.track_number, &track_meta_map);
        let encode_input = TrackOutputFlowInput {
            settings: settings.clone(),
            output_root: output_root.clone(),
            album_meta: album_meta.clone(),
            cover_arts: cover_arts_for_write.clone(),
            tracks: vec![TrackOutputInput {
                track_number: boundary.track_number,
                track_meta,
                pcm,
            }],
        };
        let track_number = boundary.track_number;
        encode_handles.push(std::thread::spawn(move || {
            write_track_outputs_with_naming_tracks(
                encode_input,
                naming_track_count,
                false,
                Some(track_number),
            )
            .map_err(|e| format!("full-rip writer flow failed: {e}"))
        }));
    }

    for handle in encode_handles {
        let result = handle
            .join()
            .map_err(|_| RunWorkflowError::Runtime("track encode thread panicked".to_string()))?
            .map_err(RunWorkflowError::Runtime)?;
        written_files.extend(result.written_files);
    }

    let mut out = String::new();
    out.push_str("cyanrip-rs full-rip bridge mode\n");
    out.push_str(&format!(
        "Source: {}\n",
        match source {
            FullRipSource::Image => "image",
            FullRipSource::Physical => "physical",
        }
    ));
    out.push_str(&format!("Output root: {}\n", output_root.display()));
    out.push_str(&format!("Written files: {}\n", written_files.len()));
    if let Some(peak_kib) = peak_rss_kib() {
        out.push_str(&format!("Peak RSS:      {}\n", format_kib_as_mib(peak_kib)));
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    if let Some(mf) = metadata_flow.as_ref() {
        let ar_line = match mf.accurip_status {
            AccuDbStatus::Found => "found",
            AccuDbStatus::NotFound => "not found",
            AccuDbStatus::Mismatch => "mismatch",
            AccuDbStatus::Disabled => "disabled",
            AccuDbStatus::Error => "error",
        };
        out.push_str(&format!("AccurateRip:    {}\n", ar_line));
        if let Some(ar) = mf.accurip.as_ref() {
            for b in &read_plans {
                let idx = b.track_number.saturating_sub(1) as usize;
                if let Some(tm) = ar.track_matches.get(idx) {
                    out.push_str(&format!(
                        "Track {} AccuRip max confidence: {}\n",
                        b.track_number, tm.max_confidence
                    ));
                }
            }
        }
    }

    for boundary in &read_plans {
        let benchmark = benchmarks
            .iter()
            .find(|b| b.track_number == boundary.track_number)
            .copied();
        out.push_str(&format!(
            "TRACK {} START_LSN {} FRAMES {}\n",
            boundary.track_number, boundary.start_lsn, boundary.frame_count
        ));
        out.push_str(&format!(
            "Track {} ripped and encoded successfully!\n",
            boundary.track_number
        ));

        out.push_str(&format!("Track {} summary:\n", boundary.track_number));
        out.push_str("  Properties:\n");
        out.push_str(&format!(
            "    Duration:    {}\n",
            format_duration_from_frames(boundary.frame_count)
        ));
        out.push_str(&format!(
            "    Samples:     {}\n",
            samples_from_frames(boundary.frame_count)
        ));
        out.push_str(&format!("    Frames:      {}\n", boundary.frame_count));
        out.push_str(&format!("    Start LSN:   {}\n", boundary.start_lsn));
        out.push_str(&format!(
            "    End LSN:     {}\n",
            boundary
                .start_lsn
                .saturating_add(boundary.frame_count.saturating_sub(1) as i32)
        ));
        if let Some(b) = benchmark {
            out.push_str(&format!(
                "    Benchmark:   {} ms, PCM {}, RSS {}\n",
                b.elapsed_ms,
                format_bytes_as_mib(b.pcm_bytes),
                b.rss_kib_after
                    .map(format_kib_as_mib)
                    .unwrap_or_else(|| "n/a".to_string())
            ));
        }

        out.push_str("\n  Metadata:\n");
        if let Some(meta) = track_meta_map.get(&boundary.track_number) {
            let mut keys: Vec<&String> = meta.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(value) = meta.get(key)
                    && !value.trim().is_empty()
                {
                    out.push_str(&format!("    {:<30} {}\n", key, value));
                }
            }
        } else {
            out.push_str("    none\n");
        }

        out.push_str("\n  File(s):\n");
        let mut files_for_track = written_files
            .iter()
            .filter(|f| f.track_number == boundary.track_number)
            .collect::<Vec<_>>();
        files_for_track.sort_by(|a, b| a.absolute_path.cmp(&b.absolute_path));
        for file in files_for_track {
            out.push_str(&format!("    {}\n", file.relative_path.display()));
        }
        out.push('\n');
    }
    for file in &written_files {
        out.push_str(&format!("FILE {}\n", file.absolute_path.display()));
    }

    write_runtime_log_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &out,
    )?;

    write_runtime_cue_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &written_files,
        &track_meta_map,
    )?;

    write_runtime_cover_files(
        settings,
        &output_root,
        &album_meta,
        naming_track_count,
        &cover_arts_for_write,
    )?;

    maybe_eject_after_success(settings, source);

    Ok(out)
}

pub fn run_workflow(settings: &Settings) -> Result<Option<String>, RunWorkflowError> {
    for fmt_kind in &settings.outputs {
        match fmt_kind {
            OutputFormat::Wav | OutputFormat::Flac => {}
            _ => return Err(RunWorkflowError::UnsupportedOutputFormat(*fmt_kind)),
        }
    }

    if settings.find_drive_offset {
        return run_find_offset_mode(settings).map(Some);
    }

    if settings.print_info_only {
        return run_info_only_mode(settings).map(Some);
    }

    if settings.generate_cue_only {
        if !settings.offset_is_set {
            return Ok(Some(
                "Offset is unset! To continue with an offset of 0, run with -s 0!".to_string(),
            ));
        }
        let out = run_cue_only_mode(settings)?;
        maybe_eject_after_success(settings, full_rip_source_from_settings(settings));
        return Ok(Some(out));
    }

    if env_var_truthy("CYANRIP_RS_ENABLE_SYNTHETIC_RIP") {
        return render_synthetic_full_rip(settings).map(Some);
    }

    run_full_rip_from_selected_source(settings).map(Some)
}

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
    /// Set when MusicBrainz found multiple releases and no -R selector was given.
    pub musicbrainz_release_choices: Option<Vec<ReleaseSummary>>,
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
    let mut musicbrainz_release_choices = None;
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
                Err(MusicBrainzError::MultipleReleases(candidates)) => {
                    warnings.push(
                        "musicbrainz lookup failed: multiple releases found, use -R to select one"
                            .to_string(),
                    );
                    musicbrainz_release_choices = Some(candidates);
                }
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
                        musicbrainz_release_choices,
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
                        musicbrainz_release_choices,
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
        musicbrainz_release_choices,
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
    pub cover_arts: Vec<CoverArtImage>,
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
    Processing {
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
            Self::Processing {
                output_format,
                path,
                message,
            } => write!(
                f,
                "processing error for {output_format:?} at {}: {message}",
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct ReplayGainStats {
    peak: f64,
    mean_square: f64,
    sample_count: u64,
}

impl ReplayGainStats {
    fn rms(self) -> f64 {
        self.mean_square.sqrt()
    }
}

fn replaygain_stats_from_processed_pcm(input: &ProcessedPcmTrackData) -> Option<ReplayGainStats> {
    if input.interleaved_i32_samples.is_empty() {
        return None;
    }

    let full_scale = if input.spec.bits_per_sample == 24 {
        8_388_608.0
    } else {
        32_768.0
    };

    let mut peak = 0.0f64;
    let mut sum_sq = 0.0f64;
    for &sample in &input.interleaved_i32_samples {
        let normalized = (sample as f64) / full_scale;
        let abs = normalized.abs();
        if abs > peak {
            peak = abs;
        }
        sum_sq += normalized * normalized;
    }

    let sample_count = input.interleaved_i32_samples.len() as u64;
    if sample_count == 0 {
        return None;
    }

    Some(ReplayGainStats {
        peak,
        mean_square: sum_sq / sample_count as f64,
        sample_count,
    })
}

fn replaygain_gain_db_from_rms(rms: f64) -> f64 {
    if rms <= 1e-12 {
        0.0
    } else {
        -18.0 - 20.0 * rms.log10()
    }
}

fn build_replaygain_track_comment_map(stats: ReplayGainStats) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let gain_db = replaygain_gain_db_from_rms(stats.rms());
    out.insert("REPLAYGAIN_REFERENCE_LOUDNESS".to_string(), "89.0 dB".to_string());
    out.insert("REPLAYGAIN_TRACK_GAIN".to_string(), format!("{gain_db:+.2} dB"));
    out.insert("REPLAYGAIN_TRACK_PEAK".to_string(), format!("{:.8}", stats.peak));
    out
}

fn aggregate_album_replaygain_stats(stats: &[ReplayGainStats]) -> Option<ReplayGainStats> {
    if stats.is_empty() {
        return None;
    }

    let mut peak = 0.0f64;
    let mut weighted_mean_square = 0.0f64;
    let mut sample_count: u64 = 0;

    for s in stats {
        if s.peak > peak {
            peak = s.peak;
        }
        weighted_mean_square += s.mean_square * s.sample_count as f64;
        sample_count = sample_count.saturating_add(s.sample_count);
    }

    if sample_count == 0 {
        return None;
    }

    Some(ReplayGainStats {
        peak,
        mean_square: weighted_mean_square / sample_count as f64,
        sample_count,
    })
}

fn build_replaygain_album_comment_map(album: ReplayGainStats) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let gain_db = replaygain_gain_db_from_rms(album.rms());
    out.insert("REPLAYGAIN_ALBUM_GAIN".to_string(), format!("{gain_db:+.2} dB"));
    out.insert("REPLAYGAIN_ALBUM_PEAK".to_string(), format!("{:.8}", album.peak));
    out
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
    embedded_picture: Option<&FlacEmbeddedPicture>,
) -> Result<(), TrackOutputFlowError> {
    let mut tag = metaflac::Tag::read_from_path(path).map_err(|e| TrackOutputFlowError::Tagging {
        output_format: OutputFormat::Flac,
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    for (key, value) in comments {
        tag.set_vorbis(key, vec![value]);
    }

    if let Some(picture) = embedded_picture {
        tag.add_picture(
            picture.mime_type.clone(),
            picture.picture_type,
            picture.data.clone(),
        );
    }

    tag.save().map_err(|e| TrackOutputFlowError::Tagging {
        output_format: OutputFormat::Flac,
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

#[derive(Debug, Clone)]
struct FlacEmbeddedPicture {
    mime_type: String,
    picture_type: metaflac::block::PictureType,
    data: Vec<u8>,
}

fn infer_cover_mime_type(art: &CoverArtImage) -> String {
    if let Some(content_type) = art.content_type.as_deref() {
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !mime.is_empty() {
            return mime;
        }
    }

    let extension = art
        .extension
        .clone()
        .or_else(|| infer_cover_extension_from_source(&art.source_url));

    match extension.as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("png") => "image/png".to_string(),
        Some("gif") => "image/gif".to_string(),
        Some("webp") => "image/webp".to_string(),
        Some("bmp") => "image/bmp".to_string(),
        Some("tif") | Some("tiff") => "image/tiff".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

fn pick_cover_art_for_embedding(cover_arts: &[CoverArtImage]) -> Option<&CoverArtImage> {
    cover_arts
        .iter()
        .find(|art| {
            art.title.eq_ignore_ascii_case("front")
                && art.data.as_ref().is_some_and(|bytes| !bytes.is_empty())
        })
        .or_else(|| {
            cover_arts
                .iter()
                .find(|art| art.data.as_ref().is_some_and(|bytes| !bytes.is_empty()))
        })
}

fn flac_embedded_picture_from_cover_arts(cover_arts: &[CoverArtImage]) -> Option<FlacEmbeddedPicture> {
    let art = pick_cover_art_for_embedding(cover_arts)?;
    let data = art.data.clone()?;

    let picture_type = if art.title.eq_ignore_ascii_case("back") {
        metaflac::block::PictureType::CoverBack
    } else {
        metaflac::block::PictureType::CoverFront
    };

    Some(FlacEmbeddedPicture {
        mime_type: infer_cover_mime_type(art),
        picture_type,
        data,
    })
}

fn warn_track_path_collisions_for_formats(
    settings: &Settings,
    album_meta: &HashMap<String, String>,
    tracks: &[(u32, HashMap<String, String>)],
    naming_track_count: usize,
) -> Result<(), TrackOutputFlowError> {
    let naming_ctx = NamingContext {
        sanitize_method: settings.sanitize_method,
        nb_tracks: naming_track_count,
    };

    for fmt_kind in &settings.outputs {
        let (format_suffix, extension) = output_format_descriptor(*fmt_kind)
            .ok_or(TrackOutputFlowError::UnsupportedOutputFormat(*fmt_kind))?;

        let mut collision_input = Vec::with_capacity(tracks.len());
        for (track_number, track_meta) in tracks {
            let relative_path_str = build_track_relative_path(
                &naming_ctx,
                album_meta,
                track_meta,
                &settings.folder_name_scheme,
                &settings.track_name_scheme,
                format_suffix,
                extension,
            )
            .map_err(TrackOutputFlowError::Naming)?;
            collision_input.push((*track_number, relative_path_str));
        }

        for (a, b, path) in detect_track_path_collisions(&collision_input) {
            log::warn!(
                "tracks {a} and {b} resolve to the same file \"{path}\", one will overwrite the other!"
            );
        }
    }

    Ok(())
}

fn write_track_outputs_with_naming_tracks(
    input: TrackOutputFlowInput,
    naming_track_count: usize,
    emit_collision_warnings: bool,
    progress_track_number: Option<u32>,
) -> Result<TrackOutputFlowResult, TrackOutputFlowError> {
    let naming_ctx = NamingContext {
        sanitize_method: input.settings.sanitize_method,
        nb_tracks: naming_track_count,
    };

    let per_output_units = |fmt_kind: OutputFormat| -> usize {
        match fmt_kind {
            OutputFormat::Wav => 2,
            OutputFormat::Flac => 3,
            _ => 1,
        }
    };
    let total_jobs = input
        .settings
        .outputs
        .iter()
        .map(|fmt_kind| per_output_units(*fmt_kind).saturating_mul(input.tracks.len()))
        .sum::<usize>()
        .max(1);
    let progress_started = Instant::now();
    let mut completed_jobs = 0usize;

    let emit_encoding_progress = |track_number: u32, completed: usize| {
        let progress = (completed as f64 / total_jobs as f64) * 100.0;
        let elapsed = progress_started.elapsed().as_secs_f64().max(0.001);
        let eta_label = if completed == 0 {
            "--:--".to_string()
        } else {
            let eta_min = if completed >= total_jobs {
                0.0
            } else {
                let rate = completed as f64 / elapsed;
                let remaining = total_jobs.saturating_sub(completed) as f64;
                (remaining / rate) / 60.0
            };
            format_eta_min_sec(eta_min)
        };

        print!(
            "\rEncoding         : track {}, progress - {:.2}%, ETA - {}   ", track_number, progress, eta_label
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());
        if completed >= total_jobs {
            println!();
        }
    };

    let mut written_files = Vec::new();
    let flac_embedded_picture = if input.settings.disable_coverart_embedding {
        None
    } else {
        flac_embedded_picture_from_cover_arts(&input.cover_arts)
    };

    for fmt_kind in &input.settings.outputs {
        let (format_suffix, extension) = output_format_descriptor(*fmt_kind)
            .ok_or(TrackOutputFlowError::UnsupportedOutputFormat(*fmt_kind))?;

        let mut flac_replaygain_stats: Vec<(PathBuf, ReplayGainStats)> = Vec::new();

        let mut planned_paths: Vec<(usize, u32, PathBuf, PathBuf)> = Vec::new();
        for (idx, track) in input.tracks.iter().enumerate() {
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

            let absolute_path = resolve_output_path(&input.output_root, &relative_path_str, true)?;
            planned_paths.push((
                idx,
                track.track_number,
                PathBuf::from(relative_path_str),
                absolute_path,
            ));
        }

        if emit_collision_warnings {
            let collision_input: Vec<(u32, String)> = planned_paths
                .iter()
                .map(|(_, track_number, rel, _)| (*track_number, rel.to_string_lossy().to_string()))
                .collect();
            for (a, b, path) in detect_track_path_collisions(&collision_input) {
                log::warn!(
                    "tracks {a} and {b} resolve to the same file \"{path}\", one will overwrite the other!"
                );
            }
        }

        for (idx, _track_number, relative_path, absolute_path) in planned_paths {
            let track = &input.tracks[idx];

            if let Some(track_number) = progress_track_number {
                if completed_jobs == 0 {
                    emit_encoding_progress(track_number, completed_jobs);
                }
            }

            let processed_pcm = process_track_pcm(
                &track.pcm,
                TrackProcessingOptions {
                    decode_hdcd: input.settings.decode_hdcd,
                    deemphasis: input.settings.deemphasis,
                    force_deemphasis: input.settings.force_deemphasis,
                    track_has_preemphasis: track_has_preemphasis(&track.track_meta),
                },
            )
            .map_err(|e| TrackOutputFlowError::Processing {
                output_format: *fmt_kind,
                path: absolute_path.clone(),
                message: e.to_string(),
            })?;

            let replaygain_stats = if input.settings.enable_replaygain {
                replaygain_stats_from_processed_pcm(&processed_pcm)
            } else {
                None
            };

            completed_jobs = completed_jobs.saturating_add(1);
            if let Some(track_number) = progress_track_number {
                emit_encoding_progress(track_number, completed_jobs);
            }

            match fmt_kind {
                OutputFormat::Wav => {
                    write_wav_file(&absolute_path, &processed_pcm).map_err(|e| {
                        TrackOutputFlowError::Encode {
                            output_format: *fmt_kind,
                            path: absolute_path.clone(),
                            message: e.to_string(),
                        }
                    })?;
                    completed_jobs = completed_jobs.saturating_add(1);
                    if let Some(track_number) = progress_track_number {
                        emit_encoding_progress(track_number, completed_jobs);
                    }
                }
                OutputFormat::Flac => {
                    write_flac_file(&absolute_path, &processed_pcm).map_err(|e| {
                        TrackOutputFlowError::Encode {
                            output_format: *fmt_kind,
                            path: absolute_path.clone(),
                            message: e.to_string(),
                        }
                    })?;
                    completed_jobs = completed_jobs.saturating_add(1);
                    if let Some(track_number) = progress_track_number {
                        emit_encoding_progress(track_number, completed_jobs);
                    }

                    let comments = build_flac_comment_map(&input.settings, &input.album_meta, track);
                    let mut comments = comments;
                    if let Some(stats) = replaygain_stats {
                        for (k, v) in build_replaygain_track_comment_map(stats) {
                            comments.insert(k, v);
                        }
                        flac_replaygain_stats.push((absolute_path.clone(), stats));
                    }

                    embed_flac_vorbis_comments(&absolute_path, &comments, flac_embedded_picture.as_ref())?;
                    completed_jobs = completed_jobs.saturating_add(1);
                    if let Some(track_number) = progress_track_number {
                        emit_encoding_progress(track_number, completed_jobs);
                    }
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

        if *fmt_kind == OutputFormat::Flac
            && input.settings.enable_replaygain
            && !flac_replaygain_stats.is_empty()
        {
            let stats_only: Vec<ReplayGainStats> = flac_replaygain_stats
                .iter()
                .map(|(_, stats)| *stats)
                .collect();
            if let Some(album_stats) = aggregate_album_replaygain_stats(&stats_only) {
                let album_comments = build_replaygain_album_comment_map(album_stats);
                for (path, _) in &flac_replaygain_stats {
                    embed_flac_vorbis_comments(path, &album_comments, None)?;
                }
            }
        }
    }

    Ok(TrackOutputFlowResult { written_files })
}

pub fn write_track_outputs(input: TrackOutputFlowInput) -> Result<TrackOutputFlowResult, TrackOutputFlowError> {
    let naming_track_count = input.tracks.len();
    write_track_outputs_with_naming_tracks(input, naming_track_count, true, None)
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
    use crate::{OutputFormat, Settings};

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

    #[test]
    fn run_workflow_rejects_unimplemented_output_codec() {
        let settings = Settings {
            outputs: vec![OutputFormat::Mp3],
            ..Settings::default()
        };

        assert_eq!(
            run_workflow(&settings),
            Err(RunWorkflowError::UnsupportedOutputFormat(OutputFormat::Mp3))
        );
    }

    #[test]
    fn run_workflow_find_offset_mode_returns_report() {
        let settings = Settings {
            find_drive_offset: true,
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        match run_workflow(&settings) {
            Ok(Some(report)) => {
                assert!(
                    report.contains("Searching for drive offset")
                        || report.contains("cyanrip-rs find-offset mode"),
                    "unexpected find-offset header: {report}"
                );
                assert!(
                    report.contains("Drive offset of ")
                        || report.contains("No track had AccuRip entry")
                        || report.contains("No track was long enough")
                        || report.contains("Was not able to find drive offset")
                        || report.contains("Status: unavailable in this build"),
                    "unexpected find-offset report: {report}"
                );
            }
            Err(RunWorkflowError::Runtime(msg)) => {
                assert!(
                    msg.contains("TOC read failed") || msg.contains("physical read failed"),
                    "unexpected runtime error: {msg}"
                );
            }
            other => panic!("unexpected find-offset outcome: {other:?}"),
        }
    }

    #[test]
    fn run_workflow_info_mode_returns_report() {
        let settings = Settings {
            print_info_only: true,
            disable_mb: true,
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        match run_workflow(&settings) {
            Ok(Some(report)) => {
                assert!(report.contains("cyanrip-rs "));
                assert!(report.contains("Paranoia level: "));
                assert!(report.contains("Outputs:        "));
                assert!(report.contains("AccurateRip:    "));
            }
            Err(RunWorkflowError::Runtime(msg)) => {
                assert!(
                    msg.contains("TOC read failed"),
                    "unexpected info-only runtime error: {msg}"
                );
            }
            other => panic!("unexpected info-only outcome: {other:?}"),
        }
    }

    #[test]
    fn eject_gate_requires_flag_and_physical_source() {
        let mut settings = Settings::default();
        assert!(!should_attempt_eject_on_success(&settings, FullRipSource::Physical));
        assert!(!should_attempt_eject_on_success(&settings, FullRipSource::Image));

        settings.eject_on_success_rip = true;
        assert!(should_attempt_eject_on_success(&settings, FullRipSource::Physical));
        assert!(!should_attempt_eject_on_success(&settings, FullRipSource::Image));
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    #[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
    fn should_attempt_eject_on_success_with_audio_cd_inserted() {
        use crate::cdda::linux_drive::read_drive_toc_tracks;

        let device = std::env::var("CYANRIP_CDROM_DEVICE")
            .unwrap_or_else(|_| "/dev/cdrom".to_string());

        let toc = read_drive_toc_tracks(Some(&device)).unwrap_or_else(|err| {
            panic!("failed to read TOC from {device}: {err:?}");
        });
        assert!(
            !toc.is_empty(),
            "no TOC tracks found on {device}; ensure an audio CD is inserted beforehand"
        );

        let settings = Settings {
            dev_path: Some(device),
            eject_on_success_rip: true,
            ..Settings::default()
        };

        assert!(should_attempt_eject_on_success(&settings, FullRipSource::Physical));
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn toc_preflight_mapper_returns_clear_no_medium_message() {
        let err = crate::cdda::reader::CddaReadError::ReadFailed(
            "invalid TOC values returned by drive".to_string(),
        );
        assert_eq!(
            map_toc_preflight_error(&err),
            RunWorkflowError::Runtime("Drive has no readable audio medium inserted".to_string())
        );

        let err_no_medium = crate::cdda::reader::CddaReadError::ReadFailed(
            "error in ioctl CDROMREADTOCHDR: No medium found".to_string(),
        );
        assert_eq!(
            map_toc_preflight_error(&err_no_medium),
            RunWorkflowError::Runtime("Drive has no readable audio medium inserted".to_string())
        );
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn toc_preflight_mapper_keeps_generic_toc_message_for_other_errors() {
        let err = crate::cdda::reader::CddaReadError::ReadFailed("permission denied".to_string());
        assert_eq!(
            map_toc_preflight_error(&err),
            RunWorkflowError::Runtime("TOC read failed: ReadFailed(\"permission denied\")".to_string())
        );
    }

    #[test]
    fn format_msf_from_frames_uses_mm_ss_ff() {
        assert_eq!(format_msf_from_frames(0), "00:00.00");
        assert_eq!(format_msf_from_frames(75), "00:01.00");
        assert_eq!(format_msf_from_frames(150), "00:02.00");
        assert_eq!(format_msf_from_frames(4510), "01:00.10");
    }

    #[test]
    fn info_only_report_with_toc_lists_tracks() {
        let settings = Settings {
            print_info_only: true,
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        let toc = vec![
            InfoTocEntry {
                number: 1,
                start_lsn: 0,
                end_lsn: 149,
                track_is_data: false,
                pregap_lsn: None,
            },
            InfoTocEntry {
                number: 2,
                start_lsn: 200,
                end_lsn: 349,
                track_is_data: true,
                pregap_lsn: Some(192),
            },
        ];

        let report = render_info_only_report_with_toc(&settings, None, &toc, None, None);
        assert!(report.contains("Disc tracks:    2"));
        assert!(report.contains("Track 1 info:"));
        assert!(report.contains("    Duration:    00:02.00"));
        assert!(report.contains("    Samples:     88200"));
        assert!(report.contains("    Pregap LSN:  none"));
        assert!(report.contains("    Start LSN:   0"));
        assert!(report.contains("    End LSN:     149"));
        assert!(report.contains("Track 2 info:"));
        assert!(report.contains("    Data bytes:  "));
        assert!(report.contains("    Pregap LSN:  192 (duration: 00:00.08)"));
    }

    #[test]
    fn info_only_report_with_toc_filters_to_selected_tracks() {
        let settings = Settings {
            print_info_only: true,
            outputs: vec![OutputFormat::Flac],
            rip_indices: vec![2],
            rip_indices_count: 1,
            ..Settings::default()
        };
        let toc = vec![
            InfoTocEntry {
                number: 1,
                start_lsn: 0,
                end_lsn: 149,
                track_is_data: false,
                pregap_lsn: None,
            },
            InfoTocEntry {
                number: 2,
                start_lsn: 200,
                end_lsn: 349,
                track_is_data: true,
                pregap_lsn: Some(192),
            },
        ];

        let report = render_info_only_report_with_toc(&settings, None, &toc, None, None);
        assert!(report.contains("Tracks to rip:  2"));
        assert!(!report.contains("Track 1 info:"));
        assert!(report.contains("Track 2 info:"));
    }

    #[test]
    fn validate_requested_track_indices_against_toc_rejects_out_of_range_and_zero() {
        let toc = vec![
            InfoTocEntry {
                number: 1,
                start_lsn: 0,
                end_lsn: 149,
                track_is_data: false,
                pregap_lsn: None,
            },
            InfoTocEntry {
                number: 2,
                start_lsn: 150,
                end_lsn: 299,
                track_is_data: false,
                pregap_lsn: None,
            },
        ];

        let err = validate_requested_track_indices_against_toc(&toc, &[0, 3])
            .expect_err("invalid selection should fail");
        match err {
            RunWorkflowError::Runtime(msg) => {
                assert_eq!(msg, "Invalid rip index 0, list has 2 tracks!");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn resolve_requested_audio_tracks_rejects_invalid_selection() {
        let err = resolve_requested_audio_tracks(&[1, 99], &[1, 2, 3], 10)
            .expect_err("invalid selected track should fail");

        match err {
            RunWorkflowError::Runtime(msg) => {
                assert_eq!(msg, "Invalid rip index 99, list has 10 tracks!");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn info_only_report_with_release_includes_metadata_block() {
        let settings = Settings {
            print_info_only: true,
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        let toc = vec![InfoTocEntry {
            number: 1,
            start_lsn: 0,
            end_lsn: 149,
            track_is_data: false,
            pregap_lsn: None,
        }];
        let release = MusicBrainzReleaseMeta {
            musicbrainz_albumid: "rel-1".to_string(),
            releasecomment: None,
            date: Some("1994".to_string()),
            album: "Album One".to_string(),
            barcode: Some("1234567890123".to_string()),
            packaging: Some("Other".to_string()),
            country: Some("US".to_string()),
            releasestatus: Some("Official".to_string()),
            catalognumber: Some("15 848".to_string()),
            label: Some("Label X".to_string()),
            album_artist: Some("Various Artists".to_string()),
            discname: None,
            format: Some("CD".to_string()),
            discnumber: Some(3),
            totaldiscs: 10,
            tracks: vec![crate::metadata::musicbrainz::MusicBrainzTrackMeta {
                mbid: Some("track-1-mbid".to_string()),
                title: "Ride of the Valkyries".to_string(),
                artist: Some("Richard Wagner".to_string()),
            }],
        };

        let report = render_info_only_report_with_toc(
            &settings,
            None,
            &toc,
            Some((
                "BKkzOxbdODYWFIOEEZ3b.b_nm64-",
                "8E0F310A",
                "https://musicbrainz.org/cdtoc/attach?toc=...",
            )),
            Some(&release),
        );

        assert!(report.contains("Release ID:     rel-1"));
        assert!(report.contains("Album:          Album One"));
        assert!(report.contains("Disc number:    3"));
        assert!(report.contains("Total discs:    10"));
        assert!(report.contains("  Metadata:"));
        assert!(report.contains("    mbid:                track-1-mbid"));
        assert!(report.contains("    disc_mcn:            0000000000000"));
        assert!(report.contains("    comment:             cyanrip 0.9.4-rc2"));
        assert!(report.contains("    date:                1994"));
        assert!(report.contains("    musicbrainz_albumid: rel-1"));
        assert!(report.contains("    packaging:           Other"));
        assert!(report.contains("    totaldiscs:          10"));
        assert!(report.contains("    disc:                3"));

        let comment_pos = report
            .find("    comment:             cyanrip 0.9.4-rc2")
            .expect("comment line should exist");
        let albumid_pos = report
            .find("    musicbrainz_albumid: rel-1")
            .expect("musicbrainz_albumid line should exist");
        assert!(comment_pos < albumid_pos, "comment should be before musicbrainz_albumid");
    }

    #[test]
    fn formats_musicbrainz_multiple_release_prompt_for_info_mode() {
        let releases = vec![
            ReleaseSummary {
                id: "id-1".to_string(),
                album: "Album One".to_string(),
                disambiguation: None,
                country: Some("US".to_string()),
                date: Some("1994".to_string()),
                num_cds: 1,
            },
            ReleaseSummary {
                id: "id-2".to_string(),
                album: "Album Two".to_string(),
                disambiguation: None,
                country: Some("US".to_string()),
                date: None,
                num_cds: 10,
            },
        ];

        let msg = format_musicbrainz_multiple_releases_message("TESTDISC", &releases);
        assert!(msg.contains("Multiple releases found in database for DiscID TESTDISC:"));
        assert!(msg.contains("1 (ID: id-1): Album One (US) (1994)"));
        assert!(msg.contains("2 (ID: id-2): Album Two (US) (10 CDs)"));
        assert!(msg.contains("Please specify which release to use by adding the -R argument"));
    }

    #[test]
    fn run_workflow_cue_only_mode_returns_preview() {
        // disable_mb keeps this deterministic; it doesn't test MusicBrainz behavior
        // and was previously flaky against the real MusicBrainz service (see repo memory).
        let settings = Settings {
            generate_cue_only: true,
            offset_is_set: true,
            disable_mb: true,
            album_metadata: Some("album=Example Album:album_artist=Example Artist".to_string()),
            track_metadata: vec![
                "1=title=Intro:artist=Example Artist".to_string(),
                "2=title=Outro:artist=Example Artist".to_string(),
            ],
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        match run_workflow(&settings) {
            Ok(Some(cue)) => {
                assert!(
                    cue.contains("cyanrip-rs cue-only preview")
                        || cue.contains("cyanrip-rs "),
                    "unexpected cue-only output: {cue}"
                );
                assert!(cue.contains("TITLE \"Example Album\"") || cue.contains("REM DISCID"));
                assert!(cue.contains("TRACK 01 AUDIO"));
            }
            Err(RunWorkflowError::Runtime(msg)) => {
                assert!(
                    msg.contains("TOC read failed"),
                    "unexpected cue-only runtime error: {msg}"
                );
            }
            other => panic!("unexpected cue-only outcome: {other:?}"),
        }
    }

    #[test]
    fn cue_only_preview_ingests_extended_track_fields() {
        let settings = Settings {
            generate_cue_only: true,
            deemphasis: false,
            album_metadata: Some("album=Extended Album:album_artist=Extended Artist".to_string()),
            track_metadata: vec![
                "1=title=Lead:artist=Singer:isrc=USAAA9912345:preemphasis=1:start_lsn=200:dropped_pregap_start=50:songwriter=Writer:composer=Composer:arranger=Arranger:flag_dcp=1:flag_4ch=1:flag_scms=1:postgap_frames=150".to_string(),
                "2=title=Data Cut:data=1:cue_file_type=binary".to_string(),
            ],
            ..Settings::default()
        };

        let cue = render_cue_only_preview(&settings);
        assert!(cue.contains("ISRC USAAA9912345"));
        assert!(cue.contains("FLAGS PRE"));
        assert!(cue.contains("FLAGS PRE DCP 4CH SCMS"));
        assert!(cue.contains("SONGWRITER \"Writer\""));
        assert!(cue.contains("COMPOSER \"Composer\""));
        assert!(cue.contains("ARRANGER \"Arranger\""));
        assert!(cue.contains("PREGAP 00:02:00"));
        assert!(cue.contains("POSTGAP 00:02:00"));
        assert!(cue.contains("FILE \"02 - Data Cut.flac\" BINARY"));
        assert!(cue.contains("TRACK 02 MODE1/2352"));
    }

    #[test]
    fn run_workflow_selects_image_source_for_default_and_cue_paths() {
        let settings_default = Settings {
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
        assert_eq!(
            full_rip_source_from_settings(&settings_default),
            FullRipSource::Physical
        );

        #[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
        assert_eq!(full_rip_source_from_settings(&settings_default), FullRipSource::Image);

        let settings_cue = Settings {
            dev_path: Some("disc.cue".to_string()),
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        assert_eq!(full_rip_source_from_settings(&settings_cue), FullRipSource::Image);
    }

    #[test]
    fn selected_track_numbers_default_to_empty_selection() {
        let settings = Settings::default();
        assert!(selected_track_numbers(&settings).is_empty());
    }

    #[test]
    fn apply_offset_frame_adjustment_matches_upstream_coarse_shift() {
        let boundary = TrackBoundary {
            track_number: 1,
            start_lsn: 0,
            frame_count: 100,
        };

        let settings_pos = Settings {
            over_under_read_frames: 1,
            ..Settings::default()
        };
        let got_pos = apply_offset_frame_adjustment(boundary, &settings_pos);
        assert_eq!(got_pos.start_lsn, 0);
        assert_eq!(got_pos.frame_count, 101);

        let settings_neg = Settings {
            over_under_read_frames: -2,
            ..Settings::default()
        };
        let got_neg = apply_offset_frame_adjustment(boundary, &settings_neg);
        assert_eq!(got_neg.start_lsn, -2);
        assert_eq!(got_neg.frame_count, 101);
    }

    #[test]
    fn apply_drive_offset_crop_realigns_samples_by_exact_sample_offset() {
        // Regression test for a real bug: apply_offset_frame_adjustment only
        // grows the read window by whole CD frames, so the sub-frame sample
        // shift must be cropped out afterward or ripped audio is a whole
        // frame too long/misaligned and never matches AccurateRip.
        let boundary = TrackBoundary {
            track_number: 1,
            start_lsn: 100,
            frame_count: 10,
        };
        let offset = 103;
        let settings = Settings {
            offset,
            over_under_read_frames: crate::calc_over_under_read_frames(offset),
            overread_leadinout: true,
            ..Settings::default()
        };
        let plan = plan_track_read(boundary, &settings, 0, 100_000);
        assert_eq!(plan.frame_count, 11, "positive sub-frame offset should overread by one whole frame");
        assert_eq!(plan.original_frame_count, 10);

        let i16_per_cd_frame = CDDA_FRAME_BYTES / 2;
        let total_i16 = plan.frame_count * i16_per_cd_frame;
        let samples: Vec<i16> = (0..total_i16).map(|i| i as i16).collect();
        let pcm = PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: samples,
        };

        let cropped = apply_drive_offset_crop(pcm, &plan, offset);
        let expected_len = plan.original_frame_count * i16_per_cd_frame;
        assert_eq!(cropped.interleaved_i16_samples.len(), expected_len);
        // offset (103) samples * 2 i16 channels = 206 i16 elements skipped from the front.
        assert_eq!(cropped.interleaved_i16_samples[0], 206);
        assert_eq!(
            *cropped.interleaved_i16_samples.last().unwrap(),
            (206 + expected_len - 1) as i16
        );
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn app_tracks_from_image_boundaries_maps_lsn_and_marks_audio_only() {
        // Locks in MusicBrainz metadata parity for image-source full rips:
        // this feeds orchestrate_metadata_flow the same way physical-drive
        // TOC tracks do, so album/track metadata gets assigned for images too.
        let boundaries = vec![
            TrackBoundary {
                track_number: 1,
                start_lsn: 0,
                frame_count: 100,
            },
            TrackBoundary {
                track_number: 2,
                start_lsn: 100,
                frame_count: 50,
            },
        ];

        let tracks = app_tracks_from_image_boundaries(&boundaries);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].number, 1);
        assert_eq!(tracks[0].start_lsn, 0);
        assert_eq!(tracks[0].end_lsn, 99);
        assert!(!tracks[0].track_is_data);
        assert_eq!(tracks[1].number, 2);
        assert_eq!(tracks[1].start_lsn, 100);
        assert_eq!(tracks[1].end_lsn, 149);
        assert!(!tracks[1].track_is_data);
    }

    #[test]
    fn plan_track_read_clips_and_adds_silence_when_overread_disabled() {
        let boundary = TrackBoundary {
            track_number: 1,
            start_lsn: 0,
            frame_count: 100,
        };
        let settings = Settings {
            over_under_read_frames: -2,
            overread_leadinout: false,
            ..Settings::default()
        };

        let plan = plan_track_read(boundary, &settings, 0, 1000);
        assert_eq!(plan.read_start_lsn, 0);
        assert_eq!(plan.read_frame_count, 99);
        assert_eq!(plan.silence_before_frames, 2);
        assert_eq!(plan.silence_after_frames, 0);
        assert_eq!(plan.frame_count, 101);
    }

    #[test]
    fn plan_track_read_keeps_out_of_range_reads_when_overread_enabled() {
        let boundary = TrackBoundary {
            track_number: 1,
            start_lsn: 0,
            frame_count: 100,
        };
        let settings = Settings {
            over_under_read_frames: -2,
            overread_leadinout: true,
            ..Settings::default()
        };

        let plan = plan_track_read(boundary, &settings, 0, 1000);
        assert_eq!(plan.read_start_lsn, -2);
        assert_eq!(plan.read_frame_count, 101);
        assert_eq!(plan.silence_before_frames, 0);
        assert_eq!(plan.silence_after_frames, 0);
        assert_eq!(plan.frame_count, 101);
    }

    #[cfg(all(target_os = "linux", feature = "cdda"))]
    #[test]
    fn paranoia_precheck_treats_retry_limit_as_failure() {
        assert!(paranoia_run_did_not_converge(
            RipState::TrackComplete,
            &[RipEvent::RetryLimitReached]
        ));
        assert!(paranoia_run_did_not_converge(RipState::Aborted, &[]));
        assert!(!paranoia_run_did_not_converge(
            RipState::TrackComplete,
            &[RipEvent::ChecksumSatisfied]
        ));
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn accurip_pcm_checksum_matches_word_weighted_formula() {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut samples = Vec::new();
        for pair in bytes.chunks_exact(2) {
            samples.push(i16::from_le_bytes([pair[0], pair[1]]));
        }

        let pcm = PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: samples,
        };

        let expected = u32::from_le_bytes([1, 2, 3, 4]).wrapping_mul(1).wrapping_add(
            u32::from_le_bytes([5, 6, 7, 8]).wrapping_mul(2),
        );
        assert_eq!(accurip_v1_checksum_pcm(&pcm, false, false), expected);
    }

    #[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
    #[test]
    fn accurip_pcm_checksum_trims_first_and_last_track_boundary_frames() {
        // AccurateRip v1 skips the first 5 frames of the first track and the
        // last 5 frames of the last track; a value placed inside the trimmed
        // region must not influence the checksum.
        let samples_per_frame = CDDA_FRAME_BYTES / 4;
        let untrimmed_words = samples_per_frame * 6; // one extra word beyond the trimmed region
        let mut samples = Vec::new();
        for w in 0..untrimmed_words {
            samples.push(w as i16);
            samples.push(0i16);
        }
        let pcm = PcmTrackData {
            spec: PcmSpec {
                channels: 2,
                sample_rate: 44_100,
                bits_per_sample: 16,
            },
            interleaved_i16_samples: samples,
        };

        let checksum_first_track = accurip_v1_checksum_pcm(&pcm, true, false);
        let checksum_middle_track = accurip_v1_checksum_pcm(&pcm, false, false);
        assert_ne!(
            checksum_first_track, checksum_middle_track,
            "trimming the first-track lead-in must change the checksum"
        );
    }

    #[test]
    fn run_workflow_selects_physical_source_for_device_like_paths() {
        let settings = Settings {
            dev_path: Some("/dev/cdrom".to_string()),
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        assert_eq!(
            full_rip_source_from_settings(&settings),
            FullRipSource::Physical
        );
    }

    #[test]
    fn resolve_track_boundaries_prefers_metadata_over_defaults() {
        let track_numbers = vec![2u32, 4u32];
        let mut map: HashMap<u32, HashMap<String, String>> = HashMap::new();
        map.insert(
            2,
            [
                ("start_lsn".to_string(), "20".to_string()),
                ("frames".to_string(), "10".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        map.insert(
            4,
            [
                ("start_lsn".to_string(), "100".to_string()),
                ("end_lsn".to_string(), "115".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        let got = resolve_track_boundaries(&track_numbers, &map, 32, None);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].track_number, 2);
        assert_eq!(got[0].start_lsn, 20);
        assert_eq!(got[0].frame_count, 10);
        assert_eq!(got[1].track_number, 4);
        assert_eq!(got[1].start_lsn, 100);
        assert_eq!(got[1].frame_count, 16);
    }

    #[test]
    fn parse_image_toc_env_parses_valid_entries_and_skips_invalid() {
        unsafe {
            std::env::set_var(
                "CYANRIP_RS_IMAGE_TOC",
                "1:0-99,2:100-199,bad,3:300-299,0:10-20,4:400-450",
            );
        }

        let got = parse_image_toc_env();

        assert_eq!(got.get(&1), Some(&(0, 100)));
        assert_eq!(got.get(&2), Some(&(100, 100)));
        assert_eq!(got.get(&4), Some(&(400, 51)));
        assert!(!got.contains_key(&3));
        assert!(!got.contains_key(&0));

        unsafe {
            std::env::remove_var("CYANRIP_RS_IMAGE_TOC");
        }
    }

    #[test]
    fn resolve_track_boundaries_prefers_image_toc_over_metadata() {
        let track_numbers = vec![2u32, 4u32];
        let mut map: HashMap<u32, HashMap<String, String>> = HashMap::new();
        map.insert(
            2,
            [
                ("start_lsn".to_string(), "20".to_string()),
                ("frames".to_string(), "10".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        let mut toc = HashMap::new();
        toc.insert(2u32, (500, 25usize));
        toc.insert(4u32, (800, 30usize));

        let got = resolve_track_boundaries(&track_numbers, &map, 32, Some(&toc));

        assert_eq!(got.len(), 2);
        assert_eq!(got[0].track_number, 2);
        assert_eq!(got[0].start_lsn, 500);
        assert_eq!(got[0].frame_count, 25);
        assert_eq!(got[1].track_number, 4);
        assert_eq!(got[1].start_lsn, 800);
        assert_eq!(got[1].frame_count, 30);
    }

    #[test]
    fn parse_cue_track_starts_reads_index_01_values() {
        let cue = r#"
FILE "disc.bin" BINARY
  TRACK 01 AUDIO
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    INDEX 00 00:00:07
    INDEX 01 00:00:10
  TRACK 03 MODE1/2352
    INDEX 01 00:01:00
"#;

        let got = parse_cue_track_starts(cue);
        assert_eq!(got, vec![(1, 0), (2, 10), (3, 75)]);
    }

    #[test]
    fn parse_image_toc_from_cue_file_derives_next_track_ranges() {
        let cue_path = unique_temp_cue_path();
        std::fs::write(
            &cue_path,
            "FILE \"disc.bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 00:00:10\n",
        )
        .expect("cue fixture write should succeed");

        let got = parse_image_toc_from_cue_file(&cue_path, 32);

        assert_eq!(got.get(&1), Some(&(0, 10)));
        assert_eq!(got.get(&2), Some(&(10, 32)));

        let _ = std::fs::remove_file(&cue_path);
    }

    #[test]
    fn image_toc_overrides_from_settings_allows_env_to_override_cue() {
        let cue_path = unique_temp_cue_path();
        std::fs::write(
            &cue_path,
            "FILE \"disc.bin\" BINARY\n  TRACK 02 AUDIO\n    INDEX 01 00:00:20\n",
        )
        .expect("cue fixture write should succeed");

        let settings = Settings {
            dev_path: Some(cue_path.to_string_lossy().to_string()),
            ..Settings::default()
        };

        unsafe {
            std::env::set_var("CYANRIP_RS_IMAGE_TOC", "2:500-524");
        }
        let got = image_toc_overrides_from_settings(&settings, 32);
        unsafe {
            std::env::remove_var("CYANRIP_RS_IMAGE_TOC");
        }

        assert_eq!(got.get(&2), Some(&(500, 25)));

        let _ = std::fs::remove_file(&cue_path);
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
        let repo_tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp");
        std::fs::create_dir_all(&repo_tmp).expect("repo tmp root should be creatable");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        repo_tmp.join(format!("cyanrip-rs-output-dispatch-{now}"))
    }

    fn unique_temp_cue_path() -> PathBuf {
        let repo_tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp");
        std::fs::create_dir_all(&repo_tmp).expect("repo tmp root should be creatable");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        repo_tmp.join(format!("cyanrip-rs-image-toc-{now}.cue"))
    }

    #[test]
    fn initial_cover_arts_from_settings_supports_local_url_and_track_forms() {
        let root = unique_temp_output_root();
        std::fs::create_dir_all(&root).expect("temp root should be creatable");
        let local_cover = root.join("front.jpg");
        std::fs::write(&local_cover, [1u8, 2u8, 3u8]).expect("local cover fixture should be writable");

        let settings = Settings {
            cover_specs: vec![
                format!("Front={}", local_cover.to_string_lossy()),
                "Back=https://example.com/back.jpg".to_string(),
                "2=/tmp/track2.png".to_string(),
            ],
            ..Settings::default()
        };

        let arts =
            initial_cover_arts_from_settings(&settings, false).expect("cover specs should stage into initial cover arts");
        assert_eq!(arts.len(), 2, "only album-level cover specs should seed initial cover arts");
        assert_eq!(arts[0].title, "Front");
        assert_eq!(arts[0].data.as_deref(), Some(&[1u8, 2u8, 3u8][..]));
        assert_eq!(arts[0].extension.as_deref(), Some("jpg"));
        assert_eq!(arts[1].title, "Back");
        assert!(arts[1].data.is_none(), "URL cover should not be loaded as local bytes");

        let cleanup = std::fs::remove_dir_all(&root);
        assert!(cleanup.is_ok(), "temporary cover fixture root should be removable");
    }

    #[test]
    fn initial_cover_arts_from_settings_rejects_missing_local_file() {
        let settings = Settings {
            cover_specs: vec!["Front=/definitely/missing/front.jpg".to_string()],
            ..Settings::default()
        };

        let err = initial_cover_arts_from_settings(&settings, false).expect_err("missing local cover should fail");
        match err {
            RunWorkflowError::Runtime(msg) => {
                assert!(msg.contains("failed to read cover art source"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
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

        let settings = Settings {
            disable_mb: true,
            disable_accurip: true,
            ..Settings::default()
        };

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
    async fn surfaces_musicbrainz_multiple_releases_choice_instead_of_ripping_blind() {
        let cover_ids = Arc::new(Mutex::new(Vec::new()));
        let candidates = vec![
            ReleaseSummary {
                id: "rel-a".to_string(),
                album: "Album A".to_string(),
                disambiguation: None,
                country: Some("GB".to_string()),
                date: Some("2010-06-07".to_string()),
                num_cds: 1,
            },
            ReleaseSummary {
                id: "rel-b".to_string(),
                album: "Album B".to_string(),
                disambiguation: None,
                country: Some("US".to_string()),
                date: Some("2010-07-13".to_string()),
                num_cds: 1,
            },
        ];
        let mb = MbMock {
            called: Arc::new(Mutex::new(0usize)),
            result: Err(MusicBrainzError::MultipleReleases(candidates.clone())),
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

        // No release must be silently assumed when disambiguation is required.
        assert!(out.musicbrainz.is_none());
        assert_eq!(out.musicbrainz_release_choices, Some(candidates));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("multiple releases found") && w.contains("-R")),
            "expected multiple-releases warning; got {:?}",
            out.warnings
        );
        // Cover art/accurip lookups still proceed using no release id, matching other lookup failures.
        assert_eq!(cover_ids.lock().expect("lock").clone(), vec![None]);
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

        let settings = Settings {
            outputs: vec![OutputFormat::Wav, OutputFormat::Flac],
            folder_name_scheme: "{album} [{format}]".to_string(),
            track_name_scheme: "{track} - {title}".to_string(),
            discnumber: 1,
            totaldiscs: 2,
            ..Settings::default()
        };

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
            cover_arts: Vec::new(),
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
    fn flac_embeds_cover_art_by_default_when_available() {
        let output_root = unique_temp_output_root();

        let settings = Settings {
            outputs: vec![OutputFormat::Flac],
            folder_name_scheme: "{album} [{format}]".to_string(),
            track_name_scheme: "{track} - {title}".to_string(),
            ..Settings::default()
        };

        let album_meta: HashMap<String, String> = [
            ("album".to_string(), "Example Album".to_string()),
            ("album_artist".to_string(), "Example Artist".to_string()),
        ]
        .into_iter()
        .collect();

        let tracks = vec![TrackOutputInput {
            track_number: 1,
            track_meta: track_meta("01", "Intro"),
            pcm: sample_pcm(),
        }];

        let cover_bytes = vec![0xFF, 0xD8, 0xFF, 0xD9];
        let cover_arts = vec![CoverArtImage {
            title: "Front".to_string(),
            source: Some("test".to_string()),
            source_url: "front.jpg".to_string(),
            extension: Some("jpg".to_string()),
            data: Some(cover_bytes.clone()),
            content_type: Some("image/jpeg".to_string()),
        }];

        let result = write_track_outputs(TrackOutputFlowInput {
            settings,
            output_root: output_root.clone(),
            album_meta,
            cover_arts,
            tracks,
        })
        .expect("flac output should succeed");

        assert_eq!(result.written_files.len(), 1);

        let flac_tag =
            metaflac::Tag::read_from_path(output_root.join("Example Album [FLAC]/01 - Intro.flac"))
                .expect("flac tags should be readable");
        let pictures: Vec<&metaflac::block::Picture> = flac_tag.pictures().collect();

        assert_eq!(pictures.len(), 1, "one front cover should be embedded");
        assert_eq!(pictures[0].picture_type, metaflac::block::PictureType::CoverFront);
        assert_eq!(pictures[0].mime_type, "image/jpeg");
        assert_eq!(pictures[0].data, cover_bytes);

        let cleanup = std::fs::remove_dir_all(&output_root);
        assert!(cleanup.is_ok(), "temporary output root should be removable");
    }

    #[test]
    fn no_coverart_embed_skips_flac_picture_embedding() {
        let output_root = unique_temp_output_root();

        let settings = Settings {
            outputs: vec![OutputFormat::Flac],
            disable_coverart_embedding: true,
            folder_name_scheme: "{album} [{format}]".to_string(),
            track_name_scheme: "{track} - {title}".to_string(),
            ..Settings::default()
        };

        let album_meta: HashMap<String, String> = [
            ("album".to_string(), "Example Album".to_string()),
            ("album_artist".to_string(), "Example Artist".to_string()),
        ]
        .into_iter()
        .collect();

        let tracks = vec![TrackOutputInput {
            track_number: 1,
            track_meta: track_meta("01", "Intro"),
            pcm: sample_pcm(),
        }];

        let cover_arts = vec![CoverArtImage {
            title: "Front".to_string(),
            source: Some("test".to_string()),
            source_url: "front.jpg".to_string(),
            extension: Some("jpg".to_string()),
            data: Some(vec![0xFF, 0xD8, 0xFF, 0xD9]),
            content_type: Some("image/jpeg".to_string()),
        }];

        let result = write_track_outputs(TrackOutputFlowInput {
            settings,
            output_root: output_root.clone(),
            album_meta,
            cover_arts,
            tracks,
        })
        .expect("flac output should succeed with embedding disabled");

        assert_eq!(result.written_files.len(), 1);

        let flac_tag =
            metaflac::Tag::read_from_path(output_root.join("Example Album [FLAC]/01 - Intro.flac"))
                .expect("flac tags should be readable");
        assert_eq!(flac_tag.pictures().count(), 0);

        let cleanup = std::fs::remove_dir_all(&output_root);
        assert!(cleanup.is_ok(), "temporary output root should be removable");
    }

    #[test]
    fn rejects_unsupported_output_formats_in_dispatch() {
        let output_root = unique_temp_output_root();

        let settings = Settings {
            outputs: vec![OutputFormat::Mp3],
            ..Settings::default()
        };

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
            cover_arts: Vec::new(),
            tracks,
        })
        .expect_err("unsupported format should error");

        assert!(matches!(
            err,
            TrackOutputFlowError::UnsupportedOutputFormat(OutputFormat::Mp3)
        ));
    }

    #[test]
    fn writes_flac_when_hdcd_option_is_enabled() {
        let output_root = unique_temp_output_root();

        let settings = Settings {
            outputs: vec![OutputFormat::Flac],
            decode_hdcd: true,
            ..Settings::default()
        };

        let album_meta: HashMap<String, String> = [
            ("album".to_string(), "Example Album".to_string()),
            ("album_artist".to_string(), "Example Artist".to_string()),
        ]
        .into_iter()
        .collect();

        let tracks = vec![TrackOutputInput {
            track_number: 1,
            track_meta: track_meta("01", "Intro"),
            pcm: sample_pcm(),
        }];

        let result = write_track_outputs(TrackOutputFlowInput {
            settings,
            output_root: output_root.clone(),
            album_meta,
            cover_arts: Vec::new(),
            tracks,
        })
        .expect("hdcd option should be handled without processing failure");

        assert_eq!(result.written_files.len(), 1);
        let output_path = output_root.join("Example Album [FLAC]/01 - Intro.flac");
        assert!(output_path.exists(), "expected hdcd-enabled output path to exist");

        let reader = claxon::FlacReader::open(&output_path)
            .expect("written flac should be readable after hdcd processing");
        assert_eq!(reader.streaminfo().bits_per_sample, 24);

        let cleanup = std::fs::remove_dir_all(&output_root);
        assert!(cleanup.is_ok(), "temporary output root should be removable");
    }
}
