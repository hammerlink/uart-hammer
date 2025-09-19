use std::{sync::atomic::Ordering, thread::sleep, time::Duration};

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::{
    cli::PortConfig,
    port::{
        PORT_DEBUG, open_control, port_default_config, retune_for_config, wait_for_command,
        write_line,
    },
    proto::{
        command::{CtrlCommand, Direction, TestName},
        parser::{format_command, parse_command},
    },
    test::{runner::run_hammer_test, test_config::TestConfig},
};

pub mod runner;
pub mod test_config;
pub mod test_max_rate;

pub fn run(args: crate::cli::TestOpts) -> Result<()> {
    if args.debug {
        PORT_DEBUG.store(true, Ordering::Relaxed);
    }
    let mut port = open_control(&args.dev)
        .with_context(|| format!("opening control channel on {}", args.dev))?;

    port_default_config(&mut *port)?;

    let my_test_id = Uuid::new_v4().to_string();
    eprintln!("[test] id={} awaiting slave", my_test_id);
    let _slave_id = wait_for_test_slave_sync(
        &mut *port,
        &my_test_id,
        args.hello_ms,
        args.hello_backoff_max_ms,
    )
    .with_context(|| "waiting for test slave sync")?;

    let mut port_config = args.to_port_config()?;
    port_config.baud = 57_600; // force 57600 for test
    send_config_set(&mut *port, &my_test_id, &port_config)?;

    run_hammer_test(
        &mut *port,
        &my_test_id,
        TestConfig {
            name: TestName::MaxRate,
            payload: 16,
            frames: Some(150),
            duration_ms: Some(Duration::from_secs(20).as_millis() as u64),
            dir: Direction::Tx,
        },
        true,
    )?;

    let terminate = CtrlCommand::Terminate { id: my_test_id };
    write_line(&mut *port, &format_command(&terminate))?;
    wait_for_command(
        &mut *port,
        Some(Duration::from_millis(5_000)),
        |line: &str| {
            let result = parse_command(line);
            if let Ok(ref cmd) = result
                && let CtrlCommand::TerminateAck { .. } = cmd
            {
                return Some(());
            }
            None
        },
    )?;

    Ok(())
}

fn wait_for_test_slave_sync(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    initial_ms: u64,
    max_ms: u64,
) -> Result<String> {
    // Ensure port is in default config

    let mut backoff = initial_ms.max(200);
    loop {
        let hello = CtrlCommand::Hello {
            id: my_id.to_string(),
        };
        write_line(port, &format_command(&hello))?;

        let slave_id =
            wait_for_command(port, Some(Duration::from_millis(backoff)), |line: &str| {
                let result = parse_command(line);
                if let Ok(ref cmd) = result
                    && let CtrlCommand::Ack { id } = cmd
                {
                    eprintln!("[test] got ACK from slave id={}", id);
                    return Some(id.clone());
                }
                None
            })
            .ok();
        if let Some(id) = slave_id {
            return Ok(id);
        }

        backoff = (backoff.saturating_mul(2)).min(max_ms.max(initial_ms));
    }
}

fn send_config_set(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    port_config: &PortConfig,
) -> Result<()> {
    let config_set = CtrlCommand::ConfigSet {
        id: my_id.to_string(),
        baud: port_config.baud,
        parity: port_config.parity,
        bits: port_config.bits,
        flow: port_config.flow,
    };
    write_line(port, &format_command(&config_set))?;
    wait_for_command(port, Some(Duration::from_millis(10_000)), |line: &str| {
        let result = parse_command(line);
        if let Ok(ref cmd) = result
            && let CtrlCommand::ConfigSetAck { .. } = cmd
        {
            return Some(());
        }
        None
    })?;
    retune_for_config(
        port,
        port_config.baud,
        port_config.parity,
        port_config.bits,
        port_config.flow,
    )?;
    sleep(Duration::from_millis(100)); // let settle
    Ok(())
}
