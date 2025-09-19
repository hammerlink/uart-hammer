use anyhow::{Context, Result};
use std::sync::atomic::Ordering;
use std::time::Duration;
use uuid::Uuid;

use crate::cli::AutoOpts;
use crate::port::{
    PORT_DEBUG, open_control, port_default_config, retune_for_config, wait_for_command, write_line,
};
use crate::proto::command::CtrlCommand;
use crate::proto::parser::{format_command, parse_command};
use crate::test::runner::run_hammer_test;
use crate::test::test_config::TestConfig;

pub mod dataplane;

pub fn run(args: AutoOpts) -> Result<()> {
    if args.debug {
        PORT_DEBUG.store(true, Ordering::Relaxed);
    }
    // Open control channel at 115200 8N1 (line-mode)
    let mut port = open_control(&args.dev)
        .with_context(|| format!("opening control channel on {}", args.dev))?;
    // IDs
    let my_auto_id = Uuid::new_v4().to_string();
    let mut master_id = wait_for_master_sync(&mut *port, &my_auto_id)?;

    loop {
        let cmd = match wait_for_command(
            &mut *port,
            Some(Duration::from_millis(args.inactive_timeout_ms)),
            |line: &str| {
                let result = parse_command(line);
                if let Ok(ref cmd) = result {
                    eprintln!("[auto] got command: {:?}", cmd);
                    return Some(cmd.clone());
                }
                None
            },
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[auto] error waiting for command: {}", e);
                eprintln!("[auto] assuming master inactive, returning to HELLO");
                master_id = wait_for_master_sync(&mut *port, &my_auto_id)?;
                continue;
            }
        };

        match cmd {
            CtrlCommand::ConfigSet {
                id,
                baud,
                parity,
                bits,
                flow,
            } => {
                // ACK with same fields
                let ack = CtrlCommand::ConfigSetAck {
                    id: my_auto_id.clone(),
                    baud,
                    parity,
                    bits,
                    flow,
                };
                write_line(&mut *port, &format_command(&ack))?;
                retune_for_config(&mut *port, baud, parity, bits, flow)
                    .with_context(|| "retuning for CONFIG SET")?;
                eprintln!(
                    "[auto] config set by {}: baud={} parity={:?} bits={} flow={:?}",
                    id, baud, parity, bits, flow
                );
            }
            CtrlCommand::TestBegin {
                id,
                name,
                frames,
                duration_ms,
                payload,
                dir,
            } => {
                eprintln!("[auto] TEST BEGIN from master id={}", id);
                if id != master_id {
                    eprintln!(
                        "[auto] warning: TEST BEGIN from unknown master id={}, expected {}",
                        id, master_id
                    );
                    continue; // ignore
                }

                match run_hammer_test(
                    &mut *port,
                    &my_auto_id,
                    TestConfig {
                        name,
                        duration_ms,
                        frames,
                        payload,
                        dir,
                    },
                    false,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[auto] error during test: {}", e);
                    }
                };
            }

            // Peer RESULT (master’s) --------------------------------------
            CtrlCommand::TestResult { .. } => {
                // Optional: print/record master’s result
                // You can parse and mirror to console if you want.
            }

            // Termination -------------------------------------------------
            CtrlCommand::Terminate { .. } => {
                eprintln!("[auto] received TERMINATE from master id={}", master_id);
                // Acknowledge and go back to discovery
                let ack = CtrlCommand::TerminateAck {
                    id: my_auto_id.clone(),
                };
                write_line(&mut *port, &format_command(&ack))?;
                master_id = wait_for_master_sync(&mut *port, &my_auto_id)?;
            }
            _ => {
                eprintln!("[auto] warning: ignoring unexpected command {:?}", cmd);
            }
        }
    }
}

/* -------------------- helpers -------------------- */
fn wait_for_master_sync(port: &mut dyn serialport::SerialPort, my_id: &str) -> Result<String> {
    // Ensure port is in default config
    port_default_config(port)?;
    eprintln!("[auto] id={} awaiting master", my_id);

    let master_id = wait_for_command(port, None, |line: &str| {
        if let Ok(cmd) = parse_command(line)
            && let CtrlCommand::Hello { id } = cmd
        {
            eprintln!(
                "[auto] id={} got HELLO from master id={}, entering main loop",
                my_id,
                id.as_str()
            );
            return Some(id);
        }
        None
    })?;

    let ack = CtrlCommand::Ack {
        id: my_id.to_string(),
    };
    write_line(port, &format_command(&ack))?;
    Ok(master_id)
}
