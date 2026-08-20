use super::paranoia::RetryPolicy;
use super::reader::{
    CDDA_FRAME_BYTES, CddaFrameReader, CddaReadError, ParanoiaHeuristicConfig,
    ParanoiaTrackRunResult, run_track_with_paranoia_heuristics_interruptible,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveTrackTocEntry {
    pub number: u8,
    pub start_lsn: i32,
    pub end_lsn: i32,
    pub track_is_data: bool,
    /// LSN of the first frame of the pregap, None if no pregap or unavailable.
    pub pregap_lsn: Option<i32>,
}

/// Hardware identification strings returned by the drive.
#[derive(Debug, Clone)]
pub struct DriveHwInfo {
    pub vendor: String,
    pub model: String,
    pub revision: String,
}

pub trait LinuxDriveBackend {
    type Handle: Copy;

    fn open(&mut self, device_path: Option<&str>) -> Result<Self::Handle, CddaReadError>;
    fn destroy(&mut self, handle: Self::Handle);
    fn read_audio_sector(
        &mut self,
        handle: Self::Handle,
        lsn: i32,
        out: &mut [u8; CDDA_FRAME_BYTES],
    ) -> Result<(), CddaReadError>;
    fn get_media_changed_code(&self, handle: Self::Handle) -> i32;
    fn media_changed_unsupported_code(&self) -> i32;
}

pub fn media_changed_from_code(code: i32, unsupported_code: i32) -> bool {
    code != 0 && code != unsupported_code
}

pub struct LinuxPhysicalDriveReader<B: LinuxDriveBackend> {
    backend: B,
    handle: B::Handle,
    next_lsn: i32,
}

impl<B: LinuxDriveBackend> LinuxPhysicalDriveReader<B> {
    pub fn new(mut backend: B, device_path: Option<&str>) -> Result<Self, CddaReadError> {
        let handle = backend.open(device_path)?;
        Ok(Self {
            backend,
            handle,
            next_lsn: 0,
        })
    }
}

impl<B: LinuxDriveBackend> Drop for LinuxPhysicalDriveReader<B> {
    fn drop(&mut self) {
        self.backend.destroy(self.handle);
    }
}

impl<B: LinuxDriveBackend> CddaFrameReader for LinuxPhysicalDriveReader<B> {
    fn seek_frame(&mut self, lsn: i32) -> Result<(), CddaReadError> {
        if lsn < 0 {
            return Err(CddaReadError::SeekFailed("negative lsn".to_string()));
        }
        self.next_lsn = lsn;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, CddaReadError> {
        let mut frame = [0u8; CDDA_FRAME_BYTES];
        self.backend
            .read_audio_sector(self.handle, self.next_lsn, &mut frame)?;
        self.next_lsn = self.next_lsn.saturating_add(1);
        Ok(frame.to_vec())
    }

    fn media_changed(&self) -> bool {
        let code = self.backend.get_media_changed_code(self.handle);
        media_changed_from_code(code, self.backend.media_changed_unsupported_code())
    }
}

#[derive(Default)]
pub struct UnsupportedLinuxDriveBackend;

impl LinuxDriveBackend for UnsupportedLinuxDriveBackend {
    type Handle = ();

    fn open(&mut self, _device_path: Option<&str>) -> Result<Self::Handle, CddaReadError> {
        Err(CddaReadError::ReadFailed(
            "linux physical-drive backend is unavailable; enable backend-libcdio-sys or backend-libcdio-rs"
                .to_string(),
        ))
    }

    fn destroy(&mut self, _handle: Self::Handle) {}

    fn read_audio_sector(
        &mut self,
        _handle: Self::Handle,
        _lsn: i32,
        _out: &mut [u8; CDDA_FRAME_BYTES],
    ) -> Result<(), CddaReadError> {
        Err(CddaReadError::ReadFailed(
            "linux physical-drive backend is unavailable".to_string(),
        ))
    }

    fn get_media_changed_code(&self, _handle: Self::Handle) -> i32 {
        0
    }

    fn media_changed_unsupported_code(&self) -> i32 {
        -1
    }
}

#[cfg(feature = "backend-libcdio-sys")]
#[derive(Default)]
pub struct LibcdioSysBackend;

#[cfg(feature = "backend-libcdio-sys")]
impl LinuxDriveBackend for LibcdioSysBackend {
    type Handle = *mut libcdio_sys::CdIo_t;

    fn open(&mut self, device_path: Option<&str>) -> Result<Self::Handle, CddaReadError> {
        let ptr = if let Some(path) = device_path {
            let c_path = std::ffi::CString::new(path).map_err(|_| {
                CddaReadError::ReadFailed("device path contains interior NUL byte".to_string())
            })?;
            unsafe { libcdio_sys::cdio_open(c_path.as_ptr(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
        } else {
            unsafe { libcdio_sys::cdio_open(std::ptr::null(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
        };

        if ptr.is_null() {
            return Err(CddaReadError::ReadFailed(
                "failed to open cd drive via libcdio".to_string(),
            ));
        }

        Ok(ptr)
    }

    fn destroy(&mut self, handle: Self::Handle) {
        if !handle.is_null() {
            unsafe { libcdio_sys::cdio_destroy(handle) };
        }
    }

    fn read_audio_sector(
        &mut self,
        handle: Self::Handle,
        lsn: i32,
        out: &mut [u8; CDDA_FRAME_BYTES],
    ) -> Result<(), CddaReadError> {
        let rc = unsafe {
            libcdio_sys::cdio_read_audio_sector(
                handle,
                out.as_mut_ptr().cast::<std::ffi::c_void>(),
                lsn as libcdio_sys::lsn_t,
            )
        };

        if (rc as i32) != 0 {
            return Err(CddaReadError::ReadFailed(format!(
                "libcdio read error at lsn {lsn}: rc={}",
                rc as i32
            )));
        }

        Ok(())
    }

    fn get_media_changed_code(&self, handle: Self::Handle) -> i32 {
        unsafe { libcdio_sys::cdio_get_media_changed(handle) as i32 }
    }

    fn media_changed_unsupported_code(&self) -> i32 {
        libcdio_sys::driver_return_code_t_DRIVER_OP_UNSUPPORTED as i32
    }
}

#[cfg(feature = "backend-libcdio-sys")]
pub type DefaultLinuxDriveBackend = LibcdioSysBackend;

#[cfg(not(feature = "backend-libcdio-sys"))]
pub type DefaultLinuxDriveBackend = UnsupportedLinuxDriveBackend;

pub fn open_linux_physical_drive(
    device_path: Option<&str>,
) -> Result<LinuxPhysicalDriveReader<DefaultLinuxDriveBackend>, CddaReadError> {
    LinuxPhysicalDriveReader::new(DefaultLinuxDriveBackend::default(), device_path)
}

#[cfg(feature = "backend-libcdio-sys")]
pub fn read_drive_toc_tracks(device_path: Option<&str>) -> Result<Vec<DriveTrackTocEntry>, CddaReadError> {
    let ptr = if let Some(path) = device_path {
        let c_path = std::ffi::CString::new(path).map_err(|_| {
            CddaReadError::ReadFailed("device path contains interior NUL byte".to_string())
        })?;
        unsafe { libcdio_sys::cdio_open(c_path.as_ptr(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
    } else {
        unsafe { libcdio_sys::cdio_open(std::ptr::null(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
    };

    if ptr.is_null() {
        return Err(CddaReadError::ReadFailed(
            "failed to open cd drive via libcdio".to_string(),
        ));
    }

    let mut out = Vec::new();
    let result = (|| {
        let first = unsafe { libcdio_sys::cdio_get_first_track_num(ptr) } as i32;
        let count = unsafe { libcdio_sys::cdio_get_num_tracks(ptr) } as i32;
        let leadout = unsafe {
            libcdio_sys::cdio_get_track_lsn(
                ptr,
                libcdio_sys::cdio_track_enums_CDIO_CDROM_LEADOUT_TRACK as u8,
            )
        } as i32;
        if first <= 0 || count <= 0 || leadout <= 0 || leadout == libcdio_sys::CDIO_INVALID_LSN as i32 {
            return Err(CddaReadError::ReadFailed(
                "invalid TOC values returned by drive".to_string(),
            ));
        }

        for i in 0..count {
            let track_number = (first + i) as u8;
            let start = unsafe { libcdio_sys::cdio_get_track_lsn(ptr, track_number) } as i32;
            if start == libcdio_sys::CDIO_INVALID_LSN as i32 || start < 0 {
                return Err(CddaReadError::ReadFailed(format!(
                    "invalid start LSN for track {track_number}"
                )));
            }

            let next_start = if i + 1 < count {
                let next = unsafe { libcdio_sys::cdio_get_track_lsn(ptr, (track_number + 1) as u8) } as i32;
                if next == libcdio_sys::CDIO_INVALID_LSN as i32 || next <= start {
                    return Err(CddaReadError::ReadFailed(format!(
                        "invalid next-track LSN for track {track_number}"
                    )));
                }
                next
            } else {
                if leadout <= start {
                    return Err(CddaReadError::ReadFailed(format!(
                        "invalid leadout LSN for track {track_number}"
                    )));
                }
                leadout
            };

            let format = unsafe { libcdio_sys::cdio_get_track_format(ptr, track_number) };
            let track_is_data = match format {
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_AUDIO => false,
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_CDI => true,
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_XA => true,
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_DATA => true,
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_PSX => true,
                x if x == libcdio_sys::track_format_t_TRACK_FORMAT_ERROR => {
                    unsafe { libcdio_sys::cdio_get_track_green(ptr, track_number) }
                }
                _ => false,
            };

            let raw_pregap = unsafe { libcdio_sys::cdio_get_track_pregap_lsn(ptr, track_number) } as i32;
            let pregap_lsn = if raw_pregap == libcdio_sys::CDIO_INVALID_LSN as i32 || raw_pregap == start {
                None
            } else {
                Some(raw_pregap)
            };

            out.push(DriveTrackTocEntry {
                number: track_number,
                start_lsn: start,
                end_lsn: next_start.saturating_sub(1),
                track_is_data,
                pregap_lsn,
            });
        }

        Ok(())
    })();

    unsafe { libcdio_sys::cdio_destroy(ptr) };

    result.map(|_| out)
}

#[cfg(not(feature = "backend-libcdio-sys"))]
pub fn read_drive_toc_tracks(_device_path: Option<&str>) -> Result<Vec<DriveTrackTocEntry>, CddaReadError> {
    Err(CddaReadError::ReadFailed(
        "drive TOC access requires backend-libcdio-sys".to_string(),
    ))
}

#[cfg(feature = "backend-libcdio-sys")]
pub fn read_drive_hwinfo(device_path: Option<&str>) -> Option<DriveHwInfo> {
    let ptr = if let Some(path) = device_path {
        let c_path = std::ffi::CString::new(path).ok()?;
        unsafe { libcdio_sys::cdio_open(c_path.as_ptr(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
    } else {
        unsafe { libcdio_sys::cdio_open(std::ptr::null(), libcdio_sys::driver_id_t_DRIVER_UNKNOWN) }
    };
    if ptr.is_null() {
        return None;
    }
    let mut hwinfo = libcdio_sys::cdio_hwinfo_t {
        psz_vendor: [0; 9],
        psz_model: [0; 17],
        psz_revision: [0; 5],
    };
    let ok = unsafe { libcdio_sys::cdio_get_hwinfo(ptr, &mut hwinfo) };
    unsafe { libcdio_sys::cdio_destroy(ptr) };
    if !ok {
        return None;
    }
    let to_str = |arr: &[i8]| -> String {
        let bytes: Vec<u8> = arr.iter().take_while(|&&b| b != 0).map(|&b| b as u8).collect();
        String::from_utf8_lossy(&bytes).trim().to_string()
    };
    Some(DriveHwInfo {
        vendor: to_str(&hwinfo.psz_vendor),
        model: to_str(&hwinfo.psz_model),
        revision: to_str(&hwinfo.psz_revision),
    })
}

#[cfg(not(feature = "backend-libcdio-sys"))]
pub fn read_drive_hwinfo(_device_path: Option<&str>) -> Option<DriveHwInfo> {
    None
}

pub fn run_paranoia_on_linux_drive_with_backend<B, F>(
    backend: B,
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    B: LinuxDriveBackend,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_paranoia_on_linux_drive_with_backend_heuristics(
        backend,
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        ParanoiaHeuristicConfig::default(),
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive_with_backend_heuristics<B, F>(
    backend: B,
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    heuristics: ParanoiaHeuristicConfig,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    B: LinuxDriveBackend,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_paranoia_on_linux_drive_with_backend_heuristics_interruptible(
        backend,
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        heuristics,
        || false,
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive_with_backend_heuristics_interruptible<B, F, I>(
    backend: B,
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    heuristics: ParanoiaHeuristicConfig,
    should_interrupt: I,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    B: LinuxDriveBackend,
    F: FnMut(u32, &[Vec<u8>]) -> u32,
    I: FnMut() -> bool,
{
    let mut reader = LinuxPhysicalDriveReader::new(backend, device_path)?;
    run_track_with_paranoia_heuristics_interruptible(
        &mut reader,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        heuristics,
        should_interrupt,
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive<F>(
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_paranoia_on_linux_drive_heuristics(
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        ParanoiaHeuristicConfig::default(),
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive_heuristics<F>(
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    heuristics: ParanoiaHeuristicConfig,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    run_paranoia_on_linux_drive_with_backend_heuristics_interruptible(
        DefaultLinuxDriveBackend::default(),
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        heuristics,
        || false,
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive_interruptible<F, I>(
    device_path: Option<&str>,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    should_interrupt: I,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    F: FnMut(u32, &[Vec<u8>]) -> u32,
    I: FnMut() -> bool,
{
    run_paranoia_on_linux_drive_with_backend_heuristics_interruptible(
        DefaultLinuxDriveBackend::default(),
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        ParanoiaHeuristicConfig::default(),
        should_interrupt,
        checksum_fn,
    )
}

pub fn run_paranoia_on_linux_drive_with_defaults_for_level<F>(
    device_path: Option<&str>,
    paranoia_level: i32,
    start_lsn: i32,
    frame_count: usize,
    max_frame_retries: u32,
    retry_policy: &mut RetryPolicy,
    checksum_fn: F,
) -> Result<ParanoiaTrackRunResult, CddaReadError>
where
    F: FnMut(u32, &[Vec<u8>]) -> u32,
{
    let heuristics = heuristics_for_paranoia_level(paranoia_level);

    run_paranoia_on_linux_drive_heuristics(
        device_path,
        start_lsn,
        frame_count,
        max_frame_retries,
        retry_policy,
        heuristics,
        checksum_fn,
    )
}

pub fn heuristics_for_paranoia_level(paranoia_level: i32) -> ParanoiaHeuristicConfig {
    if paranoia_level >= 2 {
        ParanoiaHeuristicConfig {
            overlap_frames: 1,
            verify_overlap: true,
        }
    } else {
        ParanoiaHeuristicConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdda::paranoia::{RipEvent, RipState};
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;

    #[derive(Debug, Default)]
    struct MockState {
        reads: Vec<i32>,
        read_calls: usize,
        destroyed: bool,
    }

    #[derive(Clone)]
    struct MockBackend {
        state: Rc<std::cell::RefCell<MockState>>,
        frames: HashMap<i32, [u8; CDDA_FRAME_BYTES]>,
        fail_lsns: HashSet<i32>,
        fail_on_read_calls: HashSet<usize>,
        media_changed_code: i32,
        unsupported_code: i32,
    }

    impl MockBackend {
        fn new() -> (Self, Rc<std::cell::RefCell<MockState>>) {
            let state = Rc::new(std::cell::RefCell::new(MockState::default()));
            (
                Self {
                    state: state.clone(),
                    frames: HashMap::new(),
                    fail_lsns: HashSet::new(),
                    fail_on_read_calls: HashSet::new(),
                    media_changed_code: 0,
                    unsupported_code: -2,
                },
                state,
            )
        }
    }

    impl LinuxDriveBackend for MockBackend {
        type Handle = usize;

        fn open(&mut self, _device_path: Option<&str>) -> Result<Self::Handle, CddaReadError> {
            Ok(1)
        }

        fn destroy(&mut self, _handle: Self::Handle) {
            self.state.borrow_mut().destroyed = true;
        }

        fn read_audio_sector(
            &mut self,
            _handle: Self::Handle,
            lsn: i32,
            out: &mut [u8; CDDA_FRAME_BYTES],
        ) -> Result<(), CddaReadError> {
            let mut state = self.state.borrow_mut();
            state.reads.push(lsn);
            state.read_calls = state.read_calls.saturating_add(1);
            let call = state.read_calls;
            drop(state);

            if self.fail_on_read_calls.contains(&call) {
                return Err(CddaReadError::ReadFailed(format!(
                    "mock transient read fail at call {call}"
                )));
            }
            if self.fail_lsns.contains(&lsn) {
                return Err(CddaReadError::ReadFailed(format!("mock read fail at {lsn}")));
            }
            let frame = self
                .frames
                .get(&lsn)
                .ok_or_else(|| CddaReadError::ReadFailed(format!("missing frame for {lsn}")))?;
            out.copy_from_slice(frame);
            Ok(())
        }

        fn get_media_changed_code(&self, _handle: Self::Handle) -> i32 {
            self.media_changed_code
        }

        fn media_changed_unsupported_code(&self) -> i32 {
            self.unsupported_code
        }
    }

    #[test]
    fn media_changed_mapping_matches_upstream_rule() {
        assert!(!media_changed_from_code(0, -2));
        assert!(!media_changed_from_code(-2, -2));
        assert!(media_changed_from_code(1, -2));
    }

    #[test]
    fn reader_reads_by_seek_position_and_advances_lsn() {
        let (mut backend, _state) = MockBackend::new();
        backend.frames.insert(100, [7u8; CDDA_FRAME_BYTES]);
        backend.frames.insert(101, [9u8; CDDA_FRAME_BYTES]);

        let mut reader = LinuxPhysicalDriveReader::new(backend, Some("/dev/cdrom"))
            .expect("mock open should work");
        reader.seek_frame(100).expect("seek should work");

        let a = reader.read_frame().expect("read 100");
        let b = reader.read_frame().expect("read 101");

        assert_eq!(a[0], 7);
        assert_eq!(b[0], 9);
    }

    #[test]
    fn reader_propagates_read_error() {
        let (mut backend, _state) = MockBackend::new();
        backend.frames.insert(10, [1u8; CDDA_FRAME_BYTES]);
        backend.fail_lsns.insert(10);

        let mut reader = LinuxPhysicalDriveReader::new(backend, Some("/dev/cdrom"))
            .expect("mock open should work");
        reader.seek_frame(10).expect("seek should work");

        let err = reader.read_frame().expect_err("read should fail");
        assert!(matches!(err, CddaReadError::ReadFailed(_)));
    }

    #[test]
    fn reader_uses_media_changed_codes_from_backend() {
        let (mut backend, _state) = MockBackend::new();
        backend.media_changed_code = 5;

        let reader =
            LinuxPhysicalDriveReader::new(backend, Some("/dev/cdrom")).expect("mock open");
        assert!(reader.media_changed());
    }

    #[test]
    fn reader_drop_calls_destroy() {
        let (backend, state) = MockBackend::new();
        {
            let _reader =
                LinuxPhysicalDriveReader::new(backend, Some("/dev/cdrom")).expect("mock open");
        }
        assert!(state.borrow().destroyed);
    }

    #[test]
    fn paranoia_runner_wires_retries_and_flush_transitions() {
        let (mut backend, _state) = MockBackend::new();
        backend.frames.insert(0, [1u8; CDDA_FRAME_BYTES]);
        backend.frames.insert(1, [2u8; CDDA_FRAME_BYTES]);
        backend.frames.insert(2, [3u8; CDDA_FRAME_BYTES]);
        backend.fail_on_read_calls.insert(1);

        let mut policy = RetryPolicy::disabled();
        let out = run_paranoia_on_linux_drive_with_backend(
            backend,
            Some("/dev/cdrom"),
            0,
            3,
            1,
            &mut policy,
            |_pass, _frames| 0xABCD_1234,
        )
        .expect("retry path should succeed");

        assert_eq!(out.state, RipState::TrackComplete);
        assert_eq!(out.passes, 1);
        assert!(out.events.contains(&RipEvent::FrameReadError));
        assert!(out.events.contains(&RipEvent::FlushEncoders));
        assert!(out.events.contains(&RipEvent::EncoderFlushDone));
    }

    #[test]
    fn paranoia_runner_aborts_on_media_change() {
        let (mut backend, _state) = MockBackend::new();
        backend.media_changed_code = 1;

        let mut policy = RetryPolicy::disabled();
        let out = run_paranoia_on_linux_drive_with_backend(
            backend,
            Some("/dev/cdrom"),
            0,
            1,
            0,
            &mut policy,
            |_pass, _frames| 0,
        )
        .expect("media-change path should return aborted result");

        assert_eq!(out.state, RipState::Aborted);
        assert!(out.events.contains(&RipEvent::MediaChanged));
    }

    #[test]
    fn level_based_defaults_enable_verify_overlap_for_higher_levels() {
        assert_eq!(heuristics_for_paranoia_level(0), ParanoiaHeuristicConfig::default());
        assert_eq!(
            heuristics_for_paranoia_level(1),
            ParanoiaHeuristicConfig::default()
        );
        assert_eq!(
            heuristics_for_paranoia_level(2),
            ParanoiaHeuristicConfig {
                overlap_frames: 1,
                verify_overlap: true,
            }
        );

        let (mut backend, _state) = MockBackend::new();
        backend.frames.insert(0, [1u8; CDDA_FRAME_BYTES]);

        let mut retry = RetryPolicy::new(1, 2);
        let out = run_paranoia_on_linux_drive_with_backend_heuristics(
            backend,
            Some("/dev/cdrom"),
            0,
            1,
            0,
            &mut retry,
            heuristics_for_paranoia_level(2),
            |_pass, _| 0x1234,
        )
        .expect("run should complete");
        assert!(
            out
                .callback_counters
                .get(crate::cdda::reader::ParanoiaCallbackKind::Verify)
                >= 1
        );
    }

    #[test]
    fn paranoia_runner_aborts_on_interrupt_request() {
        let (mut backend, _state) = MockBackend::new();
        backend.frames.insert(0, [1u8; CDDA_FRAME_BYTES]);
        backend.frames.insert(1, [2u8; CDDA_FRAME_BYTES]);

        let mut checks = 0usize;
        let mut policy = RetryPolicy::disabled();
        let out = run_paranoia_on_linux_drive_with_backend_heuristics_interruptible(
            backend,
            Some("/dev/cdrom"),
            0,
            2,
            0,
            &mut policy,
            ParanoiaHeuristicConfig::default(),
            || {
                checks = checks.saturating_add(1);
                checks >= 2
            },
            |_pass, _| 0,
        )
        .expect("interrupt should abort run");

        assert_eq!(out.state, RipState::Aborted);
        assert!(out.events.contains(&RipEvent::QuitRequested));
    }

    #[cfg(not(feature = "backend-libcdio-sys"))]
    #[test]
    fn unsupported_backend_returns_actionable_error() {
        let err = match open_linux_physical_drive(Some("/dev/cdrom")) {
            Ok(_) => panic!("expected backend-unavailable error"),
            Err(err) => err,
        };
        let msg = match err {
            CddaReadError::ReadFailed(msg) => msg,
            CddaReadError::SeekFailed(msg) => msg,
        };
        assert!(msg.contains("backend-libcdio-sys") || msg.contains("backend-libcdio-rs"));
    }
}
