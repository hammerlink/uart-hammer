use anyhow::{Context, Result};
use std::sync::atomic::Ordering;
use std::time::Duration;
use uuid::Uuid;

use crate::cli::AutoOpts;
use crate::port::{
    PORT_DEBUG, open_control, port_default_config, retune_for_config, wait_for_command, write_line,
};
use crate::proto::command::{CtrlCommand, Direction, TestName, TestResultFlag};
use crate::proto::parser::{format_command, parse_command};

pub mod dataplane;

#[derive(Debug, Default)]
struct TestCtx {
    name: Option<TestName>,
    frames: Option<u64>,
    duration_ms: Option<u64>,
    payload: Option<usize>,
}

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

    // Main loop state
    let mut test = TestCtx::default();

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
                // reset local state and start HELLO loop again
                test = TestCtx::default();
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
                if id != master_id {
                    eprintln!(
                        "[auto] warning: TEST BEGIN from unknown master id={}, expected {}",
                        id, master_id
                    );
                    continue; // ignore
                }
                // (1) Stash context locally if you need it elsewhere
                let _test_ctx = TestCtx {
                    name: Some(name),
                    frames,
                    duration_ms,
                    payload: Some(payload),
                };

                // (2) Send ACK back immediately
                let ack = CtrlCommand::TestBeginAck {
                    id: my_auto_id.to_string(),
                    name,
                    frames,
                    duration_ms,
                    payload,
                    dir,
                };
                write_line(&mut *port, &format_command(&ack))?;

                // TODO implement test logic here

                let is_ack_mode = is_test_done_ack_mode(dir);
                if is_ack_mode {
                    wait_for_test_done(&mut *port, Duration::from_millis(args.repeat_timeout_ms))?;
                    let ack = CtrlCommand::TestDoneAck {
                        id: my_auto_id.clone(),
                    };
                    write_line(&mut *port, &format_command(&ack))?;
                } else {
                    let test_done = CtrlCommand::TestDone {
                        id: my_auto_id.clone(),
                        result: TestResultFlag::Fail, // placeholder
                    };
                    write_line(&mut *port, &format_command(&test_done))?;
                }

                let res = build_test_result(&my_auto_id, None, "not implemented yet");
                write_line(&mut *port, &format_command(&res))?;
                // Optional: you can still log this locally
                log_test_result(&res);
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
                // reset local state and start HELLO loop again
                test = TestCtx::default();
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

fn wait_for_test_done(port: &mut dyn serialport::SerialPort, timeout: Duration) -> Result<()> {
    wait_for_command(port, Some(timeout), |line: &str| {
        if let Ok(cmd) = parse_command(line)
            && let CtrlCommand::TestDone { .. } = cmd
        {
            return Some(());
        }
        None
    })
}

fn build_test_result(
    id: &str,
    outcome: Option<&dataplane::TestOutcome>,
    default_reason: &str,
) -> CtrlCommand {
    match outcome {
        Some(outcome) => CtrlCommand::TestResult {
            id: id.to_string(),
            result: if outcome.pass {
                TestResultFlag::Pass
            } else {
                TestResultFlag::Fail
            },
            rx_frames: outcome.rx_frames,
            rx_bytes: outcome.rx_bytes,
            bad_crc: outcome.bad_crc,
            seq_gaps: outcome.seq_gaps,
            overruns: outcome.overruns,
            errors: outcome.errors,
            rate_bps: outcome.rate_bps,
            reason: outcome.reason.clone(),
        },
        None => CtrlCommand::TestResult {
            id: id.to_string(),
            result: TestResultFlag::Fail,
            rx_frames: 0,
            rx_bytes: 0,
            bad_crc: 0,
            seq_gaps: 0,
            overruns: 0,
            errors: 0,
            rate_bps: 0,
            reason: Some(default_reason.into()),
        },
    }
}

fn log_test_result(res: &CtrlCommand) {
    if let CtrlCommand::TestResult {
        result,
        rx_frames,
        rx_bytes,
        bad_crc,
        seq_gaps,
        overruns,
        errors,
        rate_bps,
        reason,
        ..
    } = res
    {
        eprintln!(
            "[auto] result={:?} frames={} bytes={} bad_crc={} gaps={} overruns={} errors=0x{:X} rate_bps={} reason={}",
            result,
            rx_frames,
            rx_bytes,
            bad_crc,
            seq_gaps,
            overruns,
            errors,
            rate_bps,
            reason.as_deref().unwrap_or("")
        );
    }
}

/// Which side sends ACK in repeat-mode?
fn is_test_done_ack_mode(dir: Direction) -> bool {
    match dir {
        Direction::Tx => false,
        Direction::Both | Direction::Rx => true,
    }
}
