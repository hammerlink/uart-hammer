use anyhow::anyhow;
use clap::{Args, Parser, Subcommand};
use std::time::Duration;

use crate::{
    port::DEFAULT_CONFIG,
    proto::command::{Direction, FlowControl, Parity, TestName},
};

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
    // test selection / params
    #[arg(long, default_value = "max-rate,fifo-residue")]
    pub tests: String,
    #[arg(long, default_value = "115200,57600,38400,19200,9600")]
    pub bauds: String,
    #[arg(long, default_value = "none")] // none,even,odd
    pub parity: String,
    #[arg(long, default_value = "8")]
    pub bits: String,
    #[arg(long, default_value = "tx,rx")] // list of tx,rx,both
    pub dir: String,
    #[arg(long, default_value = "none")] // none,rtscts
    pub flow: String,
    #[arg(long, default_value_t = 32)]
    pub payload: usize,
    #[arg(long, default_value_t = 200)]
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
    /// Print each CMD line
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct AutoOpts {
    #[arg(long)]
    pub dev: String,
    #[arg(long, default_value_t = 10_000)]
    pub repeat_timeout_ms: u64,
    /// Print each CMD line
    #[arg(long, default_value_t = false)]
    pub debug: bool,
    /// Inactive time out
    #[arg(long, default_value_t = 60_000)]
    pub inactive_timeout_ms: u64,
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

/// Cleaned-up struct for a parsed configuration
#[derive(Debug, Clone, Copy)]
pub struct PortConfig {
    pub baud: u32,
    pub parity: Parity,
    pub bits: u8,
    pub flow: FlowControl,
    pub stop_bits: u8, // currently always 1
}

impl PortConfig {
    pub fn bits_per_byte(&self) -> u32 {
        let start_bits = 1;

        let parity_bits = match self.parity {
            Parity::None => 0,
            _ => 1,
        };

        let stop_bits = self.stop_bits as u32; // adjust if you later add a field for stop bits

        start_bits + self.bits as u32 + parity_bits + stop_bits
    }
}

impl TestOpts {
    pub fn get_port_configs(&self) -> Vec<PortConfig> {
        let bauds = self.get_baud_rates();
        let parities = self.get_parities();
        let bits_list = self.get_bits();
        let flow_controls = self.get_flow_controls();

        let mut configs = Vec::new();
        for &baud in &bauds {
            for &parity in &parities {
                for &bits in &bits_list {
                    for &flow in &flow_controls {
                        configs.push(PortConfig {
                            baud,
                            parity,
                            bits,
                            flow,
                            stop_bits: 1,
                        });
                    }
                }
            }
        }

        if configs.is_empty() {
            configs.push(DEFAULT_CONFIG);
        }

        configs
    }

    pub fn get_bits(&self) -> Vec<u8> {
        let bits: Vec<u8> = self
            .bits
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if bits.is_empty() { vec![8] } else { bits }
    }

    pub fn get_flow_controls(&self) -> Vec<FlowControl> {
        let flow_controls: Vec<FlowControl> = self
            .flow
            .split(',')
            .filter_map(|s| match s.trim() {
                "none" => Some(FlowControl::None),
                "rtscts" => Some(FlowControl::RtsCts),
                _ => None,
            })
            .collect();
        if flow_controls.is_empty() {
            vec![FlowControl::None]
        } else {
            flow_controls
        }
    }

    pub fn get_dirs(&self) -> Vec<Direction> {
        let dirs: Vec<Direction> = self
            .dir
            .split(',')
            .filter_map(|s| match s.trim() {
                "tx" => Some(Direction::Tx),
                "rx" => Some(Direction::Rx),
                "both" => Some(Direction::Both),
                _ => None,
            })
            .collect();
        if dirs.is_empty() {
            vec![Direction::Tx]
        } else {
            dirs
        }
    }

    pub fn get_parities(&self) -> Vec<Parity> {
        let parities: Vec<Parity> = self
            .parity
            .split(',')
            .filter_map(|s| match s.trim() {
                "none" => Some(Parity::None),
                "even" => Some(Parity::Even),
                "odd" => Some(Parity::Odd),
                _ => None,
            })
            .collect();
        if parities.is_empty() {
            vec![Parity::None]
        } else {
            parities
        }
    }

    pub fn get_baud_rates(&self) -> Vec<u32> {
        if self.bauds.trim() == "*" {
            return vec![9_600, 19_200, 38_400, 57_600, 115_200, 230_400, 460_800, 921_600, 1_000_000, 1_500_000, 3_000_000];
        }
        let bauds: Vec<u32> = self
            .bauds
            .replace("_", "")
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if bauds.is_empty() {
            vec![115_200]
        } else {
            bauds
        }
    }

    pub fn get_test_names(&self) -> Vec<TestName> {
        if self.tests.trim() == "*" {
            return vec![TestName::MaxRate, TestName::FifoResidue];
        }
        self.tests
            .split(',')
            .filter_map(|s| match s.trim() {
                "max-rate" => Some(TestName::MaxRate),
                "fifo-residue" => Some(TestName::FifoResidue),
                _ => None,
            })
            .collect()
    }
}
