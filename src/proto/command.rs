#[derive(Debug, Clone)]
pub enum CtrlCommand {
    // ---- Discovery ----
    Hello {
        id: String,
    },
    Ack {
        id: String,
    },

    // ---- Config ----
    ConfigSet {
        id: String,
        baud: u32,
        parity: Parity,
        bits: u8,
        flow: FlowControl,
    },
    ConfigSetAck {
        id: String,
        baud: u32,
        parity: Parity,
        bits: u8,
        flow: FlowControl,
    },

    // ---- Test orchestration ----
    TestBegin {
        id: String,
        name: TestName,
        frames: Option<u64>, // either frames or duration_ms must be Some
        duration_ms: Option<u64>,
        payload: usize,
        dir: Direction,
    },
    TestBeginAck {
        id: String,
        name: TestName,
        frames: Option<u64>,
        duration_ms: Option<u64>,
        payload: usize,
        dir: Direction,
    },

    TestDone {
        id: String,
    },
    TestDoneAck {
        id: String,
        ok: u64,
        bad: u64,
        lost: u64,
        total: u64,
        duration_micros: u64,
        bytes: u64, // Bytes sent / received TODO
    },

    TestResult {
        id: String,
        result: TestResultFlag,
        rx_frames: u64,
        rx_bytes: u64,
        bad_crc: u64,
        seq_gaps: u64,
        overruns: u64,
        errors: u32, // bitmask
        rate_bps: u64,
        reason: Option<String>,
    },

    // ---- Terminate ----
    Terminate {
        id: String,
    },
    TerminateAck {
        id: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum TestName {
    MaxRate,
    FifoResidue,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Tx,
    Rx,
    Both,
}

#[derive(Debug, Clone, Copy)]
pub enum Parity {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy)]
pub enum FlowControl {
    None,
    RtsCts,
}

#[derive(Debug, Clone, Copy)]
pub enum TestResultFlag {
    Pass,
    Fail,
}
