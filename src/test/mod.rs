use std::{sync::atomic::Ordering, time::Duration};

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::{
    port::{DEBUG, open_control, port_default_config, wait_for_command, write_line},
    proto::{
        command::CtrlCommand,
        parser::{format_command, parse_command},
    },
};

pub fn run(args: crate::cli::TestOpts) -> Result<()> {
    if args.debug {
        DEBUG.store(true, Ordering::Relaxed);
    }
    let mut port = open_control(&args.dev)
        .with_context(|| format!("opening control channel on {}", args.dev))?;

    port_default_config(&mut *port)?;

    let my_auto_id = Uuid::new_v4().to_string();
    eprintln!("[test] id={} awaiting slave", my_auto_id);
    let _slave_id = wait_for_test_slave_sync(
        &mut *port,
        &my_auto_id,
        args.hello_ms,
        args.hello_backoff_max_ms,
    )
    .with_context(|| "waiting for test slave sync")?;

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
