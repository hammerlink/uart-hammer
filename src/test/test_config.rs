use crate::proto::command::{Direction, TestName};

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub name: TestName,
    pub frames: Option<u64>, // either frames or duration_ms must be Some
    pub duration_ms: Option<u64>,
    pub payload: usize, // bytes of payload per frame
    pub dir: Direction,
}
