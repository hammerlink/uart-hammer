use crate::stats::Stats;
use std::time::Duration;

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
    pub fn from_test_stats(tx_stats: Stats, rx_stats: Stats) -> Self {
        let pass = rx_stats.ok > 0 && rx_stats.bad == 0 && rx_stats.lost == 0;
        let reason = if pass {
            None
        } else if rx_stats.ok == 0 && rx_stats.bad == 0 {
            Some("no frames received".into())
        } else if rx_stats.bad > 0 {
            Some("crc errors".into())
        } else if rx_stats.lost > 0 {
            Some("sequence gaps".into())
        } else {
            Some("unknown".into())
        };
        let dur = Duration::from_micros(tx_stats.duration_micros)
            .as_secs_f64()
            .max(1e-3);
        let bps_bytes = (rx_stats.bytes as f64) / dur;
        let bps_bits = (bps_bytes * (tx_stats.bpb as f64)) as u64;

        Self {
            pass,
            rx_frames: rx_stats.ok,
            rx_bytes: rx_stats.bytes,
            bad_crc: 0,
            seq_gaps: rx_stats.lost,
            overruns: 0,
            errors: rx_stats.bad as u32,
            rate_bps: bps_bits,
            reason,
        }
    }

    pub fn log(&self) {
        eprintln!(
            "[auto] {} frames={} bytes={} bad_crc={} gaps={} overruns={} errors=0x{:X} rate_bps={} reason={}",
            match self.pass {
                true => "PASS",
                false => "FAIL",
            },
            self.rx_frames,
            self.rx_bytes,
            self.bad_crc,
            self.seq_gaps,
            self.overruns,
            self.errors,
            self.rate_bps,
            self.reason.as_deref().unwrap_or("none"),
        );
    }
}
