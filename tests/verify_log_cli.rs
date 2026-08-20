use std::path::PathBuf;
use std::process::Command;

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

#[test]
fn verify_log_cli_reports_expected_status_and_messages() {
    let rust_bin = PathBuf::from(env!("CARGO_BIN_EXE_cyanrip-rs"));

    let cases: [(&str, &[&str], i32, &[&str]); 5] = [
        (
            "valid",
            &["-Y", "tests/fixtures/log/valid.log"],
            0,
            &["checksum valid"],
        ),
        (
            "mismatch",
            &["-Y", "tests/fixtures/log/mismatch.log"],
            1,
            &["checksum mismatch", "file has been modified"],
        ),
        (
            "no_checksum",
            &["-Y", "tests/fixtures/log/no_checksum.log"],
            1,
            &["No FUN512 checksum found"],
        ),
        (
            "trailing_data",
            &["-Y", "tests/fixtures/log/trailing.log"],
            1,
            &["has data after the checksum", "file has been modified"],
        ),
        (
            "io_error",
            &["-Y", "tests/fixtures/log/does_not_exist.log"],
            1,
            &["Couldn't read"],
        ),
    ];

    for (name, args, expected_code, needles) in cases {
        let (code, out) = run_capture(&rust_bin, args);
        assert_eq!(
            code, expected_code,
            "case {name} exit code mismatch; output:\n{out}"
        );

        let out_l = out.to_lowercase();
        for needle in needles {
            assert!(
                out_l.contains(&needle.to_lowercase()),
                "case {name} missing substring {needle}; output:\n{out}"
            );
        }
    }
}
