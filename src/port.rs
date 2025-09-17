use anyhow::{Result, bail};
use serialport::{DataBits, SerialPort};
use std::time::{Duration, Instant};

use crate::{
    auto::proto::command::{FlowControl, Parity},
    cli::SerialOpts,
};

pub fn open_port(opts: &SerialOpts) -> Result<Box<dyn SerialPort>> {
    let builder = serialport::new(&opts.dev, opts.baud)
        .timeout(Duration::from_millis(100))
        .data_bits(DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(if opts.rtscts {
            serialport::FlowControl::Hardware
        } else {
            serialport::FlowControl::None
        });

    builder
        .open()
        .map_err(|e| anyhow::anyhow!("open {}: {}", opts.dev, e))
}

pub fn retune_for_config(
    port: &mut dyn serialport::SerialPort,
    baud: u32,
    parity: Parity,
    bits: u8,
    flow: FlowControl,
) -> Result<()> {
    use serialport::{DataBits, FlowControl as SpFlow, Parity as SpParity, StopBits};
    port.set_baud_rate(baud)?;
    port.set_data_bits(match bits {
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        other => bail!("unsupported data bits: {}", other),
    })?;
    port.set_parity(match parity {
        Parity::None => SpParity::None,
        Parity::Even => SpParity::Even,
        Parity::Odd => SpParity::Odd,
    })?;
    port.set_stop_bits(StopBits::One)?; // spec: only 1 stop bit
    port.set_flow_control(match flow {
        FlowControl::None => SpFlow::None,
        FlowControl::RtsCts => SpFlow::Hardware,
    })?;
    Ok(())
}

/// Open the *control channel* (always 115200, 8N1, no flow)
pub fn open_control(dev: &str) -> Result<Box<dyn SerialPort>> {
    let builder = serialport::new(dev, 115_200)
        .timeout(Duration::from_millis(100))
        .data_bits(DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None);

    builder
        .open()
        .map_err(|e| anyhow::anyhow!("open control {}: {}", dev, e))
}

/// Write a line (string must already have \r\n)
pub fn write_line(port: &mut dyn SerialPort, line: &str) -> Result<()> {
    port.write_all(line.as_bytes())?;
    port.flush()?;
    Ok(())
}

pub fn read_crlf_line(port: &mut dyn SerialPort, timeout: Option<Duration>) -> Result<Option<String>> {
    let deadline = match timeout {
        Some(t) => Some(Instant::now() + t),
        None => None,
    };

    // Instant::now() + timeout;
    let mut buf = [0u8; 1];
    let mut line = Vec::new();

    loop {
        if deadline.is_some() && Instant::now() >= deadline.unwrap() {
            return Ok(None); // timeout
        }

        match port.read(&mut buf) {
            Ok(0) => {
                // no data (possible with non-blocking read)
                continue;
            }
            Ok(1) => {
                line.push(buf[0]);
                let s = String::from_utf8_lossy(&line);

                if s.ends_with("\r\n") {
                    let mut out = s.into_owned();
                    // trim trailing CRLF
                    while out.ends_with(['\r', '\n']) {
                        out.pop();
                    }
                    return Ok(Some(out));
                }
            }
            _ => {
                return Ok(None) // treat errors as timeout for now
            }
        }
    }
}
