// src/auto/dataplane.rs (or outcome.rs)

/// Result of running one test (local side).
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// true = PASS, false = FAIL
    pub pass: bool,

    /// total frames received
    pub rx_frames: u64,
    /// total bytes received
    pub rx_bytes: u64,

    /// number of frames with CRC/checksum errors
    pub bad_crc: u64,
    /// number of sequence number gaps
    pub seq_gaps: u64,

    /// hardware FIFO overrun count (if available)
    pub overruns: u64,
    /// other error flags, packed as bitmask if driver exposes them
    pub errors: u32,

    /// measured receive rate in bits per second
    pub rate_bps: u64,

    /// reason for failure (optional, e.g. "crc errors", "timeout")
    pub reason: Option<String>,
}

impl TestOutcome {
    /// Convenience: mark a clean pass.
    pub fn pass(rx_frames: u64, rx_bytes: u64, rate_bps: u64) -> Self {
        Self {
            pass: true,
            rx_frames,
            rx_bytes,
            bad_crc: 0,
            seq_gaps: 0,
            overruns: 0,
            errors: 0,
            rate_bps,
            reason: None,
        }
    }

    /// Convenience: mark a fail with a reason.
    pub fn fail(reason: impl Into<String>) -> Self {
        Self {
            pass: false,
            rx_frames: 0,
            rx_bytes: 0,
            bad_crc: 0,
            seq_gaps: 0,
            overruns: 0,
            errors: 0,
            rate_bps: 0,
            reason: Some(reason.into()),
        }
    }
}
