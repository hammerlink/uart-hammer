use anyhow::{Context, Result};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::cli::AutoOpts;
use crate::port::{
    open_control, port_default_config, read_crlf_line, retune_for_config, write_line,
};
use crate::proto::command::{
    CtrlCommand, Direction, FlowControl, Parity, TestName, TestResultFlag,
};
use crate::proto::parser::{format_command, parse_command};

pub mod dataplane;

#[derive(Debug, Clone)]
struct CurrentConfig {
    baud: u32,
    parity: Parity,
    bits: u8,
    flow: FlowControl,
}

#[derive(Debug, Default)]
struct TestCtx {
    name: Option<TestName>,
    frames: Option<u64>,
    duration_ms: Option<u64>,
    payload: Option<usize>,
}

pub fn run(args: AutoOpts) -> Result<()> {
    // Open control channel at 115200 8N1 (line-mode)
    let mut port = open_control(&args.dev)
        .with_context(|| format!("opening control channel on {}", args.dev))?;
    // IDs
    let my_auto_id = Uuid::new_v4().to_string();
    let mut master_id: Option<String> = None;

    eprintln!("[slave] id={} starting HELLO loop", my_auto_id);

    // Wait for HELLO from master, ACK it
    listen_until_hello_sync(&mut *port, &my_auto_id)?;

    // Main loop state
    let mut cfg: Option<CurrentConfig> = None;
    let mut test = TestCtx::default();

    loop {
        let line = read_crlf_line(&mut *port, None)?;
        if line.is_none() {
            continue;
        }
        if let Ok(cmd) = parse_command(&line.unwrap()) {
            match cmd {
                // Discovery ---------------------------------------------------
                CtrlCommand::Ack { id } => {
                    // Slave should ack, do nothing
                }
                CtrlCommand::Hello { id } => {
                    // store master ID and ACK
                    master_id = Some(id.clone());
                    write_line(
                        &mut *port,
                        &format_command(&CtrlCommand::Ack {
                            id: my_auto_id.clone(),
                        }),
                    )?;
                }

                // Config ------------------------------------------------------
                CtrlCommand::ConfigSet {
                    id,
                    baud,
                    parity,
                    bits,
                    flow,
                } => {
                    master_id = Some(id.clone());
                    retune_for_config(&mut *port, baud, parity, bits, flow)
                        .with_context(|| "retuning for CONFIG SET")?;
                    eprintln!(
                        "[slave] config set by {}: baud={} parity={:?} bits={} flow={:?}",
                        id, baud, parity, bits, flow
                    );
                    cfg = Some(CurrentConfig {
                        baud,
                        parity,
                        bits,
                        flow,
                    });

                    // ACK with same fields
                    let ack = CtrlCommand::ConfigSetAck {
                        id: my_auto_id.clone(),
                        baud,
                        parity,
                        bits,
                        flow,
                    };
                    write_line(&mut *port, &format_command(&ack))?;
                }
                CtrlCommand::ConfigSetAck { .. } => {
                    // Slave never expects a ConfigSetAck; ignore
                }

                // Test orchestration -----------------------------------------
                CtrlCommand::TestBegin {
                    id,
                    name,
                    frames,
                    duration_ms,
                    payload,
                    dir,
                } => {
                    // (1) Stash context locally if you need it elsewhere
                    let _test_ctx = TestCtx {
                        name: Some(name.clone()),
                        frames,
                        duration_ms,
                        payload: Some(payload.clone()),
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

                    // TODO: replace this with your real test runner returning Option<TestOutcome>
                    let outcome: Option<dataplane::TestOutcome> = None;
                    end_with_repeat_mode(
                        &mut *port,
                        &my_auto_id,
                        ack_party(dir),
                        Duration::from_millis(args.repeat_timeout_ms),
                    )?;

                    let res =
                        build_test_result(&my_auto_id, outcome.as_ref(), "not implemented yet");
                    write_line(&mut *port, &format_command(&res))?;
                    // Optional: you can still log this locally
                    log_test_result(&res);
                }
                CtrlCommand::TestBeginAck { .. } => {
                    // Slave doesn’t expect this; ignore.
                }

                // End-of-test repeat-mode messages ----------------------------
                CtrlCommand::TestDone { id: _, result: _ } => {
                    // We are the ACK party in HD (RX) or FD (Slave), so ACK
                    let ack = CtrlCommand::TestDoneAck {
                        id: my_auto_id.clone(),
                    };
                    write_line(&mut *port, &format_command(&ack))?;
                }
                CtrlCommand::TestDoneAck { .. } => {
                    // Not strictly needed on slave (we only send ACKs), ignore
                }

                // Peer RESULT (master’s) --------------------------------------
                CtrlCommand::TestResult { .. } => {
                    // Optional: print/record master’s result
                    // You can parse and mirror to console if you want.
                }

                // Termination -------------------------------------------------
                CtrlCommand::Terminate { .. } => {
                    // Acknowledge and go back to discovery
                    let ack = CtrlCommand::TerminateAck {
                        id: my_auto_id.clone(),
                    };
                    write_line(&mut *port, &format_command(&ack))?;
                    // reset local state and start HELLO loop again
                    cfg = None;
                    test = TestCtx::default();
                    listen_until_hello_sync(&mut *port, &my_auto_id)?;
                }
                CtrlCommand::TerminateAck { .. } => {
                    // Slave doesn’t expect this; ignore
                }
            }
        }
    }
}

/* -------------------- helpers -------------------- */
fn listen_until_hello_sync(port: &mut dyn serialport::SerialPort, my_id: &str) -> Result<String> {
    // Ensure port is in default config
    port_default_config(port)?;

    loop {
        let line = read_crlf_line(port, None)?;
        if line.is_none() {
            continue;
        }
        if let Ok(cmd) = parse_command(&line.unwrap()) {
            if let CtrlCommand::Hello { id } = cmd {
                eprintln!(
                    "[auto] id={} got HELLO from master id={}, entering main loop",
                    my_id,
                    id.as_str()
                );
                // ACK back
                let ack = CtrlCommand::Ack {
                    id: my_id.to_string(),
                };
                write_line(port, &format_command(&ack))?;
                return Ok(id);
            }
        }
    }
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
            "[slave] result={:?} frames={} bytes={} bad_crc={} gaps={} overruns={} errors=0x{:X} rate_bps={} reason={}",
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
fn ack_party(dir: Direction) -> AckParty {
    match dir {
        Direction::Tx | Direction::Rx => AckParty::WeAckOnRx, // in HD, RX acks
        Direction::Both => AckParty::WeAckAlways,             // as slave in FD, we ack
    }
}

enum AckParty {
    WeAckOnRx,
    WeAckAlways,
}

/// Implements the end-of-test repeat-mode behavior.
/// - If we are the ACK party, we *only* send ACKs on incoming DONEs.
/// - Otherwise, we send DONE every 500ms until we see an ACK or timeout.
fn end_with_repeat_mode(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    ack_party: AckParty,
    timeout: Duration,
) -> Result<()> {
    let start = Instant::now();

    match ack_party {
        AckParty::WeAckOnRx | AckParty::WeAckAlways => {
            // For slave we never originate DONE in your spec; we ACK the other side's DONE.
            // We still need to sit here for the repeat window to catch the DONE and ACK it.
            while start.elapsed() < timeout {
                let line = read_crlf_line(port, Some(timeout))?;
                if line.is_none() {
                    continue;
                }
                if let Ok(cmd) = parse_command(&line.unwrap())
                    && let CtrlCommand::TestDone { .. } = cmd
                {
                    let ack = CtrlCommand::TestDoneAck {
                        id: my_id.to_string(),
                    };
                    write_line(port, &format_command(&ack))?;
                    return Ok(());
                }
            }
            // Timeout without seeing DONE — fine, proceed
            Ok(())
        }
    }
}
