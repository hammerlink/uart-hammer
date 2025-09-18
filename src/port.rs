use anyhow::{Result, bail};
use serialport::{DataBits, SerialPort};
use std::{
    io,
    sync::atomic::AtomicBool,
    thread::sleep,
    time::{Duration, Instant},
};

use crate::{
    cli::SerialOpts,
    proto::command::{FlowControl, Parity},
};

// Global flag
pub static PORT_DEBUG: AtomicBool = AtomicBool::new(false);

// Macro definition
#[macro_export]
macro_rules! debug_eprintln {
    ($($arg:tt)*) => {
        if $crate::port::PORT_DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!($($arg)*);
        }
    };
}

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
    port.set_timeout(Duration::from_millis(100))?;
    port.flush()?;
    port.clear(serialport::ClearBuffer::All)?;

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
    port.clear(serialport::ClearBuffer::All)?;
    sleep(Duration::from_millis(10)); // let settle
    debug_eprintln!(
        "[port] reconfigured to {} {}-{}-{}-{}",
        baud,
        bits,
        match parity {
            Parity::None => "N",
            Parity::Even => "E",
            Parity::Odd => "O",
        },
        1, // stop bits
        match flow {
            FlowControl::None => "",
            FlowControl::RtsCts => " +RTS/CTS",
        }
    );
    Ok(())
}

pub fn port_default_config(port: &mut dyn serialport::SerialPort) -> Result<()> {
    retune_for_config(port, 115_200, Parity::None, 8, FlowControl::None)
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
    debug_eprintln!("[port] write_line: {}", line.trim_end());
    port.write_all(line.as_bytes())?;
    port.flush()?;
    Ok(())
}

/// Read a CRLF-terminated line without changing the port's timeout.
///
/// Behavior:
/// - Ok(Some(line)) → a full line (CRLF trimmed) was read
/// - Ok(None)       → no full line available yet (WouldBlock, TimedOut, Ok(0))
/// - Err(e)         → unexpected I/O error
fn read_crlf_line(port: &mut dyn serialport::SerialPort) -> Result<Option<String>> {
    let mut buf = [0u8; 1];
    let mut line = Vec::new();

    loop {
        match port.read(&mut buf) {
            Ok(0) => {
                // No data: on some backends this can mean "nothing available right now".
                // Treat like a soft timeout for this attempt.
                return Ok(None);
            }
            Ok(1) => {
                line.push(buf[0]);

                // Fast-path CRLF check without allocating a String every byte
                let n = line.len();
                if n >= 2 && line[n - 2] == b'\r' && line[n - 1] == b'\n' {
                    // Trim trailing CRLF
                    line.truncate(n - 2);
                    // Lossy match original behavior (keeps you safe on bad utf8)
                    let out = String::from_utf8_lossy(&line).into_owned();
                    return Ok(Some(out));
                }

                // Keep reading until we hit CRLF or the OS times us out.
                continue;
            }
            Err(e) => {
                // We know `e` is an io::Error
                let kind = e.kind();
                match kind {
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                        return Ok(None);
                    }
                    io::ErrorKind::Interrupted => continue,
                    _ => return Err(e.into()), // promote to anyhow::Error
                }
            }
            _ => return Ok(None), // should not happen
        }
    }
}

/// Wait for a line that your matcher accepts, with an overall deadline.
/// - `timeout = Some(d)` → enforce total time limit across many reads
/// - `timeout = None`    → wait indefinitely
///
/// `matcher` examines each full line; return `Some(T)` to accept, `None` to keep waiting.
pub fn wait_for_command<T, F>(
    port: &mut dyn serialport::SerialPort,
    timeout: Option<Duration>,
    mut matcher: F,
) -> Result<T>
where
    F: FnMut(&str) -> Option<T>,
{
    let start = Instant::now();

    loop {
        if let Some(limit) = timeout
            && start.elapsed() >= limit
        {
            bail!("timed out after {:?}", limit);
        }

        // Try to read *one* line within the remaining window.
        match read_crlf_line(port)? {
            Some(line) => {
                if let Some(hit) = matcher(&line) {
                    debug_eprintln!("[port] matched line: {}", line);
                    return Ok(hit);
                }
                // else: keep looping until deadline.
            }
            None => {
                // This read attempt yielded nothing (timeout/WOULDBLOCK). Loop to retry
                // until the overall deadline trips.
            }
        }
    }
}
