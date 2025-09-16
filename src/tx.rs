use anyhow::{Context, Result};
use std::io::Write;

use crate::cli::{Pacing, TxOpts};
use crate::frame::build_frame;
use crate::port::open_port;

pub fn run(opts: TxOpts) -> Result<()> {
    let mut port = open_port(&opts.ser)?;
    let pacing = Pacing::from_cli(&opts.gap, opts.util)?;

    let mut seq: u64 = 0;
    let mut out = Vec::with_capacity(opts.len * 2 + 2);

    if opts.debug {
        eprintln!(
            "[tx] dev={} baud={} len={} gap={} bpb={} util={} rtscts={}",
            opts.ser.dev, opts.ser.baud, opts.len, opts.gap, opts.bpb, opts.util, opts.ser.rtscts
        );
    }

    loop {
        out.clear();
        let line = build_frame(seq, opts.len);
        if opts.debug {
            eprintln!("[tx] {}", line);
        }
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
        port.write_all(&out).context("serial write")?;

        if let Some(sleep) = pacing.sleep_for(out.len(), opts.bpb, opts.ser.baud) {
            std::thread::sleep(sleep);
        }

        seq = seq.wrapping_add(1);
    }
}
