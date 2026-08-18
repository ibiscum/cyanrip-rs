pub mod paranoia;
pub mod reader;

#[cfg(all(target_os = "linux", feature = "cdda"))]
pub mod linux_drive;
