use std::time::Duration;

use anyhow::{Context, Result};

use crate::{
    proto::command::CtrlCommand,
    port::{port_default_config, read_crlf_line, write_line},
    proto::parser::{format_command, parse_command},
};

pub fn run(opts: crate::cli::TestOpts) -> Result<()> {
    // TODO
    Ok(())
}

fn hello_until_ack_sync(
    port: &mut dyn serialport::SerialPort,
    my_id: &str,
    initial_ms: u64,
    max_ms: u64,
) -> Result<()> {
    // Ensure port is in default config
    port_default_config(port)?;

    let mut backoff = initial_ms.max(50);
    loop {
        let hello = CtrlCommand::Hello {
            id: my_id.to_string(),
        };
        write_line(port, &format_command(&hello))?;

        // poll for ACK within current backoff
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(backoff) {
            let line = read_crlf_line(port, None)?;
            if line.is_none() {
                continue;
            }
            if let Ok(cmd) = parse_command(&line.unwrap())
                && matches!(cmd, CtrlCommand::Ack { .. })
            {
                return Ok(());
            }
        }
        backoff = (backoff.saturating_mul(2)).min(max_ms.max(initial_ms));
    }
}
