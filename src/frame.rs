use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Frame {
    pub seq: u64,
    pub len: usize,
    pub pay_hex: String,
    pub sum: u8,
}

pub fn hexsum(payload_hex: &str) -> Result<u8> {
    if payload_hex.len() % 2 != 0 {
        bail!("odd hex length");
    }
    let mut sum: u8 = 0;
    for i in (0..payload_hex.len()).step_by(2) {
        let b = u8::from_str_radix(&payload_hex[i..i + 2], 16).context("bad hex in PAY")?;
        sum = sum.wrapping_add(b);
    }
    Ok(sum)
}

pub fn parse_frame(line: &str) -> Result<Frame> {
    // tolerate leading/trailing markers and flexible order
    let mut seq = None;
    let mut len = None;
    let mut pay = None;
    let mut sum = None;
    for tok in line.split_whitespace() {
        if let Some(v) = tok.strip_prefix("SEQ=") {
            seq = Some(v.parse::<u64>()?)
        } else if let Some(v) = tok.strip_prefix("LEN=") {
            len = Some(v.parse::<usize>()?)
        } else if let Some(v) = tok.strip_prefix("PAY=") {
            pay = Some(v.to_string())
        } else if let Some(v) = tok.strip_prefix("SUM=") {
            sum = Some(u8::from_str_radix(v, 16)?)
        }
    }
    let (seq, len, pay, sumrx) = (
        seq.ok_or_else(|| anyhow::anyhow!("no SEQ"))?,
        len.ok_or_else(|| anyhow::anyhow!("no LEN"))?,
        pay.ok_or_else(|| anyhow::anyhow!("no PAY"))?,
        sum.ok_or_else(|| anyhow::anyhow!("no SUM"))?,
    );
    if pay.len() != len * 2 {
        bail!("len mismatch");
    }
    let calc = hexsum(&pay)?;
    if calc != sumrx {
        bail!("checksum {}!={}", calc, sumrx);
    }
    Ok(Frame {
        seq,
        len,
        pay_hex: pay,
        sum: sumrx,
    })
}

pub fn build_frame(seq: u64, len: usize) -> String {
    // PAY = (i+seq) % 256 pattern
    let mut sum: u8 = 0;
    let mut s = String::with_capacity(2 * len);
    for i in 0..len {
        let b = ((i as u64 + seq) & 0xFF) as u8;
        sum = sum.wrapping_add(b);
        use std::fmt::Write;
        let _ = write!(s, "{:02X}", b);
    }
    format!("@@ SEQ={} LEN={} PAY={} SUM={:02X} ##", seq, len, s, sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let f = build_frame(42, 8);
        let p = parse_frame(&f).unwrap();
        assert_eq!(p.seq, 42);
        assert_eq!(p.len, 8);
        assert_eq!(hexsum(&p.pay_hex).unwrap(), p.sum);
    }
}
