use anyhow::Result;
use std::io::{BufRead, BufReader};

use crate::cli::RxOpts;
use crate::frame::parse_frame;
use crate::port::open_port;
use crate::stats::Stats;

pub fn run(opts: RxOpts) -> Result<()> {
    eprintln!("rx: {:?}", opts);
    let port = open_port(&opts.ser)?;
    let mut reader = BufReader::new(port);
    let mut line = String::new();

    let mut stats = Stats::new(opts.bpb);
    let mut expect: Option<u64> = None;

    eprintln!("Starting receive loop");

    loop {
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
                if let Some(e) = expect {
                    if f.seq != e {
                        let lost = if f.seq > e { f.seq - e } else { 1 };
                        stats.add_lost(lost);
                        if opts.debug {
                            eprintln!(
                                "[LOST] got={} expect={} (+{}) line=\"{}\"",
                                f.seq,
                                e,
                                lost,
                                line.trim_end()
                            );
                        }
                    }
                    expect = Some(f.seq.wrapping_add(1));
                } else {
                    expect = Some(f.seq.wrapping_add(1));
                }
            }
            Err(err) => {
                stats.inc_bad();
                if opts.debug {
                    eprintln!("[BAD ] {} line=\"{}\"", err, line.trim_end());
                }
            }
        }

        stats.maybe_print(opts.stats);
    }
}
