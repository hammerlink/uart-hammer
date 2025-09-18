use anyhow::Result;
use clap::Parser;

mod cli;
mod rx;
mod tx;
mod port;
mod frame;
mod stats;
mod auto;
mod proto;
mod test;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    match args.cmd {
        cli::Cmd::Rx(opts) => rx::run(opts),
        cli::Cmd::Tx(opts) => tx::run(opts),
        cli::Cmd::Auto(opts) => auto::run(opts),
        cli::Cmd::Test(opts) => test::run(opts),
    }
}
