use anyhow::Result;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::time::Duration;

use crate::cli::SerialOpts;

pub fn open_port(opts: &SerialOpts) -> Result<Box<dyn SerialPort>> {
    let builder = serialport::new(&opts.dev, opts.baud)
        .timeout(Duration::from_millis(100))
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(if opts.rtscts {
            FlowControl::Hardware
        } else {
            FlowControl::None
        });

    builder
        .open()
        .map_err(|e| anyhow::anyhow!("open {}: {}", opts.dev, e))
}
