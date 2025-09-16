use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Stats {
    pub ok: u64,
    pub bad: u64,
    pub lost: u64,
    pub bytes: u64,
    pub bpb: u32,
    t0: Instant,
    last: Instant,
}

impl Stats {
    pub fn new(bpb: u32) -> Self {
        Self {
            ok: 0,
            bad: 0,
            lost: 0,
            bytes: 0,
            bpb,
            t0: Instant::now(),
            last: Instant::now(),
        }
    }
    pub fn add_bytes(&mut self, n: usize) {
        self.bytes += n as u64;
    }
    pub fn inc_ok(&mut self) {
        self.ok += 1;
    }
    pub fn inc_bad(&mut self) {
        self.bad += 1;
    }
    pub fn add_lost(&mut self, n: u64) {
        self.lost += n;
    }

    pub fn maybe_print(&mut self, stats_int: f64) {
        if self.last.elapsed().as_secs_f64() >= stats_int {
            let dur = self.t0.elapsed().as_secs_f64().max(1e-3);
            let bps_bytes = (self.bytes as f64) / dur;
            let bps_bits = bps_bytes * (self.bpb as f64);
            eprintln!(
                "[rx] ok={} bad={} lost={} bytes={} over {:.1}s => {:.1}kB/s (~{:.0} bps, bpb={})",
                self.ok,
                self.bad,
                self.lost,
                self.bytes,
                dur,
                bps_bytes / 1000.0,
                bps_bits,
                self.bpb
            );
            self.last = Instant::now();
            self.t0 = Instant::now();
            self.bytes = 0;
        }
    }
}
