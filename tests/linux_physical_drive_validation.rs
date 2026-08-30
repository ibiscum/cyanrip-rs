#![cfg(all(target_os = "linux", feature = "backend-libcdio-sys"))]

use cyanrip_rs::cdda::linux_drive::{
    open_linux_physical_drive, read_drive_hwinfo, read_drive_toc_tracks, run_paranoia_on_linux_drive,
    run_paranoia_on_linux_drive_interruptible,
};
use cyanrip_rs::cdda::paranoia::{RetryPolicy, RipEvent, RipState};
use cyanrip_rs::cdda::reader::CddaFrameReader;
use libcdio_sys::{
    CDIO_INVALID_LSN, CdIo_t, cdio_destroy, cdio_get_first_track_num, cdio_get_num_tracks,
    cdio_get_track_lsn, cdio_open, cdio_track_enums_CDIO_CDROM_LEADOUT_TRACK,
    driver_id_t_DRIVER_UNKNOWN,
};
use std::ffi::CString;

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn reads_audio_cd_toc_from_real_drive() {
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let c_device = CString::new(device.clone()).expect("device path must not contain NUL bytes");

    let drive: *mut CdIo_t =
        unsafe { cdio_open(c_device.as_ptr(), driver_id_t_DRIVER_UNKNOWN) };
    assert!(
        !drive.is_null(),
        "failed to open {device}; ensure an audio CD is inserted beforehand"
    );

    let first_track = unsafe { cdio_get_first_track_num(drive) } as i32;
    let num_tracks = unsafe { cdio_get_num_tracks(drive) } as i32;
    assert!(
        num_tracks > 0,
        "no tracks found in TOC for {device}; ensure an audio CD is inserted beforehand"
    );

    for track in first_track..(first_track + num_tracks) {
        let lsn = unsafe { cdio_get_track_lsn(drive, track as u8) };
        assert_ne!(
            lsn, CDIO_INVALID_LSN,
            "invalid LSN in TOC for track {track} on {device}"
        );
    }

    let leadout_lsn = unsafe {
        cdio_get_track_lsn(drive, cdio_track_enums_CDIO_CDROM_LEADOUT_TRACK as u8)
    };
    assert_ne!(
        leadout_lsn, CDIO_INVALID_LSN,
        "invalid TOC leadout LSN on {device}"
    );

    unsafe { cdio_destroy(drive) };
}

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn reads_toc_entries_via_runtime_helper_and_iterates_tracks() {
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());

    let tracks = read_drive_toc_tracks(Some(&device)).unwrap_or_else(|err| {
        panic!("failed to read TOC from {device}: {err:?}");
    });

    assert!(
        !tracks.is_empty(),
        "no TOC tracks found on {device}; ensure an audio CD is inserted beforehand"
    );

    let mut previous_end = -1i32;
    for t in tracks {
        assert!(t.number > 0, "invalid track number in TOC helper output");
        assert!(
            t.start_lsn >= 0 && t.end_lsn >= t.start_lsn,
            "invalid LSN range for track {} on {}: {}..{}",
            t.number,
            device,
            t.start_lsn,
            t.end_lsn
        );
        assert!(
            t.start_lsn > previous_end,
            "non-monotonic TOC ranges at track {} on {}",
            t.number,
            device
        );
        previous_end = t.end_lsn;
    }
}

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn reads_one_audio_frame_from_real_drive() {
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());

    let mut reader = open_linux_physical_drive(Some(&device)).unwrap_or_else(|err| {
        panic!("failed to open drive at {device}: {err:?}");
    });

    reader.seek_frame(0).expect("seek to first frame should work");
    let frame = reader
        .read_frame()
        .expect("first frame read should work on a readable audio disc");

    assert_eq!(frame.len(), 2352);
}

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn runs_paranoia_pipeline_on_real_drive() {
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let mut policy = RetryPolicy::disabled();

    let out = run_paranoia_on_linux_drive(
        Some(&device),
        0,
        1,
        1,
        &mut policy,
        |_pass, frames| {
            let mut acc = 0u32;
            for frame in frames {
                for b in frame {
                    acc = acc.wrapping_add(*b as u32);
                }
            }
            acc
        },
    )
    .expect("paranoia run should complete on readable media");

    match out.state {
        RipState::TrackComplete => {
            assert!(out.events.contains(&RipEvent::FrameReadOk));
            assert!(out.events.contains(&RipEvent::FlushEncoders));
            assert!(out.events.contains(&RipEvent::EncoderFlushDone));
        }
        RipState::Aborted => {
            assert!(out.events.contains(&RipEvent::MediaChanged));
        }
        other => panic!("unexpected paranoia run state on real drive: {other:?}"),
    }
}

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn interruption_request_aborts_paranoia_pipeline_on_real_drive() {
    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let mut policy = RetryPolicy::disabled();
    let mut checks = 0usize;

    let out = run_paranoia_on_linux_drive_interruptible(
        Some(&device),
        0,
        16,
        1,
        &mut policy,
        || {
            checks = checks.saturating_add(1);
            checks >= 2
        },
        |_pass, _frames| 0,
    )
    .expect("interruptible paranoia run should return an aborted state");

    assert_eq!(out.state, RipState::Aborted);
    assert!(out.events.contains(&RipEvent::QuitRequested));
}

#[test]
#[ignore = "manual scenario: insert audio CD beforehand, then swap/eject media during read"]
fn manual_media_change_scenario_reference() {
    let enabled = std::env::var("CYANRIP_RUN_MANUAL_MEDIA_CHANGE").ok();
    if enabled.as_deref() != Some("1") {
        return;
    }

    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    eprintln!("manual media-change scenario started on {device}");
    eprintln!("ACTION REQUIRED: eject or swap media now while this test is reading.");
    eprintln!("The test waits up to ~30 seconds for media-changed detection.");

    let mut reader = open_linux_physical_drive(Some(&device))
        .expect("failed to open drive for manual media-change scenario");
    reader
        .seek_frame(0)
        .expect("seek to first frame should work before media change");

    let max_iterations = 120usize;
    for i in 0..max_iterations {
        if reader.media_changed() {
            eprintln!("media-changed detected at iteration {i}");
            return;
        }

        let _ = reader.read_frame();

        if reader.media_changed() {
            eprintln!("media-changed detected after read at iteration {i}");
            return;
        }

        if i % 10 == 0 {
            eprintln!(
                "waiting for manual media change... {}s elapsed",
                (i / 4)
            );
        }

        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    panic!(
        "manual media-change scenario did not detect media change within timeout; ensure media was swapped/ejected during test"
    );
}

#[test]
#[ignore = "requires a real optical drive and an audio CD inserted beforehand"]
fn info_mode_report_contains_toc_section_with_track_details() {
    use cyanrip_rs::{OutputFormat, Settings};
    use cyanrip_rs::app::run_workflow;

    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());

    let settings = Settings {
        dev_path: Some(device.clone()),
        print_info_only: true,
        disable_accurip: true,
        disable_mb: true,
        disable_coverart_db: true,
        outputs: vec![OutputFormat::Flac],
        ..Settings::default()
    };

    let out = run_workflow(&settings)
        .expect("info-only run_workflow should succeed")
        .expect("info-only run_workflow should produce a report");

    assert!(
        out.contains("Drive used:"),
        "report should contain drive info; output:\n{out}"
    );
    assert!(
        out.contains("Disc tracks:"),
        "report should contain disc tracks line; output:\n{out}"
    );
    assert!(
        out.contains("Track 1 info:"),
        "report should contain at least Track 1 info; output:\n{out}"
    );
    assert!(
        out.contains("Start LSN:"),
        "report should contain Start LSN line; output:\n{out}"
    );
    assert!(
        out.contains("MusicBrainz URL:"),
        "report should contain MusicBrainz submission URL; output:\n{out}"
    );
    assert!(
        out.contains("DiscID:"),
        "report should contain DiscID line; output:\n{out}"
    );
    assert!(
        out.contains("CDDB ID:"),
        "report should contain CDDB ID line; output:\n{out}"
    );
    assert!(
        out.contains("Total time:"),
        "report should contain total time; output:\n{out}"
    );

    // Drive hwinfo
    let hw = read_drive_hwinfo(Some(&device));
    assert!(
        hw.is_some(),
        "read_drive_hwinfo should succeed for an open drive; device: {device}"
    );
    if let Some(hw) = hw {
        assert!(
            out.contains(&hw.model),
            "drive model '{}' should appear in Drive used: line; output:\n{out}",
            hw.model
        );
    }
}

#[test]
#[ignore = "requires a real optical drive, network, and the multi-release fixture disc inserted beforehand"]
fn info_mode_requires_release_selection_when_musicbrainz_returns_multiple_releases() {
    use cyanrip_rs::app::run_workflow;
    use cyanrip_rs::{OutputFormat, Settings};

    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let expected_discid = std::env::var("CYANRIP_EXPECT_MULTI_RELEASE_DISCID")
        .unwrap_or_else(|_| "BKkzOxbdODYWFIOEEZ3b.b_nm64-".to_string());

    let settings = Settings {
        dev_path: Some(device.clone()),
        print_info_only: true,
        disable_mb: false,
        disable_accurip: true,
        disable_coverart_db: true,
        outputs: vec![OutputFormat::Flac],
        ..Settings::default()
    };

    let err = run_workflow(&settings).expect_err(
        "-I without -R should fail when MusicBrainz returns multiple releases for this disc",
    );
    let msg = err.to_string();

    assert!(
        msg.contains("Multiple releases found in database for DiscID"),
        "expected multi-release prompt; output:\n{msg}"
    );
    assert!(
        msg.contains(&expected_discid),
        "expected DiscID {expected_discid} in prompt; output:\n{msg}"
    );
    assert!(
        msg.contains("Please specify which release to use by adding the -R argument"),
        "expected release-selection guidance; output:\n{msg}"
    );
}

#[test]
#[ignore = "requires a real optical drive, network, and the multi-release fixture disc inserted beforehand"]
fn full_rip_requires_release_selection_when_musicbrainz_returns_multiple_releases() {
    use cyanrip_rs::app::run_workflow;
    use cyanrip_rs::{OutputFormat, Settings};

    let device = std::env::var("CYANRIP_CDROM_DEVICE").unwrap_or_else(|_| "/dev/cdrom".to_string());
    let expected_discid = std::env::var("CYANRIP_EXPECT_MULTI_RELEASE_DISCID")
        .unwrap_or_else(|_| "BKkzOxbdODYWFIOEEZ3b.b_nm64-".to_string());
    let output_root = std::env::temp_dir().join("cyanrip-rs-multi-release-abort-test");

    // Default (non -I, non -J) action must also abort with the disambiguation
    // prompt instead of silently ripping without a chosen release.
    let settings = Settings {
        dev_path: Some(device.clone()),
        disable_mb: false,
        disable_accurip: true,
        disable_coverart_db: true,
        output_root: Some(output_root.to_string_lossy().to_string()),
        outputs: vec![OutputFormat::Flac],
        ..Settings::default()
    };

    let err = run_workflow(&settings).expect_err(
        "full rip without -R should fail when MusicBrainz returns multiple releases for this disc",
    );
    let msg = err.to_string();

    assert!(
        msg.contains("Multiple releases found in database for DiscID"),
        "expected multi-release prompt; output:\n{msg}"
    );
    assert!(
        msg.contains(&expected_discid),
        "expected DiscID {expected_discid} in prompt; output:\n{msg}"
    );
    assert!(
        msg.contains("Please specify which release to use by adding the -R argument"),
        "expected release-selection guidance; output:\n{msg}"
    );
    assert!(
        !output_root.exists() || std::fs::read_dir(&output_root).map(|mut d| d.next().is_none()).unwrap_or(true),
        "no rip output should have been written before release disambiguation; found files under {}",
        output_root.display()
    );
}
