use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use crc32fast::Hasher as Crc32Hasher;
use sha2::{Digest, Sha512};

pub const LOG_FUN512_MARKER: &str = "Log FUN512: ";
pub const FUN512_MAX_IDX: u8 = 16;

/// Computes the EAC CRC32 over raw bytes using the same CRC32 implementation
/// as [`ChecksumCtx`]. Intended for repeat-rip pass checksums, where only
/// consistency across passes is required.
pub fn eac_crc32_from_bytes(data: &[u8]) -> u32 {
    let mut hasher = Crc32Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogVerify {
    Valid,
    Mismatch,
    NoChecksum,
    TrailingData,
    IoError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumResult {
    pub eac_crc: u32,
    pub accurip_checksum_v1: u32,
    pub accurip_checksum_v1_450: u32,
    pub accurip_checksum_v2: u32,
}

#[derive(Debug, Clone)]
pub struct ChecksumCtx {
    crc32: Crc32Hasher,
    acu_start: u32,
    acu_end: u32,
    acu_mult: u32,
    acu_sum_1: u32,
    acu_sum_1_450: u32,
    acu_sum_2: u32,
}

impl ChecksumCtx {
    const CD_FRAME_BYTES: u32 = 2352;
    const SAMPLE_BYTES: u32 = 4;
    const SAMPLES_PER_FRAME: u32 = Self::CD_FRAME_BYTES / Self::SAMPLE_BYTES;

    pub fn new(nb_samples: u32, accurip_track_is_first: bool, accurip_track_is_last: bool) -> Self {
        let mut acu_start = 0u32;
        let mut acu_end = nb_samples;

        if accurip_track_is_first {
            acu_start = acu_start.wrapping_add(Self::SAMPLES_PER_FRAME * 5);
        }
        if accurip_track_is_last {
            acu_end = acu_end.wrapping_sub(Self::SAMPLES_PER_FRAME * 5);
        }

        Self {
            crc32: Crc32Hasher::new(),
            acu_start,
            acu_end,
            acu_mult: 1,
            acu_sum_1: 0,
            acu_sum_1_450: 0,
            acu_sum_2: 0,
        }
    }

    pub fn process_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        self.crc32.update(data);

        let sample_count = data.len() / 4;
        for j in 0..sample_count {
            let off = j * 4;
            let val = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);

            if self.acu_mult >= self.acu_start && self.acu_mult <= self.acu_end {
                let tmp = (val as u64).wrapping_mul(self.acu_mult as u64);
                let lo = (tmp & 0xFFFF_FFFF) as u32;
                let hi = (tmp >> 32) as u32;

                self.acu_sum_1 = self.acu_sum_1.wrapping_add(self.acu_mult.wrapping_mul(val));
                self.acu_sum_2 = self.acu_sum_2.wrapping_add(hi);
                self.acu_sum_2 = self.acu_sum_2.wrapping_add(lo);
            }

            let prev = self.acu_mult.wrapping_sub(1);
            let start = 450 * Self::SAMPLES_PER_FRAME;
            let end = 451 * Self::SAMPLES_PER_FRAME;
            if prev >= start && prev < end {
                let mult = self.acu_mult.wrapping_sub(start);
                self.acu_sum_1_450 = self.acu_sum_1_450.wrapping_add(val.wrapping_mul(mult));
            }

            self.acu_mult = self.acu_mult.wrapping_add(1);
        }
    }

    pub fn finalize(self) -> ChecksumResult {
        ChecksumResult {
            eac_crc: self.crc32.finalize(),
            accurip_checksum_v1: self.acu_sum_1,
            accurip_checksum_v1_450: self.acu_sum_1_450,
            accurip_checksum_v2: self.acu_sum_2,
        }
    }
}

pub fn fun512_from_sha512_digest(sha512_digest: [u8; 64], idx: u8) -> String {
    let mut digest = sha512_digest;

    for b in &mut digest {
        *b ^= 0x81u8.wrapping_add(idx);
    }

    for j in 0..64usize {
        for k in 0..64usize {
            if j != k {
                digest[j] ^= digest[k];
            }
        }
    }

    let mut out = STANDARD.encode(digest);
    while out.ends_with('=') {
        out.pop();
    }
    out.replace('/', "_").replace('+', ".")
}

pub fn fun512_from_bytes(data: &[u8], idx: u8) -> String {
    let mut hasher = Sha512::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let mut digest = [0u8; 64];
    digest.copy_from_slice(&hash);
    fun512_from_sha512_digest(digest, idx)
}

pub fn verify_log_bytes(data: &[u8]) -> LogVerify {
    if data.is_empty() {
        return LogVerify::IoError;
    }

    let marker = LOG_FUN512_MARKER.as_bytes();
    let mut pos = None;
    let mut i = 0usize;
    while i + marker.len() <= data.len() {
        if &data[i..i + marker.len()] == marker {
            pos = Some(i);
            i += marker.len();
            continue;
        }
        i += 1;
    }

    let Some(pos) = pos else {
        return LogVerify::NoChecksum;
    };

    let truth_start = pos + marker.len();
    let mut truth_end = truth_start;
    while truth_end < data.len() && data[truth_end] != b'\r' && data[truth_end] != b'\n' {
        truth_end += 1;
    }

    let mut tail = truth_end;
    while tail < data.len() && (data[tail] == b'\r' || data[tail] == b'\n') {
        tail += 1;
    }
    if tail != data.len() {
        return LogVerify::TrailingData;
    }

    let truth = String::from_utf8_lossy(&data[truth_start..truth_end]).to_string();
    let mut hasher = Sha512::new();
    hasher.update(&data[..pos]);
    let hash = hasher.finalize();
    let mut digest = [0u8; 64];
    digest.copy_from_slice(&hash);

    for idx in 0..FUN512_MAX_IDX {
        if fun512_from_sha512_digest(digest, idx) == truth {
            return LogVerify::Valid;
        }
    }

    LogVerify::Mismatch
}

pub fn verify_log_path(path: &Path) -> LogVerify {
    match fs::read(path) {
        Ok(bytes) => verify_log_bytes(&bytes),
        Err(_) => LogVerify::IoError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples_to_le_bytes(samples: &[u32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(samples.len() * 4);
        for &s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn reference_checksums(
        samples: &[u32],
        nb_samples: u32,
        first: bool,
        last: bool,
    ) -> ChecksumResult {
        let bytes = samples_to_le_bytes(samples);
        let mut crc = Crc32Hasher::new();
        crc.update(&bytes);

        let mut acu_start = 0u32;
        let mut acu_end = nb_samples;
        let mut acu_mult = 1u32;
        let mut acu_sum_1 = 0u32;
        let mut acu_sum_1_450 = 0u32;
        let mut acu_sum_2 = 0u32;

        if first {
            acu_start = acu_start.wrapping_add(5 * ChecksumCtx::SAMPLES_PER_FRAME);
        }
        if last {
            acu_end = acu_end.wrapping_sub(5 * ChecksumCtx::SAMPLES_PER_FRAME);
        }

        let start_450 = 450 * ChecksumCtx::SAMPLES_PER_FRAME;
        let end_450 = 451 * ChecksumCtx::SAMPLES_PER_FRAME;

        for &val in samples {
            if acu_mult >= acu_start && acu_mult <= acu_end {
                let tmp = (val as u64).wrapping_mul(acu_mult as u64);
                let lo = (tmp & 0xFFFF_FFFF) as u32;
                let hi = (tmp >> 32) as u32;
                acu_sum_1 = acu_sum_1.wrapping_add(acu_mult.wrapping_mul(val));
                acu_sum_2 = acu_sum_2.wrapping_add(hi);
                acu_sum_2 = acu_sum_2.wrapping_add(lo);
            }

            let prev = acu_mult.wrapping_sub(1);
            if prev >= start_450 && prev < end_450 {
                let mult = acu_mult.wrapping_sub(start_450);
                acu_sum_1_450 = acu_sum_1_450.wrapping_add(val.wrapping_mul(mult));
            }

            acu_mult = acu_mult.wrapping_add(1);
        }

        ChecksumResult {
            eac_crc: crc.finalize(),
            accurip_checksum_v1: acu_sum_1,
            accurip_checksum_v1_450: acu_sum_1_450,
            accurip_checksum_v2: acu_sum_2,
        }
    }

    #[test]
    fn vector_zero_digest_matches_fixture_values() {
        let d = [0u8; 64];
        assert_eq!(
            fun512_from_sha512_digest(d, 0),
            "AIGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgQ"
        );
        assert_eq!(
            fun512_from_sha512_digest(d, 15),
            "AJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkA"
        );
    }

    #[test]
    fn vector_sequence_digest_matches_fixture_values() {
        let mut d = [0u8; 64];
        for (i, b) in d.iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(
            fun512_from_sha512_digest(d, 0),
            "AIGAg4KFhIeGiYiLio2Mj46RkJOSlZSXlpmYm5qdnJ.eoaCjoqWkp6apqKuqrayvrrGws7K1tLe2ubi7ur28vw"
        );
        assert_eq!(
            fun512_from_sha512_digest(d, 7),
            "AIiJiouMjY6PgIGCg4SFhoeYmZqbnJ2en5CRkpOUlZaXqKmqq6ytrq.goaKjpKWmp7i5uru8vb6_sLGys7S1tg"
        );
    }

    #[test]
    fn verify_log_fixture_outcomes_match_expected() {
        assert_eq!(
            verify_log_path(Path::new("tests/fixtures/log/valid.log")),
            LogVerify::Valid
        );
        assert_eq!(
            verify_log_path(Path::new("tests/fixtures/log/mismatch.log")),
            LogVerify::Mismatch
        );
        assert_eq!(
            verify_log_path(Path::new("tests/fixtures/log/trailing.log")),
            LogVerify::TrailingData
        );
        assert_eq!(
            verify_log_path(Path::new("tests/fixtures/log/no_checksum.log")),
            LogVerify::NoChecksum
        );
    }

    #[test]
    fn verify_log_handles_missing_file() {
        assert_eq!(
            verify_log_path(Path::new("tests/fixtures/log/does_not_exist.log")),
            LogVerify::IoError
        );
    }

    #[test]
    fn checksum_ctx_smoke() {
        let mut ctx = ChecksumCtx::new(10_000, false, false);
        let data = [1u8, 2, 3, 4, 9, 8, 7, 6, 4, 5, 6, 7, 8, 9, 10, 11];
        ctx.process_bytes(&data);
        let res = ctx.finalize();

        assert_ne!(res.eac_crc, 0);
        assert_ne!(res.accurip_checksum_v1, 0);
    }

    #[test]
    fn checksum_ctx_matches_reference_for_first_last_windows() {
        let samples: Vec<u32> = (0..8000)
            .map(|i| (i as u32).wrapping_mul(97).wrapping_add(13))
            .collect();
        let bytes = samples_to_le_bytes(&samples);
        let nb_samples = samples.len() as u32;

        let scenarios = [(false, false), (true, false), (false, true), (true, true)];

        for (first, last) in scenarios {
            let mut ctx = ChecksumCtx::new(nb_samples, first, last);
            ctx.process_bytes(&bytes);
            let got = ctx.finalize();
            let expected = reference_checksums(&samples, nb_samples, first, last);
            assert_eq!(got, expected, "mismatch for first={first}, last={last}");
        }
    }

    #[test]
    fn checksum_ctx_matches_reference_with_chunked_processing() {
        let samples: Vec<u32> = (0..5000)
            .map(|i| (i as u32).wrapping_mul(1_000_003).wrapping_add(0xABCD))
            .collect();
        let bytes = samples_to_le_bytes(&samples);
        let nb_samples = samples.len() as u32;

        let expected = reference_checksums(&samples, nb_samples, true, false);

        let mut ctx = ChecksumCtx::new(nb_samples, true, false);
        for chunk in bytes.chunks(4 * 137) {
            ctx.process_bytes(chunk);
        }
        let got = ctx.finalize();

        assert_eq!(got, expected);
    }

    #[test]
    fn checksum_ctx_short_last_track_wrap_behavior_matches_c() {
        let samples: Vec<u32> = (0..128).map(|i| i as u32 + 1).collect();
        let bytes = samples_to_le_bytes(&samples);
        let nb_samples = samples.len() as u32;

        let mut ctx = ChecksumCtx::new(nb_samples, false, true);
        ctx.process_bytes(&bytes);
        let got = ctx.finalize();
        let expected = reference_checksums(&samples, nb_samples, false, true);

        assert_eq!(got, expected);
    }
}
