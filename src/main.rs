use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serialport::SerialPort;
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name="uart-lab", about="UART tester (tx/rx) with framing & stats")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Receive and verify frames
    Rx {
        #[arg(long, default_value="/dev/ttyS0")] dev: String,
        #[arg(long, default_value_t=115200)] baud: u32,
        /// Bits per byte for implied baud (~bps): 10 for 8N1, 11 if parity/2 stop bits
        #[arg(long, default_value_t=10)] bpb: u32,
        /// Print each BAD/LOST line
        #[arg(long, default_value_t=true)] debug: bool,
        /// Stats print interval in seconds
        #[arg(long, default_value_t=1.0)] stats: f64,
        /// Enable RTS/CTS
        #[arg(long, default_value_t=false)] rtscts: bool,
    },
    /// Transmit frames (max speed by default)
    Tx {
        #[arg(long, default_value="/dev/ttyS0")] dev: String,
        #[arg(long, default_value_t=115200)] baud: u32,
        /// Payload size in bytes
        #[arg(long, default_value_t=64)] len: usize,
        /// "max" or milliseconds gap (e.g. 0, 5, 10)
        #[arg(long, default_value="max")] gap: String,
        /// Bits per byte for pacing math
        #[arg(long, default_value_t=10)] bpb: u32,
        /// Utilization (0.0..1.0) when gap="auto" (1.0 = line-rate)
        #[arg(long, default_value_t=1.0)] util: f64,
        /// Enable RTS/CTS
        #[arg(long, default_value_t=false)] rtscts: bool,
        /// Print each sent line (slow)
        #[arg(long, default_value_t=false)] debug: bool,
    },
}

fn open_port(dev: &str, baud: u32, rtscts: bool) -> Result<Box<dyn SerialPort>> {
    let b = serialport::new(dev, baud)
        .timeout(Duration::from_millis(100))
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(if rtscts {
            serialport::FlowControl::Hardware
        } else {
            serialport::FlowControl::None
        });
    b.open().with_context(|| format!("open {}", dev))
}

fn hexsum(payload_hex: &str) -> Result<u8> {
    if payload_hex.len() % 2 != 0 { bail!("odd hex length"); }
    let mut sum: u32 = 0;
    for i in (0..payload_hex.len()).step_by(2) {
        let b = u8::from_str_radix(&payload_hex[i..i+2], 16)
            .with_context(|| "bad hex in PAY")?;
        sum = (sum + b as u32) & 0xFF;
    }
    Ok(sum as u8)
}

#[derive(Debug)]
struct Frame { seq: u64, len: usize, pay_hex: String, sum: u8 }

fn parse_frame(line: &str) -> Result<Frame> {
    // tolerate leading/trailing markers and flexible order
    let mut seq=None; let mut len=None; let mut pay=None; let mut sum=None;
    for tok in line.split_whitespace() {
        if let Some(v)=tok.strip_prefix("SEQ=") { seq = Some(v.parse::<u64>()?) }
        else if let Some(v)=tok.strip_prefix("LEN="){ len = Some(v.parse::<usize>()?) }
        else if let Some(v)=tok.strip_prefix("PAY="){ pay = Some(v.to_string()) }
        else if let Some(v)=tok.strip_prefix("SUM="){ sum = Some(u8::from_str_radix(v,16)?) }
    }
    let (seq,len,pay,sumrx) = (seq.ok_or_else(|| anyhow::anyhow!("no SEQ"))?,
                               len.ok_or_else(|| anyhow::anyhow!("no LEN"))?,
                               pay.ok_or_else(|| anyhow::anyhow!("no PAY"))?,
                               sum.ok_or_else(|| anyhow::anyhow!("no SUM"))?);
    if pay.len() != len*2 { bail!("len mismatch"); }
    let calc = hexsum(&pay)?;
    if calc != sumrx { bail!("checksum {}!={}", calc, sumrx); }
    Ok(Frame{ seq, len, pay_hex: pay, sum: sumrx })
}

fn build_frame(seq: u64, len: usize) -> String {
    // PAY = (i+seq) % 256 pattern
    let mut sum: u32 = 0;
    let mut s = String::with_capacity(2*len);
    for i in 0..len {
        let b = ((i as u64 + seq) & 0xFF) as u8;
        sum = (sum + b as u32) & 0xFF;
        use std::fmt::Write;
        let _ = write!(s, "{:02X}", b);
    }
    format!("@@ SEQ={} LEN={} PAY={} SUM={:02X} ##", seq, len, s, sum as u8)
}

fn rx(dev: String, baud: u32, bpb: u32, debug: bool, stats_int: f64, rtscts: bool) -> Result<()> {
    let port = open_port(&dev, baud, rtscts)?;
    let mut reader = BufReader::new(port);
    let mut line = String::new();

    let mut ok=0u64; let mut bad=0u64; let mut lost=0u64;
    let mut expect: Option<u64> = None;
    let mut bytes: u64 = 0;
    let mut t0 = Instant::now();
    let mut last = Instant::now();

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 { continue; } // timeout
        // count bytes on wire (include newline)
        bytes += n as u64;

        match parse_frame(line.trim_end()) {
            Ok(f) => {
                ok += 1;
                if let Some(e) = expect {
                    if f.seq != e {
                        if f.seq > e { lost += f.seq - e; }
                        else { lost += 1; }
                        if debug {
                            eprintln!("[LOST] got={} expect={} (+{}) line=\"{}\"",
                                f.seq, e, if f.seq>e {f.seq-e} else {1}, line.trim_end());
                        }
                    }
                    expect = Some(f.seq + 1);
                } else {
                    expect = Some(f.seq + 1);
                }
            }
            Err(err) => {
                bad += 1;
                if debug {
                    eprintln!("[BAD ] {}  line=\"{}\"", err, line.trim_end());
                }
            }
        }

        if last.elapsed().as_secs_f64() >= stats_int {
            let dur = t0.elapsed().as_secs_f64().max(1e-3);
            let bps_bytes = (bytes as f64)/dur;
            let bps_bits  = bps_bytes * (bpb as f64);
            eprintln!("[rx] ok={} bad={} lost={} bytes={} over {:.1}s  => {:.1}kB/s (~{:.0} bps, bpb={})",
                ok, bad, lost, bytes, dur, bps_bytes/1000.0, bps_bits, bpb);
            last = Instant::now();
            t0 = Instant::now();
            bytes = 0;
        }
    }
}

fn tx(dev: String, baud: u32, len: usize, gap: String, bpb: u32, util: f64, rtscts: bool, debug: bool) -> Result<()> {
    let mut port = open_port(&dev, baud, rtscts)?;
    let mut seq: u64 = 0;
    let auto = gap.eq_ignore_ascii_case("auto");
    let fixed_ms = if !auto && gap != "max" {
        Some(gap.parse::<u64>().context("gap must be integer ms, 'auto', or 'max'")?)
    } else { None };

    loop {
        let line = build_frame(seq, len);
        if debug { eprintln!("[tx] {}", line); }
        // write once per line (includes CRLF)
        let mut buf = line.into_bytes();
        buf.extend_from_slice(b"\r\n");
        port.write_all(&buf)?;

        if auto {
            let bytes = buf.len() as f64;
            let ms = (bytes * (bpb as f64) * 1000.0 / (baud as f64)) / util.max(1e-3);
            std::thread::sleep(Duration::from_micros((ms*1000.0) as u64));
        } else if let Some(ms) = fixed_ms {
            std::thread::sleep(Duration::from_millis(ms));
        } // else "max": no sleep

        seq = seq.wrapping_add(1);
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Rx { dev, baud, bpb, debug, stats, rtscts } =>
            rx(dev, baud, bpb, debug, stats, rtscts),
        Cmd::Tx { dev, baud, len, gap, bpb, util, rtscts, debug } =>
            tx(dev, baud, len, gap, bpb, util, rtscts, debug),
    }
}
