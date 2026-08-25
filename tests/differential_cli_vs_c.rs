use std::path::PathBuf;
use std::process::Command;

fn resolve_c_binary() -> PathBuf {
    if let Ok(path) = std::env::var("CYANRIP_C_BIN") {
        return PathBuf::from(path);
    }
    PathBuf::from("/home/ulf/data/cyanrip/build/src/cyanrip")
}

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

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
#[ignore = "requires compiled C reference binary and compares only CLI-first-slice semantics"]
fn differential_cli_first_slice_against_c_binary() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));
    let c_bin = resolve_c_binary();

    assert!(
        c_bin.exists(),
        "missing C binary at {} (set CYANRIP_C_BIN)",
        c_bin.display()
    );

    let cases: [(&str, &[&str], bool, &[&str]); 11] = [
        (
            "help",
            &["--help"],
            true,
            &["Ripping options", "Output options", "Metadata options"],
        ),
        (
            "outputs_help",
            &["-o", "help"],
            true,
            &["Supported output codecs", "flac", "wav", "pcm"],
        ),
        (
            "invalid_paranoia",
            &["-P", "4"],
            false,
            &["Invalid paranoia level 4 must be between 0 and 3"],
        ),
        (
            "mode_conflict",
            &["-I", "-J"],
            false,
            &["-J (only generate a CUE sheet) cannot be used with -I"],
        ),
        (
            "find_offset_info_conflict",
            &["-f", "-I"],
            false,
            &["-f (find drive offset) cannot be used with -I"],
        ),
        (
            "find_offset_cue_only_conflict",
            &["-f", "-J"],
            false,
            &["-f (find drive offset) cannot be used with -J"],
        ),
        (
            "verify_log_valid",
            &["-Y", "tests/fixtures/log/valid.log"],
            true,
            &["checksum valid"],
        ),
        (
            "verify_log_mismatch",
            &["-Y", "tests/fixtures/log/mismatch.log"],
            false,
            &["checksum mismatch", "file has been modified"],
        ),
        (
            "verify_log_no_checksum",
            &["-Y", "tests/fixtures/log/no_checksum.log"],
            false,
            &["No FUN512 checksum found"],
        ),
        (
            "verify_log_trailing_data",
            &["-Y", "tests/fixtures/log/trailing.log"],
            false,
            &["has data after the checksum", "file has been modified"],
        ),
        (
            "verify_log_io_error",
            &["-Y", "tests/fixtures/log/does_not_exist.log"],
            false,
            &["Couldn't read"],
        ),
    ];

    for (name, args, expect_success, needles) in cases {
        let (rust_code, rust_out) = run_capture(&rust_bin, args);
        let (c_code, c_out) = run_capture(&c_bin, args);

        if expect_success {
            assert_eq!(
                rust_code, 0,
                "rust case {name} failed with {rust_code}; output:\n{}",
                normalize(&rust_out)
            );
            assert_eq!(
                c_code, 0,
                "c case {name} failed with {c_code}; output:\n{}",
                normalize(&c_out)
            );
        } else {
            assert_ne!(
                rust_code, 0,
                "rust case {name} expected failure; output:\n{}",
                normalize(&rust_out)
            );
            assert_ne!(
                c_code, 0,
                "c case {name} expected failure; output:\n{}",
                normalize(&c_out)
            );
        }

        let rust_n = normalize(&rust_out).to_lowercase();
        let c_n = normalize(&c_out).to_lowercase();

        for needle in needles {
            let n = needle.to_lowercase();
            assert!(
                rust_n.contains(&n),
                "rust case {name} missing substring {needle}; output:\n{}",
                normalize(&rust_out)
            );
            assert!(
                c_n.contains(&n),
                "c case {name} missing substring {needle}; output:\n{}",
                normalize(&c_out)
            );
        }
    }
}
