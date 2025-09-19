use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Stats {
    pub ok: u64,
    pub bad: u64,
    pub lost: u64,
    pub total: u64,
    pub bytes: u64,
    pub bpb: u32,
    pub duration_micros: u64,
}

impl Stats {
    pub fn new(bpb: u32) -> Self {
        Self {
            ok: 0,
            bad: 0,
            lost: 0,
            total: 0,
            bytes: 0,
            bpb,
            duration_micros: 0,
        }
    }
    pub fn add_bytes(&mut self, n: usize) {
        self.bytes += n as u64;
    }
    pub fn inc_ok(&mut self) {
        self.ok += 1;
        self.total += 1;
    }
    pub fn inc_bad(&mut self) {
        self.bad += 1;
        self.total += 1;
    }
    pub fn add_lost(&mut self, n: u64) {
        self.lost += n;
        self.total += n;
    }

    pub fn maybe_print(&mut self, stats_int: f64) {
        let dur = Duration::from_micros(self.duration_micros)
            .as_secs_f64()
            .max(1e-3);
        if dur >= stats_int {
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
            self.bytes = 0;
        }
    }
}
