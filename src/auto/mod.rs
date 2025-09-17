use crate::cli::AutoOpts;
use anyhow::Result;

pub mod dataplane;
pub mod master;
pub mod proto;
pub mod slave;

pub fn run(opts: AutoOpts) -> Result<()> {
    if opts.slave {
        return slave::run(slave::SlaveArgs {
            dev: opts.dev,
            hello_ms: opts.hello_ms,
            hello_backoff_max_ms: opts.hello_backoff_max_ms,
            repeat_timeout_ms: opts.repeat_timeout_ms,
        });
    }
    loop {}
}
