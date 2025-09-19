use anyhow::Result;
use std::io::{BufRead, BufReader};

use crate::{
    cli::Pacing,
    frame::{build_frame, parse_frame},
    port::get_port_config,
    stats::Stats,
    test::test_config::TestConfig,
};

const MAX_RATE: f64 = 0.999; // target 99.9% utilization
const MAX_TEST_DURATION_MS: u64 = 20_000;

pub fn run_max_rate_tx(
    port: &mut dyn serialport::SerialPort,
    TestConfig {
        duration_ms: input_duration_ms,
        frames,
        payload,
        ..
    }: TestConfig,
) -> Result<Stats> {
    let port_config = get_port_config();
    let start = std::time::Instant::now();
    let mut stats = crate::stats::Stats::new(port_config.bits as u32);
    let duration_ms = input_duration_ms.unwrap_or(MAX_TEST_DURATION_MS);
    let bits_per_byte = port_config.bits_per_byte();
    let mut seq: u64 = 0;
    let pacing = Pacing::Auto { util: MAX_RATE };
    let mut out = Vec::with_capacity(payload * 2 + 2);

    loop {
        if start.elapsed().as_millis() as u64 >= duration_ms {
            break;
        }
        if let Some(max_frames) = frames
            && seq >= max_frames
        {
            break;
        }
        out.clear();
        let line = build_frame(seq, payload);
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
        port.write_all(&out)?;

        // Update stats
        stats.add_bytes(out.len());
        stats.inc_ok();

        if let Some(sleep) = pacing.sleep_for(out.len(), bits_per_byte, port_config.baud) {
            std::thread::sleep(sleep);
        }
        seq = seq.wrapping_add(1);
    }
    stats.duration_micros = start.elapsed().as_micros() as u64;

    Ok(stats)
}

pub fn run_max_rate_rx(
    port: &mut dyn serialport::SerialPort,
    TestConfig {
        duration_ms: input_duration_ms,
        frames,
        ..
    }: TestConfig,
) -> Result<Stats> {
    let start = std::time::Instant::now();
    let mut reader = BufReader::new(port.try_clone()?); // Clone it for independent read/write handles
    let mut line = String::new();

    let mut stats = crate::stats::Stats::new(get_port_config().bits as u32);
    let duration_ms = input_duration_ms.unwrap_or(MAX_TEST_DURATION_MS);
    let mut expect: Option<u64> = None;

    loop {
        if start.elapsed().as_millis() as u64 >= duration_ms {
            break;
        }
        if let Some(max_frames) = frames
            && stats.total >= max_frames
        {
            break;
        }
        line.clear();

        let line_result = reader.read_line(&mut line);
        let n = line_result.unwrap_or(0); // newline-terminated; timeout is set by builder
        if n == 0 {
            continue;
        } // timeout
        stats.add_bytes(n);

        match parse_frame(line.trim_end()) {
            Ok(f) => {
                stats.inc_ok();
                if let Some(e) = expect
                    && f.seq != e
                {
                    let lost = if f.seq > e { f.seq - e } else { 1 };
                    stats.add_lost(lost);
                }
                expect = Some(f.seq.wrapping_add(1));
            }
            Err(_) => {
                stats.inc_bad();
            }
        }
    }
    stats.duration_micros = start.elapsed().as_micros() as u64;

    Ok(stats)
}
