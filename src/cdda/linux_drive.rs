use super::reader::{CDDA_FRAME_BYTES, CddaFrameReader, CddaReadError};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;

    #[derive(Debug, Default)]
    struct MockState {
        reads: Vec<i32>,
        destroyed: bool,
    }

    #[derive(Clone)]
    struct MockBackend {
        state: Rc<std::cell::RefCell<MockState>>,
        frames: HashMap<i32, [u8; CDDA_FRAME_BYTES]>,
        fail_lsns: HashSet<i32>,
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
            self.state.borrow_mut().reads.push(lsn);
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
