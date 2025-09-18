use anyhow::anyhow;
use clap::{Args, Parser, Subcommand};
use std::time::Duration;

#[derive(Parser, Debug, Clone)]
#[command(name = "uart-lab", about = "UART tester (tx/rx) with framing & stats")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Cmd {
    /// Receive and verify frames
    Rx(RxOpts),
    /// Transmit frames (max speed by default)
    Tx(TxOpts),
    /// Automated UART validation master/slave
    Auto(AutoOpts),
    /// Run specific tests (internal)
    Test(TestOpts),
}

#[derive(Args, Debug, Clone)]
pub struct SerialOpts {
    /// Serial device path
    #[arg(long, default_value = "/dev/ttyS0")]
    pub dev: String,
    /// Baud rate
    #[arg(long, default_value_t = 115_200)]
    pub baud: u32,
    /// Enable RTS/CTS
    #[arg(long, default_value_t = false)]
    pub rtscts: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RxOpts {
    #[command(flatten)]
    pub ser: SerialOpts,
    /// Bits per byte for implied baud (~bps): 10 for 8N1, 11 if parity/2 stop bits
    #[arg(long, default_value_t = 10)]
    pub bpb: u32,
    /// Print each BAD/LOST line
    #[arg(long, default_value_t = true)]
    pub debug: bool,
    /// Stats print interval in seconds
    #[arg(long, default_value_t = 1.0)]
    pub stats: f64,
}

#[derive(Args, Debug, Clone)]
pub struct TxOpts {
    #[command(flatten)]
    pub ser: SerialOpts,
    /// Payload size in bytes
    #[arg(long, default_value_t = 64)]
    pub len: usize,
    /// "max" or milliseconds gap (e.g. 0, 5, 10) or "auto"
    #[arg(long, default_value = "max")]
    pub gap: String,
    /// Bits per byte for pacing math
    #[arg(long, default_value_t = 10)]
    pub bpb: u32,
    /// Utilization (0.0..1.0) when gap="auto" (1.0 = line-rate)
    #[arg(long, default_value_t = 1.0)]
    pub util: f64,
    /// Print each sent line (slow)
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TestOpts {
    #[arg(long)]
    pub dev: String,
    #[arg(long)]
    pub master: bool,
    #[arg(long)]
    pub slave: bool,
    // test selection / params
    #[arg(long, default_value = "max-rate,fifo-residue")]
    pub tests: String,
    #[arg(long, default_value = "defaults")]
    pub bauds: String, // "defaults" or csv
    #[arg(long, default_value = "none,even,odd")]
    pub parity: String,
    #[arg(long, default_value = "8,7")]
    pub bits: String,
    #[arg(long, default_value = "tx,rx,both")]
    pub dir: String,
    #[arg(long, default_value = "none,rtscts")]
    pub flow: String,
    #[arg(long, default_value_t = 128)]
    pub payload: usize,
    #[arg(long, default_value_t = 10_000)]
    pub frames: usize,
    #[arg(long)]
    pub duration_ms: Option<u64>,
    // protocol timings
    #[arg(long, default_value_t = 500)]
    pub hello_ms: u64,
    #[arg(long, default_value_t = 4_000)]
    pub hello_backoff_max_ms: u64,
    #[arg(long, default_value_t = 10_000)]
    pub repeat_timeout_ms: u64,
    #[arg(long, default_value_t = 2)]
    pub repeat_hz: u32, // “current baud / 2” in spec; we’ll map to 2 Hz control repeats
}

#[derive(clap::Args, Debug, Clone)]
pub struct AutoOpts {
    #[arg(long)]
    pub dev: String,
    #[arg(long, default_value_t = 10_000)]
    pub repeat_timeout_ms: u64,
}

/// Typed pacing model to replace ad-hoc gap handling.
#[derive(Debug, Clone, Copy)]
pub enum Pacing {
    Max,
    Fixed(Duration),
    Auto { util: f64 },
}

impl Pacing {
    pub fn from_cli(gap: &str, util: f64) -> anyhow::Result<Self> {
        if gap.eq_ignore_ascii_case("max") {
            Ok(Pacing::Max)
        } else if gap.eq_ignore_ascii_case("auto") {
            Ok(Pacing::Auto { util })
        } else {
            let ms: u64 = gap
                .parse()
                .map_err(|_| anyhow!("gap must be integer ms, 'auto', or 'max'"))?;
            Ok(Pacing::Fixed(Duration::from_millis(ms)))
        }
    }
    /// Compute sleep time to achieve desired pacing given a write of `bytes`.
    pub fn sleep_for(self, bytes: usize, bpb: u32, baud: u32) -> Option<Duration> {
        match self {
            Pacing::Max => None,
            Pacing::Fixed(d) => Some(d),
            Pacing::Auto { util } => {
                let util = util.max(1e-3); // avoid div by 0
                let bit_time_s = (bytes as f64) * (bpb as f64) / (baud as f64);
                let target_s = bit_time_s / util;
                Some(Duration::from_micros((target_s * 1_000_000.0) as u64))
            }
        }
    }
}
