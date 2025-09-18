use crate::proto::command::{Direction, TestName};

pub struct TestConfig {
    pub name: TestName,
    pub frames: Option<u64>, // either frames or duration_ms must be Some
    pub duration_ms: Option<u64>,
    pub payload: usize,
    pub dir: Direction,
}
