use anyhow::Result;
use clap::Parser;

mod cli;
mod rx;
mod tx;
mod port;
mod frame;
mod stats;
mod auto;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    match args.cmd {
        cli::Cmd::Rx(opts) => rx::run(opts),
        cli::Cmd::Tx(opts) => tx::run(opts),
        cli::Cmd::Auto(_opts) => std::result::Result::Err(anyhow::anyhow!("auto mode not implemented yet")),
    }
}
