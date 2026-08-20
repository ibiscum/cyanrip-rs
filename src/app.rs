use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audio::flac::write_flac_file;
use crate::audio::wav::write_wav_file;
use crate::audio::{PcmSpec, PcmTrackData};
use crate::cdda::paranoia::{RetryPolicy, RipState};
use crate::cdda::reader::run_track_with_paranoia_heuristics_interruptible;
use crate::cdda::reader::{
    CDDA_FRAME_BYTES, CddaFrameReader, FaultInjectedImageReader, ParanoiaHeuristicConfig,
};
use crate::cue::{CueDoc, CueFileType, CueTrack, render_cue};
use crate::metadata::accurip::{
    AccuDbStatus, AccuRipError, AccuRipLookupResult, AccuRipService, AccuRipTrackInput,
};
use crate::metadata::coverart::{CoverArtError, CoverArtImage, CoverArtService};
use crate::metadata::discid::{DiscTrack, DiscidInfo, compute_discid};
use crate::metadata::musicbrainz::{MusicBrainzError, MusicBrainzReleaseMeta, MusicBrainzService};
use crate::naming::{NamingContext, build_track_relative_path};
use crate::{DriverKind, OutputFormat, ReleaseSelection, Settings, open_dev_kind};

const DEFAULT_SYNTHETIC_FRAME_COUNT: usize = 32;
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
const FIND_OFFSET_INITIAL_RADIUS_FRAMES: usize = 6;
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
const FIND_OFFSET_MAX_RADIUS_FRAMES: usize = 1536;

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use tokio::runtime::Builder as TokioRuntimeBuilder;
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
use crate::metadata::accurip::{AccuRipDbEntry, compute_accurip_ids, find_accurip_confidence};

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
        .filter_map(|o| match o {
            OutputFormat::Flac => Some("flac"),
            OutputFormat::Wav => Some("wav"),
            OutputFormat::Mp3 => Some("mp3"),
            OutputFormat::Tta => Some("tta"),
            OutputFormat::Opus => Some("opus"),
            OutputFormat::Aac => Some("aac"),
            OutputFormat::AacMp4 => Some("aac_mp4"),
            OutputFormat::Wavpack => Some("wavpack"),
            OutputFormat::Vorbis => Some("vorbis"),
            OutputFormat::Alac => Some("alac"),
            OutputFormat::AlacMp4 => Some("alac_mp4"),
            OutputFormat::OpusMp4 => Some("opus_mp4"),
            OutputFormat::Pcm => Some("pcm"),
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
) -> String {
    let mut out = render_info_only_report(settings, drive_used);

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
            out.push_str(&format!("CDDB ID:        {cddb_str}\n"));
        }
        out.push_str(&format!("Total time:     {}\n", format_msf_from_frames(total_frames)));

        out.push_str("\nTracks:\n");
        for track in toc {
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
            out.push('\n');
        }
    }

    out
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

    let report = render_info_only_report_with_toc(
        settings,
        drive_used.as_deref(),
        &toc_entries,
        discid_parts
            .as_ref()
            .map(|(id, cddb, url)| (id.as_str(), cddb.as_str(), url.as_str())),
    );
    Ok(report)
}

#[cfg(not(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys")))]
fn run_info_only_mode(settings: &Settings) -> Result<String, RunWorkflowError> {
    Ok(render_info_only_report(settings, None))
}

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
    use crate::cdda::linux_drive::read_drive_toc_tracks;

    let mut lines = render_find_offset_report_header(settings);
    let toc = read_drive_toc_tracks(settings.dev_path.as_deref())
        .map_err(|e| RunWorkflowError::Runtime(format!("TOC read failed: {e:?}")))?;
    if toc.is_empty() {
        lines.push("Status: no tracks detected on drive".to_string());
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
    let disc_ids = compute_accurip_ids(&ar_tracks)
        .map_err(|e| RunWorkflowError::Runtime(format!("accurip id computation failed: {e:?}")))?;

    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| RunWorkflowError::Runtime(format!("tokio runtime init failed: {e}")))?;
    let service = AccuRipService::default();
    let lookup = runtime
        .block_on(service.lookup(&ar_tracks, cddb_id))
        .map_err(|e| RunWorkflowError::Runtime(format!("accurip lookup failed: {e:?}")))?;

    lines.push(format!("AccuRip request: {}", lookup.request_url));
    lines.push(format!("AccuRip status: {:?}", lookup.status));

    if lookup.status != AccuDbStatus::Found {
        lines.push("Status: no matching AccurateRip entry; cannot determine drive offset".to_string());
        return Ok(lines.join("\n"));
    }

    let mut radius = FIND_OFFSET_INITIAL_RADIUS_FRAMES;
    let mut offset_found_confidence = 0i32;
    let mut offset_found_samples = 0i32;
    let mut had_any_ar = false;
    let mut had_any_eligible_track = false;
    let mut exhausted_radius = false;

    while radius <= FIND_OFFSET_MAX_RADIUS_FRAMES {
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

            let start_lsn = (track.start_lsn as i32)
                .saturating_add(450)
                .saturating_sub(radius as i32)
                .max(0);
            let window = read_drive_window(
                settings.dev_path.as_deref(),
                start_lsn,
                2 * radius + 1,
            )?;
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
        if next_radius > FIND_OFFSET_MAX_RADIUS_FRAMES {
            exhausted_radius = true;
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
            "Status: drive offset found: {:+} samples (confidence {})",
            offset_found_samples, offset_found_confidence
        ));
    } else if !had_any_ar {
        lines.push("Status: no track had AccurateRip entry; cannot determine drive offset".to_string());
    } else if !had_any_eligible_track {
        lines.push("Status: no track was long enough for offset probing".to_string());
    } else if exhausted_radius {
        lines.push(format!(
            "Status: unable to find drive offset up to radius {} frames",
            FIND_OFFSET_MAX_RADIUS_FRAMES
        ));
    } else {
        lines.push("Status: unable to find drive offset".to_string());
    }

    lines.push(format!("DiscID: {}", discid.musicbrainz_discid));
    lines.push(format!("CDDB: {}", discid.cddb));
    lines.push(format!("AccuRip IDs: {:08x}/{:08x}", disc_ids.id_type_1, disc_ids.id_type_2));
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

            CueTrack {
                number: idx,
                index: idx,
                is_data: false,
                preemphasis: false,
                file_path,
                cue_path: None,
                file_type: CueFileType::Wave,
                title,
                performer,
                isrc: None,
                pregap_lsn: None,
                start_lsn: 0,
                start_lsn_sig: 0,
                dropped_pregap_start: None,
                merged_pregap_end: None,
                previous_start_lsn_sig: None,
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

fn env_var_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let n = v.trim().to_ascii_lowercase();
            n == "1" || n == "true" || n == "yes" || n == "on"
        }
        Err(_) => false,
    }
}

fn default_synthetic_output_root() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("cyanrip-rs-synthetic-rip-{now}"))
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
    let output_root = match std::env::var("CYANRIP_RS_OUTPUT_ROOT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => default_synthetic_output_root(),
    };

    let mut album_meta: HashMap<String, String> = parse_album_metadata_map(settings.album_metadata.as_deref())
        .into_iter()
        .collect();

    album_meta
        .entry("album".to_string())
        .or_insert_with(|| "Synthetic Album".to_string());
    album_meta
        .entry("album_artist".to_string())
        .or_insert_with(|| "Synthetic Artist".to_string());

    let mut tracks = Vec::new();
    for entry in &settings.track_metadata {
        if let Some((idx, fields)) = parse_track_meta_entry(entry) {
            let mut track_meta: HashMap<String, String> = fields.into_iter().collect();
            track_meta
                .entry("track".to_string())
                .or_insert_with(|| format!("{idx:02}"));
            track_meta
                .entry("title".to_string())
                .or_insert_with(|| format!("Synthetic Track {idx:02}"));
            let pcm = synthetic_track_pcm_for_source()?;
            tracks.push(TrackOutputInput {
                track_number: idx,
                track_meta,
                pcm,
            });
        }
    }

    if tracks.is_empty() {
        let mut track_meta = HashMap::new();
        track_meta.insert("track".to_string(), "01".to_string());
        track_meta.insert("title".to_string(), "Synthetic Track 01".to_string());
        let pcm = synthetic_track_pcm_for_source()?;
        tracks.push(TrackOutputInput {
            track_number: 1,
            track_meta,
            pcm,
        });
    }

    let result = write_track_outputs(TrackOutputFlowInput {
        settings: settings.clone(),
        output_root: output_root.clone(),
        album_meta,
        tracks,
    })
    .map_err(|e| RunWorkflowError::Runtime(format!("synthetic full-rip failed: {e}")))?;

    let mut out = String::new();
    out.push_str("cyanrip-rs synthetic full-rip mode\n");
    out.push_str(&format!("Output root: {}\n", output_root.display()));
    out.push_str(&format!("Written files: {}\n", result.written_files.len()));
    for file in &result.written_files {
        out.push_str(&format!("FILE {}\n", file.absolute_path.display()));
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FullRipSource {
    Image,
    Physical,
}

fn full_rip_source_from_settings(settings: &Settings) -> FullRipSource {
    match settings.dev_path.as_deref() {
        Some(path) => match open_dev_kind(path) {
            DriverKind::BinCue | DriverKind::Cue | DriverKind::Nrg | DriverKind::CdrDao => {
                FullRipSource::Image
            }
            DriverKind::Unknown => FullRipSource::Physical,
        },
        None => FullRipSource::Image,
    }
}

#[cfg(all(target_os = "linux", feature = "cdda"))]
fn acquire_track_pcm_from_physical_reader(
    settings: &Settings,
    frame_count: usize,
    start_lsn: i32,
) -> Result<PcmTrackData, RunWorkflowError> {
    use crate::cdda::linux_drive::{open_linux_physical_drive, run_paranoia_on_linux_drive_with_defaults_for_level};

    if settings.paranoia_level > 0 {
        let mut retry_policy = if settings.ripping_retries > 0 {
            RetryPolicy::new(
                settings.ripping_retries as u32,
                settings.max_retries.max(1) as u32,
            )
        } else {
            RetryPolicy::disabled()
        };

        let run = run_paranoia_on_linux_drive_with_defaults_for_level(
            settings.dev_path.as_deref(),
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
        )
        .map_err(|e| RunWorkflowError::Runtime(format!("physical paranoia run failed: {e:?}")))?;

        if run.state != RipState::TrackComplete {
            return Err(RunWorkflowError::Runtime(format!(
                "physical paranoia run did not complete track: {:?}",
                run.state
            )));
        }
    }

    let mut reader = open_linux_physical_drive(settings.dev_path.as_deref()).map_err(|e| {
        RunWorkflowError::Runtime(format!("physical drive open failed: {e:?}"))
    })?;
    acquire_track_pcm_from_reader(&mut reader, start_lsn, frame_count)
}

#[cfg(not(all(target_os = "linux", feature = "cdda")))]
fn acquire_track_pcm_from_physical_reader(
    _settings: &Settings,
    _frame_count: usize,
    _start_lsn: i32,
) -> Result<PcmTrackData, RunWorkflowError> {
    Err(RunWorkflowError::Runtime(
        "physical drive reader requires linux + cdda feature support".to_string(),
    ))
}

fn selected_track_numbers(settings: &Settings) -> Vec<u32> {
    if settings.rip_indices.is_empty() {
        return vec![1];
    }

    settings
        .rip_indices
        .iter()
        .filter(|n| **n > 0)
        .map(|n| *n as u32)
        .collect()
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

fn parse_i32_meta(map: &HashMap<String, String>, key: &str) -> Option<i32> {
    map.get(key).and_then(|v| v.trim().parse::<i32>().ok())
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
    boundaries: &[TrackBoundary],
) -> Result<Vec<(u32, PcmTrackData)>, RunWorkflowError> {
    let max_frame_end = boundaries
        .iter()
        .map(|b| (b.start_lsn.max(0) as usize).saturating_add(b.frame_count))
        .max()
        .unwrap_or(DEFAULT_SYNTHETIC_FRAME_COUNT);
    let total_frames = max_frame_end.max(DEFAULT_SYNTHETIC_FRAME_COUNT);
    let frames = build_synthetic_frames(total_frames);
    let mut reader = FaultInjectedImageReader::new(frames);

    let mut out = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        let start_lsn = boundary.start_lsn;
        let frame_count = boundary.frame_count;

        if settings.paranoia_level > 0 {
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
            )
            .map_err(|e| {
                RunWorkflowError::Runtime(format!("image paranoia run failed: {e:?}"))
            })?;

            if run.state != RipState::TrackComplete {
                return Err(RunWorkflowError::Runtime(format!(
                    "image paranoia run did not complete track: {:?}",
                    run.state
                )));
            }
        }

        let pcm = acquire_track_pcm_from_reader(&mut reader, start_lsn, frame_count)?;
        out.push((boundary.track_number, pcm));
    }

    Ok(out)
}

fn acquire_tracks_pcm_from_physical_reader(
    settings: &Settings,
    boundaries: &[TrackBoundary],
) -> Result<Vec<(u32, PcmTrackData)>, RunWorkflowError> {
    let mut out = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        let start_lsn = boundary.start_lsn;
        let frame_count = boundary.frame_count;
        let pcm = acquire_track_pcm_from_physical_reader(settings, frame_count, start_lsn)?;
        out.push((boundary.track_number, pcm));
    }
    Ok(out)
}

fn run_full_rip_from_selected_source(settings: &Settings) -> Result<String, RunWorkflowError> {
    let source = full_rip_source_from_settings(settings);
    let track_numbers = selected_track_numbers(settings);
    let default_frame_count = configured_frame_count();
    let track_meta_map = track_meta_map_from_settings(settings);
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
    let boundaries = resolve_track_boundaries(
        &track_numbers,
        &track_meta_map,
        default_frame_count,
        image_toc_overrides.as_ref(),
    );
    let pcm_tracks = match source {
        FullRipSource::Image => {
            acquire_tracks_pcm_from_image_reader(settings, &boundaries)?
        }
        FullRipSource::Physical => {
            acquire_tracks_pcm_from_physical_reader(settings, &boundaries)?
        }
    };

    let output_root = match std::env::var("CYANRIP_RS_OUTPUT_ROOT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => default_synthetic_output_root(),
    };

    let mut album_meta: HashMap<String, String> = parse_album_metadata_map(settings.album_metadata.as_deref())
        .into_iter()
        .collect();
    album_meta
        .entry("album".to_string())
        .or_insert_with(|| "Runtime Album".to_string());
    album_meta
        .entry("album_artist".to_string())
        .or_insert_with(|| "Runtime Artist".to_string());

    let tracks: Vec<TrackOutputInput> = pcm_tracks
        .into_iter()
        .map(|(track_number, pcm)| TrackOutputInput {
            track_number,
            track_meta: track_meta_for_number(track_number, &track_meta_map),
            pcm,
        })
        .collect();

    let result = write_track_outputs(TrackOutputFlowInput {
        settings: settings.clone(),
        output_root: output_root.clone(),
        album_meta,
        tracks,
    })
    .map_err(|e| RunWorkflowError::Runtime(format!("full-rip writer flow failed: {e}")))?;

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
    out.push_str(&format!("Written files: {}\n", result.written_files.len()));
    for boundary in &boundaries {
        out.push_str(&format!(
            "TRACK {} START_LSN {} FRAMES {}\n",
            boundary.track_number, boundary.start_lsn, boundary.frame_count
        ));
    }
    for file in &result.written_files {
        out.push_str(&format!("FILE {}\n", file.absolute_path.display()));
    }
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
        return Ok(Some(render_cue_only_preview(settings)));
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

        let out = run_workflow(&settings).expect("find-offset mode should be wired");
        let report = out.expect("find-offset mode should produce a report");
        assert!(report.contains("cyanrip-rs find-offset mode"));
        assert!(report.contains("AccurateRip: enabled"));
    }

    #[test]
    fn run_workflow_info_mode_returns_report() {
        let settings = Settings {
            print_info_only: true,
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        let out = run_workflow(&settings).expect("info-only mode should be wired");
        let report = out.expect("info-only mode should produce a report");
        assert!(report.contains("cyanrip-rs "));
        assert!(report.contains("Paranoia level: "));
        assert!(report.contains("Outputs:        "));
        assert!(report.contains("AccurateRip:    "));
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

        let report = render_info_only_report_with_toc(&settings, None, &toc, None);
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
    fn run_workflow_cue_only_mode_returns_preview() {
        let settings = Settings {
            generate_cue_only: true,
            album_metadata: Some("album=Example Album:album_artist=Example Artist".to_string()),
            track_metadata: vec![
                "1=title=Intro:artist=Example Artist".to_string(),
                "2=title=Outro:artist=Example Artist".to_string(),
            ],
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };

        let out = run_workflow(&settings).expect("cue-only mode should be wired");
        let cue = out.expect("cue-only mode should produce CUE text");
        assert!(cue.contains("cyanrip-rs cue-only preview"));
        assert!(cue.contains("TITLE \"Example Album\""));
        assert!(cue.contains("TRACK 01 AUDIO"));
        assert!(cue.contains("TRACK 02 AUDIO"));
    }

    #[test]
    fn run_workflow_selects_image_source_for_default_and_cue_paths() {
        let settings_default = Settings {
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        assert_eq!(full_rip_source_from_settings(&settings_default), FullRipSource::Image);

        let settings_cue = Settings {
            dev_path: Some("disc.cue".to_string()),
            outputs: vec![OutputFormat::Flac],
            ..Settings::default()
        };
        assert_eq!(full_rip_source_from_settings(&settings_cue), FullRipSource::Image);
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
        assert!(got.get(&3).is_none());
        assert!(got.get(&0).is_none());

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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cyanrip-rs-output-dispatch-{now}"))
    }

    fn unique_temp_cue_path() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cyanrip-rs-image-toc-{now}.cue"))
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
