use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run_capture(binary: &PathBuf, args: &[&str]) -> (i32, String) {
    let out = Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", binary.display()));

    let status = out.status.code().unwrap_or(-1);
    let mut merged = String::new();
    merged.push_str(&String::from_utf8_lossy(&out.stdout));
    merged.push_str(&String::from_utf8_lossy(&out.stderr));
    (status, merged)
}

fn run_capture_with_env(binary: &PathBuf, args: &[&str], envs: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(binary);
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", binary.display()));

    let status = out.status.code().unwrap_or(-1);
    let mut merged = String::new();
    merged.push_str(&String::from_utf8_lossy(&out.stdout));
    merged.push_str(&String::from_utf8_lossy(&out.stderr));
    (status, merged)
}

fn unique_temp_output_root() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cyanrip-rs-synth-cli-it-{now}"))
}

fn unique_temp_cue_path() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cyanrip-rs-run-workflow-{now}.cue"))
}

#[test]
fn run_mode_defaults_to_image_reader_full_rip_bridge() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &["-o", "flac"],
        &[("CYANRIP_RS_OUTPUT_ROOT", &output_root_s)],
    );

    assert_eq!(code, 0);
    assert!(out.contains("cyanrip-rs full-rip bridge mode"));
    assert!(out.contains("Source: image"));
    assert!(out.contains("Written files: 1"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 1, "expected one written file, output: {out}");
    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "full-rip bridge output root should be removable");
}

#[test]
fn run_mode_full_rip_bridge_writes_selected_tracks() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &["-o", "wav,flac", "-l", "1,3", "-t", "1=title=One", "-t", "3=title=Three"],
        &[("CYANRIP_RS_OUTPUT_ROOT", &output_root_s)],
    );

    assert_eq!(code, 0);
    assert!(out.contains("cyanrip-rs full-rip bridge mode"));
    assert!(out.contains("Source: image"));
    assert!(out.contains("Written files: 4"));
    assert!(out.contains("TRACK 1 START_LSN 0 FRAMES 32"));
    assert!(out.contains("TRACK 3 START_LSN 64 FRAMES 32"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 4, "expected four written files, output: {out}");
    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "multi-track full-rip bridge output root should be removable");
}

#[test]
fn run_mode_full_rip_bridge_honors_track_boundary_metadata() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &[
            "-o",
            "wav",
            "-l",
            "2,4",
            "-t",
            "2=title=Two:start_lsn=20:frames=10",
            "-t",
            "4=title=Four:start_lsn=100:end_lsn=115",
        ],
        &[("CYANRIP_RS_OUTPUT_ROOT", &output_root_s)],
    );

    assert_eq!(code, 0);
    assert!(out.contains("TRACK 2 START_LSN 20 FRAMES 10"));
    assert!(out.contains("TRACK 4 START_LSN 100 FRAMES 16"));
    assert!(out.contains("Written files: 2"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 2, "expected two written files, output: {out}");
    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "boundary-metadata full-rip bridge output root should be removable");
}

#[test]
fn run_mode_full_rip_bridge_honors_image_toc_env_overrides() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &[
            "-o",
            "wav",
            "-l",
            "2,4",
            "-t",
            "2=title=Two:start_lsn=20:frames=10",
            "-t",
            "4=title=Four:start_lsn=100:end_lsn=115",
        ],
        &[
            ("CYANRIP_RS_OUTPUT_ROOT", &output_root_s),
            ("CYANRIP_RS_IMAGE_TOC", "2:500-524,4:800-829"),
        ],
    );

    assert_eq!(code, 0);
    assert!(out.contains("TRACK 2 START_LSN 500 FRAMES 25"));
    assert!(out.contains("TRACK 4 START_LSN 800 FRAMES 30"));
    assert!(out.contains("Written files: 2"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 2, "expected two written files, output: {out}");
    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "image-toc full-rip bridge output root should be removable");
}

#[test]
fn run_mode_full_rip_bridge_honors_cue_toc_boundaries() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();
    let cue_path = unique_temp_cue_path();

    fs::write(
        &cue_path,
        "FILE \"disc.bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 00:00:10\n",
    )
    .expect("cue fixture should be writable");

    let cue_path_s = cue_path.to_string_lossy().to_string();
    let (code, out) = run_capture_with_env(
        &rust_bin,
        &["-o", "wav", "-d", &cue_path_s, "-l", "1,2"],
        &[("CYANRIP_RS_OUTPUT_ROOT", &output_root_s)],
    );

    assert_eq!(code, 0);
    assert!(out.contains("Source: image"));
    assert!(out.contains("TRACK 1 START_LSN 0 FRAMES 10"));
    assert!(out.contains("TRACK 2 START_LSN 10 FRAMES 32"));
    assert!(out.contains("Written files: 2"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 2, "expected two written files, output: {out}");
    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let _ = fs::remove_file(&cue_path);
    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "cue-toc full-rip bridge output root should be removable");
}

#[test]
fn run_mode_rejects_unimplemented_codec_early() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));

    let (code, out) = run_capture(&rust_bin, &["-o", "mp3"]);
    assert_eq!(code, 1);
    assert!(out.contains("output format Mp3 is not yet implemented"));
}

#[test]
fn info_only_mode_returns_success_with_report() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));

    let (code, out) = run_capture(&rust_bin, &["-I", "-o", "flac"]);
    if code == 0 {
        assert!(out.contains("cyanrip-rs "));
        assert!(out.contains("Paranoia level: "));
        assert!(out.contains("Outputs:        "));
        assert!(out.contains("AccurateRip:    "));
    } else {
        assert_eq!(code, 1, "unexpected exit code for info-only mode: {code}, output: {out}");
        assert!(
            out.contains("TOC read failed") || out.contains("musicbrainz lookup failed"),
            "unexpected info-only error output: {out}"
        );
    }
}

#[test]
fn cue_only_mode_returns_success_with_cue_preview() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));

    let (code, out) = run_capture(
        &rust_bin,
        &[
            "-J",
            "-s",
            "0",
            "-a",
            "album=Example Album:album_artist=Example Artist",
            "-t",
            "1=title=Intro:artist=Example Artist",
            "-t",
            "2=title=Outro:artist=Example Artist",
            "-o",
            "flac",
        ],
    );
    if code == 0 {
        assert!(
            out.contains("cyanrip-rs cue-only preview") || out.contains("cyanrip-rs "),
            "unexpected cue-only output: {out}"
        );
        assert!(out.contains("TRACK 01 AUDIO"));
        assert!(
            out.contains("TITLE \"Example Album\"")
                || out.contains("REM DISCID")
                || out.contains("REM MUSICBRAINZ_DISCID"),
            "unexpected cue metadata output: {out}"
        );
    } else {
        assert_eq!(code, 1, "unexpected exit code for cue-only mode: {code}, output: {out}");
        assert!(
            out.contains("TOC read failed") || out.contains("musicbrainz lookup failed"),
            "unexpected cue-only error output: {out}"
        );
    }
}

#[test]
fn cue_only_mode_without_explicit_offset_matches_c_error() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let fixture = include_str!("fixtures/cli/cue_only_offset_unset_upstream.txt");
    let expected_line = fixture
        .lines()
        .find_map(|line| line.strip_prefix("Observed terminal line: "))
        .expect("fixture must include observed terminal line");

    let (code, out) = run_capture(&rust_bin, &["-J"]);
    assert_eq!(code, 0);
    assert!(out.contains(expected_line));
}

#[test]
fn find_offset_mode_returns_success_with_report() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));

    let (code, out) = run_capture(&rust_bin, &["-f", "-o", "flac"]);
    if code == 0 {
        assert!(out.contains("cyanrip-rs find-offset mode"));
        assert!(out.contains("AccurateRip: enabled"));
        assert!(out.contains("Status:"));
    } else {
        assert_eq!(code, 1, "unexpected exit code for find-offset mode: {code}, output: {out}");
        assert!(
            out.contains("TOC read failed")
                || out.contains("physical read failed")
                || out.contains("discid computation failed")
                || out.contains("accurip lookup failed"),
            "unexpected find-offset error output: {out}"
        );
    }
}

#[test]
fn synthetic_full_rip_mode_writes_real_output_files_when_enabled() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &[
            "-o",
            "wav,flac",
            "-a",
            "album=Synth Album:album_artist=Synth Artist",
            "-t",
            "1=title=Intro:artist=Synth Artist",
        ],
        &[
            ("CYANRIP_RS_ENABLE_SYNTHETIC_RIP", "1"),
            ("CYANRIP_RS_OUTPUT_ROOT", &output_root_s),
        ],
    );

    assert_eq!(code, 0);
    assert!(out.contains("cyanrip-rs synthetic full-rip mode"));
    assert!(out.contains("Written files: 2"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 2, "expected two written files, output: {out}");

    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "synthetic output root should be removable");
}

#[test]
fn synthetic_full_rip_mode_supports_image_reader_source() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let output_root = unique_temp_output_root();
    let output_root_s = output_root.to_string_lossy().to_string();

    let (code, out) = run_capture_with_env(
        &rust_bin,
        &["-o", "wav", "-a", "album=Reader Album:album_artist=Reader Artist"],
        &[
            ("CYANRIP_RS_ENABLE_SYNTHETIC_RIP", "1"),
            ("CYANRIP_RS_SYNTHETIC_SOURCE", "image-reader"),
            ("CYANRIP_RS_OUTPUT_ROOT", &output_root_s),
        ],
    );

    assert_eq!(code, 0);
    assert!(out.contains("cyanrip-rs synthetic full-rip mode"));
    assert!(out.contains("Written files: 1"));

    let file_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("FILE ")).collect();
    assert_eq!(file_lines.len(), 1, "expected one written file, output: {out}");

    for line in file_lines {
        let path = line.trim_start_matches("FILE ").trim();
        assert!(PathBuf::from(path).exists(), "expected output file to exist: {path}");
    }

    let cleanup = fs::remove_dir_all(&output_root);
    assert!(cleanup.is_ok(), "synthetic output root should be removable");
}

#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
fn assert_info_mode_release_selection_succeeds(
    release_index: &str,
    expected_release_id: &str,
    expected_album: &str,
    expected_disc_number: i32,
    expected_total_discs: i32,
) {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let expected_discid = std::env::var("CYANRIP_EXPECT_MULTI_RELEASE_DISCID")
        .unwrap_or_else(|_| "BKkzOxbdODYWFIOEEZ3b.b_nm64-".to_string());

    let (code, out) = run_capture(
        &rust_bin,
        &["-I", "-d", &device, "-R", release_index, "-o", "flac"],
    );

    assert_eq!(
        code, 0,
        "expected -I -R {release_index} to succeed on multi-release disc; output:\n{out}"
    );
    assert!(
        out.contains(&format!("DiscID:         {expected_discid}")),
        "expected DiscID line for {expected_discid}; output:\n{out}"
    );
    assert!(
        out.contains(&format!("Release ID:     {expected_release_id}")),
        "expected Release ID {expected_release_id}; output:\n{out}"
    );
    assert!(
        out.contains(&format!("Album:          {expected_album}")),
        "expected album '{expected_album}'; output:\n{out}"
    );
    assert!(
        out.contains(&format!("Disc number:    {expected_disc_number}")),
        "expected disc number {expected_disc_number}; output:\n{out}"
    );
    assert!(
        out.contains(&format!("Total discs:    {expected_total_discs}")),
        "expected total discs {expected_total_discs}; output:\n{out}"
    );
    assert!(
        out.contains("  Metadata:"),
        "expected per-track metadata block in info output; output:\n{out}"
    );
    assert!(
        out.contains("MusicBrainz URL:"),
        "expected info-only report output after disambiguation; output:\n{out}"
    );
    assert!(
        !out.contains("Multiple releases found in database for DiscID"),
        "did not expect multi-release prompt when -R is provided; output:\n{out}"
    );
}

#[test]
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
#[ignore = "requires a real optical drive, network access, and the captured multi-release disc inserted"]
fn info_only_mode_with_release_index_1_disambiguates_musicbrainz_result() {
    assert_info_mode_release_selection_succeeds(
        "1",
        "4c63d77d-6348-4ae1-9616-f25e625fa0d7",
        "Power Classics! Classical Music for Your Active Lifestyle, Volume 3",
        1,
        1,
    );
}

#[test]
#[cfg(all(target_os = "linux", feature = "cdda", feature = "backend-libcdio-sys"))]
#[ignore = "requires a real optical drive, network access, and the captured multi-release disc inserted"]
fn info_only_mode_with_release_index_2_disambiguates_musicbrainz_result() {
    assert_info_mode_release_selection_succeeds(
        "2",
        "1f504c20-5423-47fb-8d25-243ce749b92c",
        "Power Classics! Classical Music for your Active Lifestyle",
        3,
        10,
    );
}
