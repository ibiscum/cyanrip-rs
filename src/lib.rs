pub const MAX_TRACKS: usize = 198;
pub const MAX_OUTPUTS: usize = 32;
pub const MAX_PARANOIA_LEVEL: i32 = 3;
const CD_FRAMESIZE_RAW_DIV4: i32 = 588;

pub mod app;
pub mod cli;
pub mod cue;
pub mod fun512;
pub mod log_report;
pub mod metadata;
pub mod naming;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    BinCue,
    Cue,
    Nrg,
    CdrDao,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Flac,
    Mp3,
    Tta,
    Opus,
    Aac,
    AacMp4,
    Wavpack,
    Vorbis,
    Alac,
    AlacMp4,
    Wav,
    OpusMp4,
    Pcm,
}

impl OutputFormat {
    pub fn from_cli_name(name: &str) -> Option<Self> {
        match name {
            "flac" => Some(Self::Flac),
            "mp3" => Some(Self::Mp3),
            "tta" => Some(Self::Tta),
            "opus" => Some(Self::Opus),
            "aac" => Some(Self::Aac),
            "aac_mp4" => Some(Self::AacMp4),
            "wavpack" => Some(Self::Wavpack),
            "vorbis" => Some(Self::Vorbis),
            "alac" => Some(Self::Alac),
            "alac_mp4" => Some(Self::AlacMp4),
            "wav" => Some(Self::Wav),
            "opus_mp4" => Some(Self::OpusMp4),
            "pcm" => Some(Self::Pcm),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeMethod {
    Simple,
    OsSimple,
    Unicode,
    OsUnicode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PregapAction {
    Default,
    Drop,
    Merge,
    Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverArtLookupSize {
    Original,
    Px250,
    Px500,
    Px1200,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseSelection {
    Index(i32),
    Id(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub dev_path: Option<String>,
    pub folder_name_scheme: String,
    pub track_name_scheme: String,
    pub log_name_scheme: String,
    pub cue_name_scheme: String,
    pub sanitize_method: SanitizeMethod,
    pub speed: i32,
    pub max_retries: i32,
    pub over_under_read_frames: i32,
    pub offset: i32,
    pub ripping_retries: i32,
    pub print_info_only: bool,
    pub disable_mb: bool,
    pub disable_coverart_db: bool,
    pub coverart_lookup_size: CoverArtLookupSize,
    pub decode_hdcd: bool,
    pub deemphasis: bool,
    pub force_deemphasis: bool,
    pub bitrate_kbps: f32,
    pub overread_leadinout: bool,
    pub rip_indices_count: i32,
    pub disable_accurip: bool,
    pub eject_on_success_rip: bool,
    pub outputs: Vec<OutputFormat>,
    pub disable_coverart_embedding: bool,
    pub enable_replaygain: bool,
    pub paranoia_level: i32,
    pub pregap_action: [PregapAction; MAX_TRACKS],
    pub rip_indices: Vec<i32>,
    pub release: Option<ReleaseSelection>,
    pub discnumber: i32,
    pub totaldiscs: i32,
    pub generate_cue_only: bool,
    pub find_drive_offset: bool,
    pub verify_log: Option<String>,
    pub album_metadata: Option<String>,
    pub track_metadata: Vec<String>,
    pub cover_specs: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dev_path: None,
            folder_name_scheme:
                "{album}{if #releasecomment# > #0# (|releasecomment|)} [{format}]".to_string(),
            track_name_scheme: "{if #totaldiscs# > #1#|disc|.}{track} - {title}".to_string(),
            log_name_scheme: "{album}{if #totaldiscs# > #1# CD|disc|}".to_string(),
            cue_name_scheme: "{album}{if #totaldiscs# > #1# CD|disc|}".to_string(),
            sanitize_method: SanitizeMethod::Unicode,
            speed: 0,
            max_retries: 10,
            over_under_read_frames: 0,
            offset: 0,
            ripping_retries: 0,
            print_info_only: false,
            disable_mb: false,
            disable_coverart_db: false,
            coverart_lookup_size: CoverArtLookupSize::Original,
            decode_hdcd: false,
            deemphasis: true,
            force_deemphasis: false,
            bitrate_kbps: 256.0,
            overread_leadinout: false,
            rip_indices_count: -1,
            disable_accurip: false,
            eject_on_success_rip: false,
            outputs: vec![OutputFormat::Flac],
            disable_coverart_embedding: false,
            enable_replaygain: true,
            paranoia_level: 3,
            pregap_action: [PregapAction::Default; MAX_TRACKS],
            rip_indices: Vec::new(),
            release: None,
            discnumber: 0,
            totaldiscs: 0,
            generate_cue_only: false,
            find_drive_offset: false,
            verify_log: None,
            album_metadata: None,
            track_metadata: Vec::new(),
            cover_specs: Vec::new(),
        }
    }
}

pub fn ends_with(input: &str, suffix: &str) -> bool {
    input.ends_with(suffix)
}

pub fn open_dev_kind(dev_path: &str) -> DriverKind {
    if ends_with(dev_path, ".bin") {
        DriverKind::BinCue
    } else if ends_with(dev_path, ".cue") {
        DriverKind::Cue
    } else if ends_with(dev_path, ".nrg") {
        DriverKind::Nrg
    } else if ends_with(dev_path, ".toc") {
        DriverKind::CdrDao
    } else {
        DriverKind::Unknown
    }
}

pub fn calc_over_under_read_frames(offset: i32) -> i32 {
    if offset == 0 {
        return 0;
    }

    let sign = if offset < 0 { -1 } else { 1 };
    let frames = (offset.abs() + CD_FRAMESIZE_RAW_DIV4 - 1) / CD_FRAMESIZE_RAW_DIV4;
    sign * frames
}

pub fn parse_paranoia(input: &str, max_level: i32) -> Result<i32, String> {
    let level = if input == "none" {
        0
    } else if input == "max" {
        max_level
    } else {
        c_style_strtol_i32(input)
    };

    if (0..=max_level).contains(&level) {
        Ok(level)
    } else {
        Err(format!(
            "Invalid paranoia level {level} must be between 0 and {max_level}"
        ))
    }
}

fn c_style_strtol_i32(input: &str) -> i32 {
    let s = input.trim_start();
    let mut chars = s.chars().peekable();

    let mut sign = 1i64;
    if let Some(&ch) = chars.peek() {
        if ch == '-' {
            sign = -1;
            chars.next();
        } else if ch == '+' {
            chars.next();
        }
    }

    let mut had_digit = false;
    let mut value: i64 = 0;
    while let Some(&ch) = chars.peek() {
        if !ch.is_ascii_digit() {
            break;
        }
        had_digit = true;
        value = value
            .saturating_mul(10)
            .saturating_add((ch as i64) - ('0' as i64));
        chars.next();
    }

    if !had_digit {
        return 0;
    }

    let signed = value.saturating_mul(sign);
    if signed > i32::MAX as i64 {
        i32::MAX
    } else if signed < i32::MIN as i64 {
        i32::MIN
    } else {
        signed as i32
    }
}

pub fn parse_cover_size(size: i32) -> Result<CoverArtLookupSize, String> {
    match size {
        -1 => Ok(CoverArtLookupSize::Original),
        250 => Ok(CoverArtLookupSize::Px250),
        500 => Ok(CoverArtLookupSize::Px500),
        1200 => Ok(CoverArtLookupSize::Px1200),
        _ => Err(format!(
            "Invalid max coverart size {size} (must be 250, 500, 1200 or -1)"
        )),
    }
}

pub fn parse_sanitize(input: &str) -> Result<SanitizeMethod, String> {
    match input {
        "simple" => Ok(SanitizeMethod::Simple),
        "os_simple" => Ok(SanitizeMethod::OsSimple),
        "unicode" => Ok(SanitizeMethod::Unicode),
        "os_unicode" => Ok(SanitizeMethod::OsUnicode),
        _ => Err(format!("Invalid sanitation method {input}")),
    }
}

pub fn parse_disc(input: &str) -> Result<(i32, i32), String> {
    let mut parts = input.split('/');

    let disc_number = parts
        .next()
        .ok_or_else(|| "Invalid discnumber 0".to_string())?
        .parse::<i32>()
        .map_err(|_| "Invalid discnumber 0".to_string())?;

    if disc_number <= 0 {
        return Err(format!("Invalid discnumber {disc_number}"));
    }

    let total_discs = if let Some(total) = parts.next() {
        let parsed_total = total
            .parse::<i32>()
            .map_err(|_| "Invalid totaldiscs 0".to_string())?;
        if parsed_total <= 0 {
            return Err(format!("Invalid totaldiscs {parsed_total}"));
        }
        if disc_number > parsed_total {
            return Err(format!(
                "discnumber {disc_number} is larger than totaldiscs {parsed_total}"
            ));
        }
        parsed_total
    } else {
        0
    };

    Ok((disc_number, total_discs))
}

pub fn parse_release(input: &str) -> Result<ReleaseSelection, String> {
    if let Ok(index) = input.parse::<i32>() {
        if index > 0 {
            return Ok(ReleaseSelection::Index(index));
        }
        return Err(format!("Invalid release index {index}!"));
    }

    Ok(ReleaseSelection::Id(input.to_string()))
}

pub fn parse_outputs(entries: &[&str]) -> Result<Vec<OutputFormat>, String> {
    if entries.is_empty() {
        return Ok(vec![OutputFormat::Flac]);
    }

    if entries[0] == "help" {
        return Ok(Vec::new());
    }

    let mut outputs = Vec::new();
    for entry in entries.iter().take(MAX_OUTPUTS) {
        let fmt = OutputFormat::from_cli_name(entry)
            .ok_or_else(|| format!("Invalid format \"{entry}\""))?;
        if outputs.contains(&fmt) {
            return Err(format!("Duplicated format \"{entry}\""));
        }
        outputs.push(fmt);
    }

    Ok(outputs)
}

pub fn parse_track_indices(indices: &[i32]) -> Result<Vec<i32>, String> {
    let mut out = Vec::new();
    for idx in indices.iter().copied() {
        if out.contains(&idx) {
            return Err(format!("Duplicated rip idx {idx}"));
        }
        out.push(idx);
    }
    out.sort_unstable();
    Ok(out)
}

pub fn parse_pregap_entry(entry: &str) -> Result<(usize, PregapAction), String> {
    let (idx_raw, action_raw) = entry
        .split_once('=')
        .ok_or_else(|| "Missing pregap action".to_string())?;

    let idx = idx_raw
        .parse::<usize>()
        .map_err(|_| "Invalid track idx for pregap: 0".to_string())?;

    if !(1..=197).contains(&idx) {
        return Err(format!("Invalid track idx for pregap: {idx}"));
    }

    let action = match action_raw {
        x if x.starts_with("default") => PregapAction::Default,
        x if x.starts_with("drop") => PregapAction::Drop,
        x if x.starts_with("merge") => PregapAction::Merge,
        x if x.starts_with("track") => PregapAction::Track,
        _ => return Err(format!("Invalid pregap action {action_raw}")),
    };

    Ok((idx - 1, action))
}

pub fn apply_pregap_entries(
    pregap: &mut [PregapAction; MAX_TRACKS],
    entries: &[&str],
) -> Result<(), String> {
    for entry in entries {
        let (idx, action) = parse_pregap_entry(entry)?;
        pregap[idx] = action;
    }
    Ok(())
}

pub fn validate_folder_scheme(outputs_num: usize, folder_name_scheme: &str) -> Result<(), String> {
    if outputs_num > 1 && !folder_name_scheme.contains("{format}") {
        return Err(
            "Directory name scheme must contain {format} with multiple output formats!".to_string(),
        );
    }
    Ok(())
}

pub fn validate_mode_combo(print_info_only: bool, generate_cue_only: bool) -> Result<(), String> {
    if print_info_only && generate_cue_only {
        return Err("-J (only generate a CUE sheet) cannot be used with -I (only print info)!"
            .to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_c_main() {
        let s = Settings::default();
        assert_eq!(s.max_retries, 10);
        assert_eq!(s.offset, 0);
        assert_eq!(s.outputs, vec![OutputFormat::Flac]);
        assert_eq!(s.paranoia_level, 3);
        assert_eq!(s.pregap_action[0], PregapAction::Default);
        assert!(s.enable_replaygain);
        assert!(s.deemphasis);
    }

    #[test]
    fn open_dev_suffix_mapping_regression() {
        assert_eq!(open_dev_kind("disc.bin"), DriverKind::BinCue);
        assert_eq!(open_dev_kind("disc.cue"), DriverKind::Cue);
        assert_eq!(open_dev_kind("disc.nrg"), DriverKind::Nrg);
        assert_eq!(open_dev_kind("disc.toc"), DriverKind::CdrDao);
        assert_eq!(open_dev_kind("/dev/cdrom"), DriverKind::Unknown);
    }

    #[test]
    fn over_under_read_frames_regression() {
        assert_eq!(calc_over_under_read_frames(0), 0);
        assert_eq!(calc_over_under_read_frames(1), 1);
        assert_eq!(calc_over_under_read_frames(588), 1);
        assert_eq!(calc_over_under_read_frames(589), 2);
        assert_eq!(calc_over_under_read_frames(-588), -1);
        assert_eq!(calc_over_under_read_frames(-589), -2);
    }

    #[test]
    fn paranoia_parsing_regression() {
        assert_eq!(parse_paranoia("none", 3).unwrap(), 0);
        assert_eq!(parse_paranoia("max", 3).unwrap(), 3);
        assert_eq!(parse_paranoia("2", 3).unwrap(), 2);
        assert_eq!(parse_paranoia("bogus", 3).unwrap(), 0);
        assert_eq!(parse_paranoia("2abc", 3).unwrap(), 2);
        assert!(parse_paranoia("4", 3).is_err());
    }

    #[test]
    fn cover_size_regression() {
        assert_eq!(parse_cover_size(-1).unwrap(), CoverArtLookupSize::Original);
        assert_eq!(parse_cover_size(250).unwrap(), CoverArtLookupSize::Px250);
        assert_eq!(parse_cover_size(500).unwrap(), CoverArtLookupSize::Px500);
        assert_eq!(parse_cover_size(1200).unwrap(), CoverArtLookupSize::Px1200);
        assert!(parse_cover_size(999).is_err());
    }

    #[test]
    fn sanitize_regression() {
        assert_eq!(parse_sanitize("simple").unwrap(), SanitizeMethod::Simple);
        assert_eq!(parse_sanitize("os_simple").unwrap(), SanitizeMethod::OsSimple);
        assert_eq!(parse_sanitize("unicode").unwrap(), SanitizeMethod::Unicode);
        assert_eq!(parse_sanitize("os_unicode").unwrap(), SanitizeMethod::OsUnicode);
        assert!(parse_sanitize("bad").is_err());
    }

    #[test]
    fn disc_parse_regression() {
        assert_eq!(parse_disc("1").unwrap(), (1, 0));
        assert_eq!(parse_disc("2/3").unwrap(), (2, 3));
        assert!(parse_disc("0/3").is_err());
        assert!(parse_disc("3/2").is_err());
        assert!(parse_disc("1/0").is_err());
    }

    #[test]
    fn release_parse_regression() {
        assert_eq!(parse_release("2").unwrap(), ReleaseSelection::Index(2));
        assert_eq!(
            parse_release("1f2a3b4c").unwrap(),
            ReleaseSelection::Id("1f2a3b4c".to_string())
        );
        assert!(parse_release("0").is_err());
    }

    #[test]
    fn outputs_validation_regression() {
        assert_eq!(parse_outputs(&[]).unwrap(), vec![OutputFormat::Flac]);
        assert_eq!(parse_outputs(&["help"]).unwrap(), Vec::<OutputFormat>::new());
        assert_eq!(
            parse_outputs(&["flac", "mp3"]).unwrap(),
            vec![OutputFormat::Flac, OutputFormat::Mp3]
        );
        assert!(parse_outputs(&["flac", "flac"]).is_err());
        assert!(parse_outputs(&["wat"]).is_err());
    }

    #[test]
    fn track_indices_are_sorted_and_unique() {
        assert_eq!(parse_track_indices(&[5, 1, 3]).unwrap(), vec![1, 3, 5]);
        assert!(parse_track_indices(&[1, 2, 2]).is_err());
    }

    #[test]
    fn pregap_parsing_regression() {
        assert_eq!(parse_pregap_entry("1=default").unwrap(), (0, PregapAction::Default));
        assert_eq!(parse_pregap_entry("2=drop").unwrap(), (1, PregapAction::Drop));
        assert_eq!(parse_pregap_entry("3=merge").unwrap(), (2, PregapAction::Merge));
        assert_eq!(parse_pregap_entry("4=track").unwrap(), (3, PregapAction::Track));
        assert!(parse_pregap_entry("0=drop").is_err());
        assert!(parse_pregap_entry("1=invalid").is_err());
        assert!(parse_pregap_entry("1").is_err());
    }

    #[test]
    fn pregap_application_regression() {
        let mut pregap = [PregapAction::Default; MAX_TRACKS];
        apply_pregap_entries(&mut pregap, &["1=drop", "3=merge"]).unwrap();
        assert_eq!(pregap[0], PregapAction::Drop);
        assert_eq!(pregap[2], PregapAction::Merge);
        assert_eq!(pregap[1], PregapAction::Default);
    }

    #[test]
    fn folder_scheme_validation_regression() {
        assert!(validate_folder_scheme(1, "{album}").is_ok());
        assert!(validate_folder_scheme(2, "{album}-{format}").is_ok());
        assert!(validate_folder_scheme(2, "{album}").is_err());
    }

    #[test]
    fn mode_combo_validation_regression() {
        assert!(validate_mode_combo(false, false).is_ok());
        assert!(validate_mode_combo(true, false).is_ok());
        assert!(validate_mode_combo(false, true).is_ok());
        assert!(validate_mode_combo(true, true).is_err());
    }
}