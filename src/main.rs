use anyhow::Result;
use clap::Parser;

mod cli;
mod rx;
mod tx;
mod port;
mod frame;
mod stats;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    match args.cmd {
        cli::Cmd::Rx(opts) => rx::run(opts),
        cli::Cmd::Tx(opts) => tx::run(opts),
    }
}
