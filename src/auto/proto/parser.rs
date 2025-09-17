// src/proto/parser.rs
use std::{collections::BTreeMap, str::FromStr};

use thiserror::Error;

use super::command::{CtrlCommand, Direction, FlowControl, Parity, TestName, TestResultFlag};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("empty line")]
    Empty,
    #[error("missing tag")]
    MissingTag,
    #[error("malformed key=value pair: {0}")]
    BadPair(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("invalid integer for {0}: {1}")]
    BadInt(&'static str, String),
    #[error("invalid enum for {0}: {1}")]
    BadEnum(&'static str, String),
    #[error("unknown tag: {0}")]
    UnknownTag(String),
    #[error("semantic error: {0}")]
    Semantic(&'static str),
}

/// Public API: serialize a command to a CRLF-terminated line.
pub fn format_command(cmd: &CtrlCommand) -> String {
    use CtrlCommand::*;
    let mut out = String::new();

    macro_rules! push_pair {
        ($k:literal, $v:expr) => {{
            out.push(' ');
            out.push_str($k);
            out.push('=');
            out.push_str(&$v.to_string());
        }};
    }

    match cmd {
        // ---- Discovery
        Hello { id } => {
            out.push_str("HELLO");
            push_pair!("id", id);
        }
        Ack { id } => {
            out.push_str("ACK");
            push_pair!("id", id);
        }

        // ---- Config
        ConfigSet {
            id,
            baud,
            parity,
            bits,
            dir,
            flow,
        } => {
            out.push_str("CONFIG SET");
            push_pair!("id", id);
            push_pair!("baud", baud);
            push_pair!("parity", parity_to_str(*parity));
            push_pair!("bits", bits);
            push_pair!("dir", direction_to_str(*dir));
            push_pair!("flow", flow_to_str(*flow));
        }
        ConfigSetAck {
            id,
            baud,
            parity,
            bits,
            dir,
            flow,
        } => {
            out.push_str("CONFIG SET ACK");
            push_pair!("id", id);
            push_pair!("baud", baud);
            push_pair!("parity", parity_to_str(*parity));
            push_pair!("bits", bits);
            push_pair!("dir", direction_to_str(*dir));
            push_pair!("flow", flow_to_str(*flow));
        }

        // ---- Test orchestration
        TestBegin {
            id,
            name,
            frames,
            duration_ms,
            payload,
        } => {
            out.push_str("TEST BEGIN");
            push_pair!("id", id);
            push_pair!("name", testname_to_str(*name));
            if let Some(m) = frames {
                push_pair!("frames", m);
            }
            if let Some(t) = duration_ms {
                push_pair!("duration_ms", t);
            }
            push_pair!("payload", payload);
        }
        TestBeginAck {
            id,
            name,
            frames,
            duration_ms,
            payload,
        } => {
            out.push_str("TEST BEGIN ACK");
            push_pair!("id", id);
            push_pair!("name", testname_to_str(*name));
            if let Some(m) = frames {
                push_pair!("frames", m);
            }
            if let Some(t) = duration_ms {
                push_pair!("duration_ms", t);
            }
            push_pair!("payload", payload);
        }

        TestDone { id, result } => {
            out.push_str("TEST DONE");
            push_pair!("id", id);
            push_pair!("result", resultflag_to_str(*result));
        }
        TestDoneAck { id } => {
            out.push_str("TEST DONE ACK");
            push_pair!("id", id);
        }

        TestResult {
            id,
            result,
            rx_frames,
            rx_bytes,
            bad_crc,
            seq_gaps,
            overruns,
            errors,
            rate_bps,
            reason,
        } => {
            out.push_str("TEST RESULT");
            push_pair!("id", id);
            push_pair!("result", resultflag_to_str(*result));
            push_pair!("rx_frames", rx_frames);
            push_pair!("rx_bytes", rx_bytes);
            push_pair!("bad_crc", bad_crc);
            push_pair!("seq_gaps", seq_gaps);
            push_pair!("overruns", overruns);
            push_pair!("errors", errors);
            push_pair!("rate_bps", rate_bps);
            if let Some(r) = reason
                && !r.is_empty()
            {
                push_pair!("reason", escape_reason(r));
            }
        }

        // ---- Terminate
        Terminate { id } => {
            out.push_str("TERMINATE");
            push_pair!("id", id);
        }
        TerminateAck { id } => {
            out.push_str("TERMINATE ACK");
            push_pair!("id", id);
        }
    }

    out.push_str("\r\n");
    out
}

/// Public API: parse a CR/LF-terminated line into a command.
pub fn parse_command(line: &str) -> Result<CtrlCommand, ParseError> {
    let s = line.trim_matches(|c| c == '\r' || c == '\n' || c == ' ');
    if s.is_empty() {
        return Err(ParseError::Empty);
    }

    // Split into tokens, find first k=v; everything before is the tag (can be multi-word).
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ParseError::MissingTag);
    }

    let kv_start = tokens
        .iter()
        .position(|t| t.contains('='))
        .unwrap_or(tokens.len());
    let tag = tokens[..kv_start].join(" ");
    let mut map = BTreeMap::<String, String>::new();

    for &tok in &tokens[kv_start..] {
        let mut it = tok.splitn(2, '=');
        let k = it
            .next()
            .ok_or_else(|| ParseError::BadPair(tok.to_string()))?;
        let v = it
            .next()
            .ok_or_else(|| ParseError::BadPair(tok.to_string()))?;
        map.insert(k.to_string(), v.to_string());
    }

    use CtrlCommand::*;

    match tag.as_str() {
        // ---- Discovery
        "HELLO" => Ok(Hello {
            id: req_s(&map, "id")?.to_string(),
        }),
        "ACK" => Ok(Ack {
            id: req_s(&map, "id")?.to_string(),
        }),

        // ---- Config
        "CONFIG SET" => Ok(ConfigSet {
            id: req_s(&map, "id")?.to_string(),
            baud: req_u32(&map, "baud")?,
            parity: req_parity(&map, "parity")?,
            bits: req_u8(&map, "bits")?,
            dir: req_dir(&map, "dir")?,
            flow: req_flow(&map, "flow")?,
        }),
        "CONFIG SET ACK" => Ok(ConfigSetAck {
            id: req_s(&map, "id")?.to_string(),
            baud: req_u32(&map, "baud")?,
            parity: req_parity(&map, "parity")?,
            bits: req_u8(&map, "bits")?,
            dir: req_dir(&map, "dir")?,
            flow: req_flow(&map, "flow")?,
        }),

        // ---- Test orchestration
        "TEST BEGIN" => {
            let frames = opt_u64(&map, "frames")?;
            let duration_ms = opt_u64(&map, "duration_ms")?;
            if frames.is_none() && duration_ms.is_none() {
                return Err(ParseError::Semantic(
                    "TEST BEGIN requires frames or duration_ms",
                ));
            }
            Ok(TestBegin {
                id: req_s(&map, "id")?.to_string(),
                name: req_testname(&map, "name")?,
                frames,
                duration_ms,
                payload: req_usize(&map, "payload")?,
            })
        }
        "TEST BEGIN ACK" => {
            let frames = opt_u64(&map, "frames")?;
            let duration_ms = opt_u64(&map, "duration_ms")?;
            Ok(TestBeginAck {
                id: req_s(&map, "id")?.to_string(),
                name: req_testname(&map, "name")?,
                frames,
                duration_ms,
                payload: req_usize(&map, "payload")?,
            })
        }

        "TEST DONE" => Ok(TestDone {
            id: req_s(&map, "id")?.to_string(),
            result: req_resultflag(&map, "result")?,
        }),
        "TEST DONE ACK" => Ok(TestDoneAck {
            id: req_s(&map, "id")?.to_string(),
        }),

        "TEST RESULT" => Ok(TestResult {
            id: req_s(&map, "id")?.to_string(),
            result: req_resultflag(&map, "result")?,
            rx_frames: req_u64(&map, "rx_frames")?,
            rx_bytes: req_u64(&map, "rx_bytes")?,
            bad_crc: req_u64(&map, "bad_crc")?,
            seq_gaps: req_u64(&map, "seq_gaps")?,
            overruns: req_u64(&map, "overruns")?,
            errors: req_u32(&map, "errors")?,
            rate_bps: req_u64(&map, "rate_bps")?,
            reason: map
                .get("reason")
                .map(|s| unescape_reason(s))
                .filter(|s| !s.is_empty()),
        }),

        // ---- Terminate
        "TERMINATE" => Ok(Terminate {
            id: req_s(&map, "id")?.to_string(),
        }),
        "TERMINATE ACK" => Ok(TerminateAck {
            id: req_s(&map, "id")?.to_string(),
        }),

        _ => Err(ParseError::UnknownTag(tag)),
    }
}

/* ---------- helpers ---------- */

fn req_s<'a>(map: &'a BTreeMap<String, String>, k: &'static str) -> Result<&'a str, ParseError> {
    map.get(k)
        .map(|s| s.as_str())
        .ok_or(ParseError::MissingField(k))
}

fn req_u8(map: &BTreeMap<String, String>, k: &'static str) -> Result<u8, ParseError> {
    map.get(k).ok_or(ParseError::MissingField(k)).and_then(|v| {
        v.parse::<u8>()
            .map_err(|_| ParseError::BadInt(k, v.clone()))
    })
}
fn req_u32(map: &BTreeMap<String, String>, k: &'static str) -> Result<u32, ParseError> {
    map.get(k).ok_or(ParseError::MissingField(k)).and_then(|v| {
        v.parse::<u32>()
            .map_err(|_| ParseError::BadInt(k, v.clone()))
    })
}
fn req_u64(map: &BTreeMap<String, String>, k: &'static str) -> Result<u64, ParseError> {
    map.get(k).ok_or(ParseError::MissingField(k)).and_then(|v| {
        v.parse::<u64>()
            .map_err(|_| ParseError::BadInt(k, v.clone()))
    })
}
fn req_usize(map: &BTreeMap<String, String>, k: &'static str) -> Result<usize, ParseError> {
    map.get(k).ok_or(ParseError::MissingField(k)).and_then(|v| {
        v.parse::<usize>()
            .map_err(|_| ParseError::BadInt(k, v.clone()))
    })
}

fn opt_u64(map: &BTreeMap<String, String>, k: &'static str) -> Result<Option<u64>, ParseError> {
    Ok(match map.get(k) {
        None => None,
        Some(v) => Some(
            v.parse::<u64>()
                .map_err(|_| ParseError::BadInt(k, v.clone()))?,
        ),
    })
}

fn req_parity(map: &BTreeMap<String, String>, k: &'static str) -> Result<Parity, ParseError> {
    map.get(k)
        .ok_or(ParseError::MissingField(k))
        .and_then(|v| Parity::from_str(v).map_err(|_| ParseError::BadEnum(k, v.clone())))
}
fn req_dir(map: &BTreeMap<String, String>, k: &'static str) -> Result<Direction, ParseError> {
    map.get(k)
        .ok_or(ParseError::MissingField(k))
        .and_then(|v| Direction::from_str(v).map_err(|_| ParseError::BadEnum(k, v.clone())))
}
fn req_flow(map: &BTreeMap<String, String>, k: &'static str) -> Result<FlowControl, ParseError> {
    map.get(k)
        .ok_or(ParseError::MissingField(k))
        .and_then(|v| FlowControl::from_str(v).map_err(|_| ParseError::BadEnum(k, v.clone())))
}
fn req_testname(map: &BTreeMap<String, String>, k: &'static str) -> Result<TestName, ParseError> {
    map.get(k)
        .ok_or(ParseError::MissingField(k))
        .and_then(|v| TestName::from_str(v).map_err(|_| ParseError::BadEnum(k, v.clone())))
}
fn req_resultflag(
    map: &BTreeMap<String, String>,
    k: &'static str,
) -> Result<TestResultFlag, ParseError> {
    map.get(k)
        .ok_or(ParseError::MissingField(k))
        .and_then(|v| TestResultFlag::from_str(v).map_err(|_| ParseError::BadEnum(k, v.clone())))
}

/* ---------- enum string helpers & FromStr impls ---------- */

fn parity_to_str(p: Parity) -> &'static str {
    match p {
        Parity::None => "none",
        Parity::Even => "even",
        Parity::Odd => "odd",
    }
}
fn direction_to_str(d: Direction) -> &'static str {
    match d {
        Direction::Tx => "tx",
        Direction::Rx => "rx",
        Direction::Both => "both",
    }
}
fn flow_to_str(f: FlowControl) -> &'static str {
    match f {
        FlowControl::None => "none",
        FlowControl::RtsCts => "rtscts",
    }
}
fn testname_to_str(t: TestName) -> &'static str {
    match t {
        TestName::MaxRate => "max-rate",
        TestName::FifoResidue => "fifo-residue",
    }
}
fn resultflag_to_str(r: TestResultFlag) -> &'static str {
    match r {
        TestResultFlag::Pass => "pass",
        TestResultFlag::Fail => "fail",
    }
}

// Allow simple FromStr for enums.

impl FromStr for Parity {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Parity::None),
            "even" => Ok(Parity::Even),
            "odd" => Ok(Parity::Odd),
            _ => Err(()),
        }
    }
}
impl FromStr for Direction {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "tx" => Ok(Direction::Tx),
            "rx" => Ok(Direction::Rx),
            "both" => Ok(Direction::Both),
            _ => Err(()),
        }
    }
}
impl FromStr for FlowControl {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(FlowControl::None),
            "rtscts" => Ok(FlowControl::RtsCts),
            _ => Err(()),
        }
    }
}
impl FromStr for TestName {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "max-rate" => Ok(TestName::MaxRate),
            "fifo-residue" => Ok(TestName::FifoResidue),
            _ => Err(()),
        }
    }
}
impl FromStr for TestResultFlag {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "pass" => Ok(TestResultFlag::Pass),
            "fail" => Ok(TestResultFlag::Fail),
            _ => Err(()),
        }
    }
}

/* ---------- value escaping for reason ---------- */

fn escape_reason(s: &str) -> String {
    // Key=value format can’t contain spaces, so replace spaces with underscores
    // and escape CR/LF. You can later swap to JSON if you want richer reasons.
    s.replace(' ', "_")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}
fn unescape_reason(s: &str) -> String {
    s.replace("\\r", "\r")
        .replace("\\n", "\n")
        .replace('_', " ")
}

/* ---------- tests ---------- */

#[cfg(test)]
mod tests {
    use super::super::command::*;
    use super::*;

    #[test]
    fn roundtrip_config_set() {
        let cmd = CtrlCommand::ConfigSet {
            id: "m1".into(),
            baud: 115200,
            parity: Parity::None,
            bits: 8,
            dir: Direction::Both,
            flow: FlowControl::None,
        };
        let line = format_command(&cmd);
        assert!(line.ends_with("\r\n"));
        let parsed = parse_command(&line).unwrap();
        match parsed {
            CtrlCommand::ConfigSet {
                id,
                baud,
                parity,
                bits,
                dir,
                flow,
            } => {
                assert_eq!(id, "m1");
                assert_eq!(baud, 115200);
                assert!(matches!(parity, Parity::None));
                assert_eq!(bits, 8);
                assert!(matches!(dir, Direction::Both));
                assert!(matches!(flow, FlowControl::None));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_test_begin_frames() {
        let line = "TEST BEGIN id=aa name=max-rate frames=100 payload=128\r\n";
        let cmd = parse_command(line).unwrap();
        match cmd {
            CtrlCommand::TestBegin {
                id,
                name,
                frames,
                duration_ms,
                payload,
            } => {
                assert_eq!(id, "aa");
                assert!(matches!(name, TestName::MaxRate));
                assert_eq!(frames, Some(100));
                assert_eq!(duration_ms, None);
                assert_eq!(payload, 128);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_test_result() {
        let line = "TEST RESULT id=s1 result=fail rx_frames=99 rx_bytes=1000 bad_crc=1 seq_gaps=0 overruns=0 errors=0 rate_bps=123456 reason=timeout\r\n";
        let cmd = parse_command(line).unwrap();
        match cmd {
            CtrlCommand::TestResult {
                id,
                result,
                rx_frames,
                bad_crc,
                reason,
                ..
            } => {
                assert_eq!(id, "s1");
                assert!(matches!(result, TestResultFlag::Fail));
                assert_eq!(rx_frames, 99);
                assert_eq!(bad_crc, 1);
                assert_eq!(reason.unwrap(), "timeout");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_hello() {
        let cmd = CtrlCommand::Hello {
            id: "device1".into(),
        };
        let line = format_command(&cmd);
        let parsed = parse_command(&line).unwrap();
        match parsed {
            CtrlCommand::Hello { id } => {
                assert_eq!(id, "device1");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_ack() {
        let cmd = CtrlCommand::Ack { id: "host2".into() };
        let line = format_command(&cmd);
        let parsed = parse_command(&line).unwrap();
        match parsed {
            CtrlCommand::Ack { id } => {
                assert_eq!(id, "host2");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_test_begin_duration() {
        let line = "TEST BEGIN id=bb name=fifo-residue duration_ms=5000 payload=64\r\n";
        let cmd = parse_command(line).unwrap();
        match cmd {
            CtrlCommand::TestBegin {
                id,
                name,
                frames,
                duration_ms,
                payload,
            } => {
                assert_eq!(id, "bb");
                assert!(matches!(name, TestName::FifoResidue));
                assert_eq!(frames, None);
                assert_eq!(duration_ms, Some(5000));
                assert_eq!(payload, 64);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_test_done() {
        let cmd = CtrlCommand::TestDone {
            id: "test3".into(),
            result: TestResultFlag::Pass,
        };
        let line = format_command(&cmd);
        let parsed = parse_command(&line).unwrap();
        match parsed {
            CtrlCommand::TestDone { id, result } => {
                assert_eq!(id, "test3");
                assert!(matches!(result, TestResultFlag::Pass));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_terminate() {
        let cmd = CtrlCommand::Terminate { id: "sess4".into() };
        let line = format_command(&cmd);
        let parsed = parse_command(&line).unwrap();
        match parsed {
            CtrlCommand::Terminate { id } => {
                assert_eq!(id, "sess4");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_reason_escaping() {
        let original = "Error with\r\nnewlines and spaces";
        let escaped = escape_reason(original);
        assert_eq!(escaped, "Error_with\\r\\nnewlines_and_spaces");
        let unescaped = unescape_reason(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn test_error_cases() {
        // Empty line
        assert!(matches!(parse_command(""), Err(ParseError::Empty)));

        // Unknown command
        assert!(matches!(
            parse_command("UNKNOWN id=123"),
            Err(ParseError::UnknownTag(_))
        ));

        // Missing required field
        assert!(matches!(
            parse_command("HELLO"),
            Err(ParseError::MissingField(_))
        ));

        // Bad integer
        assert!(matches!(
            parse_command("CONFIG SET id=x1 baud=invalid parity=none bits=8 dir=both flow=none"),
            Err(ParseError::BadInt(_, _))
        ));

        // Bad enum
        assert!(matches!(
            parse_command("CONFIG SET id=x1 baud=9600 parity=invalid bits=8 dir=both flow=none"),
            Err(ParseError::BadEnum(_, _))
        ));

        // Test BEGIN without frames or duration
        assert!(matches!(
            parse_command("TEST BEGIN id=x1 name=max-rate payload=128"),
            Err(ParseError::Semantic(_))
        ));
    }
}
