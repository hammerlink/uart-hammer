use std::time::Duration;

use anyhow::Result;

use crate::{
    auto::dataplane::{self, TestOutcome},
    port::{wait_for_command, write_line},
    proto::{
        command::{CtrlCommand, Direction, TestResultFlag},
        parser::{format_command, parse_command},
    },
    stats::Stats,
    test::{
        test_config::TestConfig,
        test_max_rate::{run_max_rate_rx, run_max_rate_tx},
    },
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

    // TODO handle multiple test types
    let stats = match conf.dir {
        Direction::Tx => run_max_rate_tx(port, conf.clone())?,
        Direction::Rx => run_max_rate_rx(port, conf.clone())?,
        Direction::Both => Stats::new(8),
    };

    let is_ack_mode = is_test_done_ack_mode(conf.dir, true);
    let mut other_stats: Option<Stats> = None;

    // Send Done and Ack with stats sharing
    if !is_master {
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
            ok: stats.ok,
            bad: stats.bad,
            lost: stats.lost,
            total: stats.total,
            duration_micros: stats.duration_micros,
        };
        write_line(&mut *port, &format_command(&ack))?;
    } else {
        let test_done_ack = wait_for_test_done_ack_sync(&mut *port, my_id, 200, 1_000)?;
        other_stats = if let CtrlCommand::TestDoneAck {
            ok,
            bad,
            lost,
            total,
            duration_micros,
            ..
        } = test_done_ack
        {
            Some(Stats {
                ok,
                bad,
                lost,
                total,
                duration_micros,
                ..Stats::new(8)
            })
        } else {
            None
        };
    }
    if is_master && other_stats.is_some() {
        let outcome: TestOutcome = if is_ack_mode {
            // is_ack_mode = is rx
            TestOutcome::from_test_stats(other_stats.unwrap(), stats)
        } else {
            TestOutcome::from_test_stats(stats, other_stats.unwrap())
        };
        outcome.log();
    }

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

fn wait_for_test_done_ack_sync(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    initial_ms: u64,
    max_ms: u64,
) -> Result<CtrlCommand> {
    let mut backoff = initial_ms.max(200);

    let cmd = CtrlCommand::TestDone {
        id: my_id.to_string(),
    };
    let line = format_command(&cmd);
    loop {
        write_line(port, &line)?;

        let test_done_ack =
            wait_for_command(port, Some(Duration::from_millis(backoff)), |line: &str| {
                let result = parse_command(line);
                if let Ok(ref cmd) = result
                    && let CtrlCommand::TestDoneAck { .. } = cmd
                {
                    return Some(cmd.clone());
                }
                None
            })
            .ok();
        if test_done_ack.is_some() {
            return Ok(test_done_ack.unwrap());
        }

        backoff = (backoff.saturating_mul(2)).min(max_ms.max(initial_ms));
    }
}
