use std::time::Duration;

use anyhow::Result;

use crate::{
    auto::dataplane,
    port::{wait_for_command, write_line},
    proto::{
        command::{CtrlCommand, Direction, TestResultFlag},
        parser::{format_command, parse_command},
    },
    test::test_config::TestConfig,
};

pub fn run_hammer_test(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    conf: TestConfig,
    is_master: bool,
) -> Result<()> {
    // Auto slave should already be synced and configured
    if is_master {
        let test_cmd = CtrlCommand::TestBegin {
            id: my_id.to_string(),
            name: conf.name,
            frames: conf.frames,
            duration_ms: conf.duration_ms,
            payload: conf.payload,
            // If tester = tx then slave = rx, and vice versa
            dir: match conf.dir {
                Direction::Tx => Direction::Rx,
                Direction::Both => Direction::Both,
                Direction::Rx => Direction::Tx,
            },
        };
        write_line(port, &format_command(&test_cmd))?;
        wait_for_command(port, Some(Duration::from_millis(10_000)), |line: &str| {
            let result = parse_command(line);
            if let Ok(ref cmd) = result
                && let CtrlCommand::TestBeginAck { .. } = cmd
            {
                return Some(());
            }
            None
        })?;
    } else {
        let ack_cmd = CtrlCommand::TestBeginAck {
            id: my_id.to_string(),
            name: conf.name,
            frames: conf.frames,
            duration_ms: conf.duration_ms,
            payload: conf.payload,
            dir: conf.dir,
        };
        write_line(port, &format_command(&ack_cmd))?;
    }

    // Test is done, now wait for TestDone or send it
    let is_ack_mode = is_test_done_ack_mode(conf.dir, true);
    if is_ack_mode {
        wait_for_command(port, Some(Duration::from_millis(10_000)), |line: &str| {
            if let Ok(cmd) = parse_command(line)
                && let CtrlCommand::TestDone { .. } = cmd
            {
                return Some(());
            }
            None
        })?;
        let ack = CtrlCommand::TestDoneAck {
            id: my_id.to_string(),
        };
        write_line(&mut *port, &format_command(&ack))?;
    } else {
        let test_done = CtrlCommand::TestDone {
            id: my_id.to_string(),
            result: TestResultFlag::Fail, // placeholder
        };
        write_line(&mut *port, &format_command(&test_done))?;
        wait_for_command(port, Some(Duration::from_millis(10_000)), |line: &str| {
            if let Ok(cmd) = parse_command(line)
                && let CtrlCommand::TestDoneAck { .. } = cmd
            {
                return Some(());
            }
            None
        })?;
    }

    // TODO handle reporting
    let res = build_test_result(&my_id, None, "not implemented yet");
    write_line(&mut *port, &format_command(&res))?;
    // Optional: you can still log this locally
    log_test_result(&res);

    Ok(())
}

fn is_test_done_ack_mode(dir: Direction, is_master: bool) -> bool {
    match dir {
        Direction::Tx => false,
        Direction::Both if is_master => false,
        _ => true,
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
