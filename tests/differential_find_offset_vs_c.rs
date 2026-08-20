#![cfg(all(target_os = "linux", feature = "backend-libcdio-sys"))]

use std::path::PathBuf;
use std::process::Command;

fn run_capture(bin: &PathBuf, args: &[&str]) -> (i32, String) {
    let out = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let status = out.status.code().unwrap_or(-1);
    let mut merged = String::new();
    merged.push_str(&String::from_utf8_lossy(&out.stdout));
    merged.push_str(&String::from_utf8_lossy(&out.stderr));
    (status, merged)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OffsetOutcome {
    Found,
    NoAr,
    TooShort,
    NotFound,
    Unknown,
}

fn classify_rust(out: &str) -> OffsetOutcome {
    if out.contains("Status: drive offset found:") {
        return OffsetOutcome::Found;
    }
    if out.contains("Status: no matching AccurateRip entry;")
        || out.contains("Status: no track had AccurateRip entry;")
    {
        return OffsetOutcome::NoAr;
    }
    if out.contains("Status: no track was long enough") {
        return OffsetOutcome::TooShort;
    }
    if out.contains("Status: unable to find drive offset") {
        return OffsetOutcome::NotFound;
    }
    OffsetOutcome::Unknown
}

fn classify_c(out: &str) -> OffsetOutcome {
    if out.contains("Drive offset of ") && out.contains(" found (confidence:") {
        return OffsetOutcome::Found;
    }
    if out.contains("No track had AccuRip entry") {
        return OffsetOutcome::NoAr;
    }
    if out.contains("No track was long enough") {
        return OffsetOutcome::TooShort;
    }
    if out.contains("Was not able to find drive offset") {
        return OffsetOutcome::NotFound;
    }
    OffsetOutcome::Unknown
}

#[test]
#[ignore = "requires real optical drive, inserted audio CD, and upstream C binary"]
fn differential_find_offset_against_c_binary() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let c_bin = std::env::var("CYANRIP_C_BIN")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/ulf/data/cyanrip/build/src/cyanrip"));
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());

    let (rust_code, rust_out) = run_capture(&rust_bin, &["-f", "-o", "flac", "-d", &device]);
    let (c_code, c_out) = run_capture(&c_bin, &["-f", "-o", "flac", "-d", &device]);

    assert_eq!(rust_code, 0, "rust find-offset should succeed\n{rust_out}");
    assert_eq!(c_code, 0, "c find-offset should succeed\n{c_out}");

    let rust_state = classify_rust(&rust_out);
    let c_state = classify_c(&c_out);

    assert_eq!(
        rust_state,
        c_state,
        "find-offset differential outcome mismatch\nRUST:\n{rust_out}\n\nC:\n{c_out}"
    );
}
