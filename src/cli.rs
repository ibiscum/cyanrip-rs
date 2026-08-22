use clap::{CommandFactory, Parser};

use crate::{
    MAX_PARANOIA_LEVEL, Settings, apply_pregap_entries,
    calc_over_under_read_frames, parse_cover_size, parse_disc, parse_outputs, parse_paranoia,
    parse_release, parse_sanitize, parse_track_indices, validate_folder_scheme,
    validate_mode_combo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliAction {
    Run,
    ShowOutputsHelp,
    VerifyLog,
}

pub const SUPPORTED_OUTPUTS_HELP: &str = "Supported output codecs:\n    flac\n    mp3\n    tta\n    opus\n    aac\n    aac_mp4\n    wavpack\n    vorbis\n    alac\n    alac_mp4\n    wav\n    opus_mp4\n    pcm";

#[derive(Debug, Clone, PartialEq)]
pub struct CliConfig {
    pub settings: Settings,
    pub action: CliAction,
}

#[derive(Debug, Parser)]
#[command(
    name = "cyanrip-rs",
    version,
    about = "Rust migration of cyanrip",
    long_about = None,
    disable_help_subcommand = true,
    help_template = "{name} {version}\n\n{about-with-newline}{usage-heading} {usage}\n\n{all-args}"
)]
pub struct CliArgs {
    #[arg(
        short = 'd',
        long = "device",
        visible_alias = "dev_path",
        help_heading = "Ripping options",
        help = "Set device path (can be a TOC file)"
    )]
    pub device: Option<String>,

    #[arg(
        short = 's',
        long = "offset",
        help_heading = "Ripping options",
        help = "CD drive offset in samples"
    )]
    pub offset: Option<i32>,

    #[arg(
        short = 'r',
        long = "retries",
        default_value_t = 10,
        help_heading = "Ripping options",
        help = "Maximum number of retries for frames and repeated rips"
    )]
    pub retries: i32,

    #[arg(
        short = 'Z',
        long = "repeat-rips",
        visible_alias = "repeat_rips",
        default_value_t = 0,
        help_heading = "Ripping options",
        help = "Rip tracks until checksums match N times (for damaged CDs)"
    )]
    pub repeat_rips: i32,

    #[arg(
        short = 'S',
        long = "speed",
        default_value_t = 0,
        help_heading = "Ripping options",
        help = "Set drive speed"
    )]
    pub speed: i32,

    #[arg(
        short = 'p',
        long = "pregap",
        help_heading = "Ripping options",
        help = "Track pregap handling: N=default|drop|merge|track (repeatable)"
    )]
    pub pregap: Vec<String>,

    #[arg(
        short = 'P',
        long = "paranoia",
        help_heading = "Ripping options",
        help = "Paranoia level (0..max, or 'none'/'max')"
    )]
    pub paranoia: Option<String>,

    #[arg(
        short = 'O',
        long = "overread",
        default_value_t = false,
        help_heading = "Ripping options",
        help = "Enable overreading into lead-in and lead-out"
    )]
    pub overread: bool,

    #[arg(
        short = 'H',
        long = "hdcd",
        default_value_t = false,
        help_heading = "Ripping options",
        help = "Enable HDCD decoding"
    )]
    pub hdcd: bool,

    #[arg(
        short = 'E',
        long = "force-deemphasis",
        visible_alias = "force_deemphasis",
        default_value_t = false,
        help_heading = "Ripping options",
        help = "Force CD deemphasis"
    )]
    pub force_deemphasis: bool,

    #[arg(
        short = 'W',
        long = "no-deemphasis",
        visible_alias = "no_deemphasis",
        default_value_t = false,
        help_heading = "Ripping options",
        help = "Disable automatic CD deemphasis"
    )]
    pub no_deemphasis: bool,

    #[arg(
        short = 'K',
        long = "no-replaygain",
        visible_alias = "no_replaygain",
        default_value_t = false,
        help_heading = "Ripping options",
        help = "Disable ReplayGain tagging"
    )]
    pub no_replaygain: bool,

    #[arg(
        short = 'o',
        long = "outputs",
        help_heading = "Output options",
        help = "Comma separated list of output formats ('help' lists all)"
    )]
    pub outputs: Option<String>,

    #[arg(
        short = 'b',
        long = "bitrate",
        default_value_t = 256.0,
        help_heading = "Output options",
        help = "Bitrate of lossy files in kbps"
    )]
    pub bitrate: f32,

    #[arg(
        short = 'D',
        long = "folder-scheme",
        visible_alias = "folder_scheme",
        default_value = "{album}{if #releasecomment# > #0# (|releasecomment|)} [{format}]"
        ,help_heading = "Output options"
        ,help = "Directory naming scheme"
    )]
    pub folder_scheme: String,

    #[arg(
        short = 'F',
        long = "track-scheme",
        visible_alias = "track_scheme",
        default_value = "{if #totaldiscs# > #1#|disc|.}{track} - {title}"
        ,help_heading = "Output options"
        ,help = "Track naming scheme"
    )]
    pub track_scheme: String,

    #[arg(
        short = 'L',
        long = "log-scheme",
        visible_alias = "log_scheme",
        default_value = "{album}{if #totaldiscs# > #1# CD|disc|}"
        ,help_heading = "Output options"
        ,help = "Log file name scheme"
    )]
    pub log_scheme: String,

    #[arg(
        short = 'M',
        long = "cue-scheme",
        visible_alias = "cue_scheme",
        default_value = "{album}{if #totaldiscs# > #1# CD|disc|}"
        ,help_heading = "Output options"
        ,help = "CUE file name scheme"
    )]
    pub cue_scheme: String,

    #[arg(
        short = 'l',
        long = "tracks",
        help_heading = "Output options",
        help = "Comma separated list of tracks to rip (default: all)"
    )]
    pub tracks: Option<String>,

    #[arg(
        short = 'T',
        long = "sanitize",
        help_heading = "Output options",
        help = "Filename sanitation: simple, os_simple, unicode, os_unicode"
    )]
    pub sanitize: Option<String>,

    #[arg(
        short = 'I',
        long = "info",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Only print CD and track info"
    )]
    pub info: bool,

    #[arg(
        short = 'J',
        long = "cue-only",
        visible_alias = "cue_only",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Only generate and print a CUE sheet, don't rip"
    )]
    pub cue_only: bool,

    #[arg(
        short = 'R',
        long = "release",
        help_heading = "Metadata options",
        help = "MusicBrainz release: 1-based index or ID string"
    )]
    pub release: Option<String>,

    #[arg(
        short = 'c',
        long = "disc",
        help_heading = "Metadata options",
        help = "Multi-disc tag: disc/totaldiscs"
    )]
    pub disc: Option<String>,

    #[arg(
        short = 'a',
        long = "album-meta",
        visible_alias = "album_meta",
        help_heading = "Metadata options",
        help = "Album metadata, key=value:key=value"
    )]
    pub album_meta: Option<String>,

    #[arg(
        short = 't',
        long = "track-meta",
        visible_alias = "track_meta",
        help_heading = "Metadata options",
        help = "Track metadata as N=key=value:key=value (repeatable)"
    )]
    pub track_meta: Vec<String>,

    #[arg(
        short = 'C',
        long = "cover",
        help_heading = "Metadata options",
        help = "Cover art: title=path (or N=path per-track, repeatable)"
    )]
    pub cover: Vec<String>,

    #[arg(
        short = 'N',
        long = "no-musicbrainz",
        visible_alias = "no_musicbrainz",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Disable MusicBrainz lookup"
    )]
    pub no_musicbrainz: bool,

    #[arg(
        short = 'A',
        long = "no-accurip",
        visible_alias = "no_accurip",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Disable AccurateRip database query and validation"
    )]
    pub no_accurip: bool,

    #[arg(
        short = 'U',
        long = "no-coverart-db",
        visible_alias = "no_coverart_db",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Disable Cover art DB query and retrieval"
    )]
    pub no_coverart_db: bool,

    #[arg(
        short = 'm',
        long = "cover-size",
        visible_alias = "cover_size",
        default_value_t = -1,
        help_heading = "Metadata options",
        help = "Cover art max size: 250, 500, 1200, or -1 for original"
    )]
    pub cover_size: i32,

    #[arg(
        short = 'G',
        long = "no-coverart-embed",
        visible_alias = "no_coverart_embed",
        default_value_t = false,
        help_heading = "Metadata options",
        help = "Disable embedding of cover art images"
    )]
    pub no_coverart_embed: bool,

    #[arg(
        short = 'Q',
        long = "eject",
        default_value_t = false,
        help_heading = "Misc. options",
        help = "Eject tray once successfully done"
    )]
    pub eject: bool,

    #[arg(
        short = 'f',
        long = "find-offset",
        visible_alias = "find_offset",
        default_value_t = false,
        help_heading = "Misc. options",
        help = "Find drive offset (requires a disc with an AccuRip entry)"
    )]
    pub find_offset: bool,

    #[arg(
        short = 'Y',
        long = "verify-log",
        visible_alias = "verify_log",
        help_heading = "Misc. options",
        help = "Verify a rip log's FUN512 checksum"
    )]
    pub verify_log: Option<String>,
}

impl CliArgs {
    pub fn to_config(&self) -> Result<CliConfig, String> {
        let mut settings = Settings::default();
        let mut action = CliAction::Run;

        settings.dev_path = self.device.clone();
        settings.verify_log = self.verify_log.clone();
        if settings.verify_log.is_some() {
            action = CliAction::VerifyLog;
            return Ok(CliConfig { settings, action });
        }

        let offset = self.offset.unwrap_or(0);
        settings.offset = offset;
        settings.offset_is_set = self.offset.is_some();
        settings.over_under_read_frames = calc_over_under_read_frames(offset);
        settings.max_retries = self.retries;
        settings.ripping_retries = self.repeat_rips;
        settings.speed = self.speed;
        settings.bitrate_kbps = self.bitrate;
        settings.overread_leadinout = self.overread;
        settings.decode_hdcd = self.hdcd;
        settings.force_deemphasis = self.force_deemphasis;
        settings.deemphasis = !self.no_deemphasis;
        settings.enable_replaygain = !self.no_replaygain;
        settings.disable_mb = self.no_musicbrainz;
        settings.disable_accurip = self.no_accurip;
        settings.disable_coverart_db = self.no_coverart_db;
        settings.disable_coverart_embedding = self.no_coverart_embed;
        settings.print_info_only = self.info;
        settings.generate_cue_only = self.cue_only;
        settings.eject_on_success_rip = self.eject;
        settings.find_drive_offset = self.find_offset;
        settings.album_metadata = self.album_meta.clone();
        settings.track_metadata = self.track_meta.clone();
        settings.cover_specs = self.cover.clone();

        settings.folder_name_scheme = self.folder_scheme.clone();
        settings.track_name_scheme = self.track_scheme.clone();
        settings.log_name_scheme = self.log_scheme.clone();
        settings.cue_name_scheme = self.cue_scheme.clone();

        if let Some(ref paranoia) = self.paranoia {
            settings.paranoia_level = parse_paranoia(paranoia, MAX_PARANOIA_LEVEL)?;
        }

        settings.coverart_lookup_size = parse_cover_size(self.cover_size)?;

        if let Some(ref sanitize) = self.sanitize {
            settings.sanitize_method = parse_sanitize(sanitize)?;
        }

        if let Some(ref outputs_raw) = self.outputs {
            let outputs_list = parse_csv(outputs_raw);
            let outputs_refs: Vec<&str> = outputs_list.iter().map(String::as_str).collect();
            let outputs = parse_outputs(&outputs_refs)?;
            if outputs.is_empty() {
                action = CliAction::ShowOutputsHelp;
            } else {
                settings.outputs = outputs;
            }
        }

        if action == CliAction::ShowOutputsHelp {
            return Ok(CliConfig { settings, action });
        }

        if let Some(ref tracks_raw) = self.tracks {
            let tracks_list = parse_csv(tracks_raw);
            let mut parsed = Vec::with_capacity(tracks_list.len());
            for raw in tracks_list {
                let idx = raw
                    .parse::<i32>()
                    .map_err(|_| format!("Invalid track index {raw}"))?;
                parsed.push(idx);
            }
            let normalized = parse_track_indices(&parsed)?;
            settings.rip_indices_count = normalized.len() as i32;
            settings.rip_indices = normalized;
        }

        if !self.pregap.is_empty() {
            let pregap_refs: Vec<&str> = self.pregap.iter().map(String::as_str).collect();
            apply_pregap_entries(&mut settings.pregap_action, &pregap_refs)?;
        }

        validate_folder_scheme(settings.outputs.len(), &settings.folder_name_scheme)?;
        validate_mode_combo(settings.print_info_only, self.cue_only)?;

        if settings.generate_cue_only {
            settings.disable_accurip = true;
            settings.disable_coverart_db = true;
        }

        if settings.print_info_only {
            // Info-only mode never performs ripping, so eject is effectively disabled.
            settings.eject_on_success_rip = false;
        }

        if settings.find_drive_offset {
            settings.disable_accurip = false;
            settings.disable_mb = true;
            settings.disable_coverart_db = true;
            settings.offset = 0;
            settings.over_under_read_frames = 0;
            settings.eject_on_success_rip = false;
        }

        settings.release = if let Some(ref value) = self.release {
            Some(parse_release(value)?)
        } else {
            None
        };

        if let Some(ref value) = self.disc {
            let (discnumber, totaldiscs) = parse_disc(value)?;
            settings.discnumber = discnumber;
            settings.totaldiscs = totaldiscs;
        }

        Ok(CliConfig {
            settings,
            action,
        })
    }
}

pub fn parse_from_env() -> Result<CliConfig, String> {
    let args = CliArgs::parse();
    args.to_config()
}

pub fn parse_from_iter<I, T>(iter: I) -> Result<CliConfig, String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = CliArgs::try_parse_from(iter).map_err(|e| e.to_string())?;
    args.to_config()
}

pub fn render_help() -> String {
    let mut cmd = CliArgs::command();
    let mut out = Vec::new();
    let _ = cmd.write_long_help(&mut out);
    String::from_utf8_lossy(&out).to_string()
}

fn parse_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CoverArtLookupSize, OutputFormat, PregapAction, ReleaseSelection, SanitizeMethod,
    };
    use std::collections::BTreeSet;

    #[test]
    fn maps_defaults() {
        let cfg = parse_from_iter(["cyanrip-rs"]).unwrap();
        assert_eq!(cfg.settings, Settings::default());
        assert_eq!(cfg.action, CliAction::Run);
    }

    #[test]
    fn maps_basic_flags_to_settings() {
        let cfg = parse_from_iter([
            "cyanrip-rs",
            "-s",
            "589",
            "-r",
            "15",
            "-Z",
            "2",
            "-S",
            "4",
            "-b",
            "320",
            "-H",
            "-E",
            "-W",
            "-K",
            "-N",
            "-A",
            "-U",
            "-G",
            "-Q",
        ])
        .unwrap();

        assert_eq!(cfg.settings.offset, 589);
        assert_eq!(cfg.settings.over_under_read_frames, 2);
        assert_eq!(cfg.settings.max_retries, 15);
        assert_eq!(cfg.settings.ripping_retries, 2);
        assert_eq!(cfg.settings.speed, 4);
        assert_eq!(cfg.settings.bitrate_kbps, 320.0);
        assert!(cfg.settings.decode_hdcd);
        assert!(cfg.settings.force_deemphasis);
        assert!(!cfg.settings.deemphasis);
        assert!(!cfg.settings.enable_replaygain);
        assert!(cfg.settings.disable_mb);
        assert!(cfg.settings.disable_accurip);
        assert!(cfg.settings.disable_coverart_db);
        assert!(cfg.settings.disable_coverart_embedding);
        assert!(cfg.settings.eject_on_success_rip);
    }

    #[test]
    fn maps_outputs_tracks_and_pregap() {
        let cfg = parse_from_iter([
            "cyanrip-rs",
            "-o",
            "flac,mp3",
            "-l",
            "5,1,3",
            "-p",
            "1=drop",
            "-p",
            "3=merge",
        ])
        .unwrap();

        assert_eq!(cfg.settings.outputs, vec![OutputFormat::Flac, OutputFormat::Mp3]);
        assert_eq!(cfg.settings.rip_indices, vec![1, 3, 5]);
        assert_eq!(cfg.settings.rip_indices_count, 3);
        assert_eq!(cfg.settings.pregap_action[0], PregapAction::Drop);
        assert_eq!(cfg.settings.pregap_action[2], PregapAction::Merge);
    }

    #[test]
    fn parses_extended_fields() {
        let cfg = parse_from_iter([
            "cyanrip-rs",
            "-P",
            "max",
            "-T",
            "os_unicode",
            "-m",
            "500",
            "-R",
            "2",
            "-c",
            "1/2",
        ])
        .unwrap();

        assert_eq!(cfg.settings.paranoia_level, 3);
        assert_eq!(cfg.settings.sanitize_method, SanitizeMethod::OsUnicode);
        assert_eq!(cfg.settings.coverart_lookup_size, CoverArtLookupSize::Px500);
        assert_eq!(cfg.settings.release, Some(ReleaseSelection::Index(2)));
        assert_eq!(cfg.settings.discnumber, 1);
        assert_eq!(cfg.settings.totaldiscs, 2);
    }

    #[test]
    fn rejects_invalid_modes() {
        let err = parse_from_iter(["cyanrip-rs", "-I", "-J"]).unwrap_err();
        assert_eq!(
            err,
            "-J (only generate a CUE sheet) cannot be used with -I (only print info)!"
        );
    }

    #[test]
    fn parses_outputs_help_mode() {
        let cfg = parse_from_iter(["cyanrip-rs", "-o", "help"]).unwrap();
        assert_eq!(cfg.action, CliAction::ShowOutputsHelp);
        assert_eq!(cfg.settings.outputs, vec![OutputFormat::Flac]);
    }

    #[test]
    fn outputs_help_short_circuits_late_validations_like_c() {
        let cfg = parse_from_iter(["cyanrip-rs", "-o", "help", "-I", "-J"]).unwrap();
        assert_eq!(cfg.action, CliAction::ShowOutputsHelp);
    }

    #[test]
    fn verify_log_short_circuits_other_parsing_like_c() {
        let cfg = parse_from_iter(["cyanrip-rs", "-Y", "rip.log", "-P", "bogus"]).unwrap();
        assert_eq!(cfg.action, CliAction::VerifyLog);
        assert_eq!(cfg.settings.verify_log.as_deref(), Some("rip.log"));
    }

    #[test]
    fn paranoia_non_numeric_matches_c_strtol_behavior() {
        let cfg = parse_from_iter(["cyanrip-rs", "-P", "bogus"]).unwrap();
        assert_eq!(cfg.settings.paranoia_level, 0);
    }

    #[test]
    fn golden_c_style_full_rip_invocation() {
        let cfg = parse_from_iter([
            "cyanrip-rs",
            "-d",
            "/dev/cdrom",
            "-s",
            "6",
            "-r",
            "10",
            "-S",
            "4",
            "-o",
            "flac,mp3",
            "-b",
            "320",
            "-D",
            "{album} [{format}]",
            "-F",
            "{track} - {title}",
            "-p",
            "1=drop",
            "-l",
            "1,2,3",
            "-R",
            "2",
            "-c",
            "1/2",
            "-Q",
        ])
        .unwrap();

        assert_eq!(cfg.action, CliAction::Run);
        assert_eq!(cfg.settings.dev_path.as_deref(), Some("/dev/cdrom"));
        assert_eq!(cfg.settings.offset, 6);
        assert_eq!(cfg.settings.speed, 4);
        assert_eq!(cfg.settings.outputs, vec![OutputFormat::Flac, OutputFormat::Mp3]);
        assert_eq!(cfg.settings.bitrate_kbps, 320.0);
        assert_eq!(cfg.settings.pregap_action[0], PregapAction::Drop);
        assert_eq!(cfg.settings.rip_indices, vec![1, 2, 3]);
        assert_eq!(cfg.settings.release, Some(ReleaseSelection::Index(2)));
        assert_eq!(cfg.settings.discnumber, 1);
        assert_eq!(cfg.settings.totaldiscs, 2);
        assert!(cfg.settings.eject_on_success_rip);
    }

    #[test]
    fn golden_c_style_metadata_invocation() {
        let cfg = parse_from_iter([
            "cyanrip-rs",
            "-a",
            "album=Test:album_artist=Tester",
            "-t",
            "1=title=Intro:artist=Tester",
            "-t",
            "2=title=Outro",
            "-C",
            "Front=front.jpg",
            "-C",
            "2=track2.jpg",
        ])
        .unwrap();

        assert_eq!(cfg.settings.album_metadata.as_deref(), Some("album=Test:album_artist=Tester"));
        assert_eq!(cfg.settings.track_metadata.len(), 2);
        assert_eq!(cfg.settings.cover_specs.len(), 2);
    }

    #[test]
    fn cue_only_applies_c_side_effects() {
        let cfg = parse_from_iter(["cyanrip-rs", "-J", "-s", "0"]).unwrap();
        assert!(cfg.settings.generate_cue_only);
        assert!(cfg.settings.disable_accurip);
        assert!(cfg.settings.disable_coverart_db);
    }

    #[test]
    fn cue_only_without_offset_keeps_offset_unset_marker() {
        let cfg = parse_from_iter(["cyanrip-rs", "-J"]).unwrap();
        assert!(cfg.settings.generate_cue_only);
        assert!(!cfg.settings.offset_is_set);
    }

    #[test]
    fn info_only_disables_eject_side_effect() {
        let cfg = parse_from_iter(["cyanrip-rs", "-I", "-Q"]).unwrap();
        assert!(cfg.settings.print_info_only);
        assert!(!cfg.settings.eject_on_success_rip);
    }

    #[test]
    fn find_offset_applies_c_side_effects() {
        let cfg = parse_from_iter(["cyanrip-rs", "-f", "-A", "-Q", "-s", "42"]).unwrap();
        assert!(cfg.settings.find_drive_offset);
        assert!(!cfg.settings.disable_accurip);
        assert!(cfg.settings.disable_mb);
        assert!(cfg.settings.disable_coverart_db);
        assert_eq!(cfg.settings.offset, 0);
        assert_eq!(cfg.settings.over_under_read_frames, 0);
        assert!(!cfg.settings.eject_on_success_rip);
    }

    #[test]
    fn rejects_invalid_pregap() {
        let err = parse_from_iter(["cyanrip-rs", "-p", "0=drop"]).unwrap_err();
        assert_eq!(err, "Invalid track idx for pregap: 0");
    }

    #[test]
    fn rejects_invalid_cover_size_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-m", "999"]).unwrap_err();
        assert_eq!(
            err,
            "Invalid max coverart size 999 (must be 250, 500, 1200 or -1)"
        );
    }

    #[test]
    fn rejects_invalid_sanitize_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-T", "bad"]).unwrap_err();
        assert_eq!(err, "Invalid sanitation method bad");
    }

    #[test]
    fn rejects_invalid_release_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-R", "0"]).unwrap_err();
        assert_eq!(err, "Invalid release index 0!");
    }

    #[test]
    fn rejects_invalid_disc_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-c", "3/2"]).unwrap_err();
        assert_eq!(err, "discnumber 3 is larger than totaldiscs 2");
    }

    #[test]
    fn rejects_duplicate_outputs_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-o", "flac,flac"]).unwrap_err();
        assert_eq!(err, "Duplicated format \"flac\"");
    }

    #[test]
    fn rejects_unknown_output_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-o", "wat"]).unwrap_err();
        assert_eq!(err, "Invalid format \"wat\"");
    }

    #[test]
    fn rejects_invalid_paranoia_range_with_exact_message() {
        let err = parse_from_iter(["cyanrip-rs", "-P", "4"]).unwrap_err();
        assert_eq!(err, "Invalid paranoia level 4 must be between 0 and 3");
    }

    #[test]
    fn rejects_folder_scheme_without_format_for_multi_output() {
        let err = parse_from_iter(["cyanrip-rs", "-o", "flac,mp3", "-D", "{album}"]).unwrap_err();
        assert_eq!(
            err,
            "Directory name scheme must contain {format} with multiple output formats!"
        );
    }

    #[test]
    fn matches_upstream_short_option_surface() {
        let cmd = CliArgs::command();
        let got: BTreeSet<char> = cmd
            .get_arguments()
            .filter_map(|a| a.get_short())
            .collect();

        let expected: BTreeSet<char> = [
            'd', 's', 'r', 'Z', 'S', 'p', 'P', 'O', 'H', 'E', 'W', 'K', 'o', 'b', 'D', 'F',
            'L', 'M', 'l', 'T', 'I', 'J', 'a', 't', 'R', 'c', 'C', 'N', 'A', 'U', 'm', 'G',
            'Q', 'f', 'Y',
        ]
        .into_iter()
        .collect();

        assert_eq!(got, expected);
    }

    #[test]
    fn help_contains_c_style_sections() {
        let help = render_help();
        assert!(help.contains("Ripping options"));
        assert!(help.contains("Output options"));
        assert!(help.contains("Metadata options"));
        assert!(help.contains("Misc. options"));
    }

    #[test]
    fn help_contains_c_style_descriptions() {
        let help = render_help();
        assert!(help.contains("Track pregap handling: N=default|drop|merge|track (repeatable)"));
        assert!(help.contains("Only generate and print a CUE sheet, don't rip"));
        assert!(help.contains("Verify a rip log's FUN512 checksum"));
    }
}
