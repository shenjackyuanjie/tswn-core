#[derive(Debug, Clone)]
pub struct InputRecord {
    pub raw_score: f64,
    pub raw_score_ptt: f64,
    pub name: String,
}

#[allow(dead_code)]
impl InputRecord {
    pub fn parse_scored_line(line: &str) -> Option<Self> {
        // C++ two_*.exe parser behavior:
        // 1) `cin >> score; getchar(); cin >> ptt; getchar();`
        // 2) then read the remainder of the line as raw name content.
        //
        // This intentionally preserves trailing '\r' (for CRLF lines) and
        // any extra spaces after the second numeric field.
        let bytes = line.as_bytes();
        let mut idx = 0usize;

        let raw_score = parse_number_token(line, bytes, &mut idx)?;
        if idx >= bytes.len() {
            return None;
        }
        idx += 1;

        let raw_score_ptt = parse_number_token(line, bytes, &mut idx)?;
        if idx >= bytes.len() {
            return None;
        }
        idx += 1;

        let name = line.get(idx..)?.to_string();
        if name.is_empty() {
            return None;
        }

        Some(Self {
            raw_score,
            raw_score_ptt,
            name,
        })
    }
}

fn parse_number_token(line: &str, bytes: &[u8], idx: &mut usize) -> Option<f64> {
    while *idx < bytes.len() && bytes[*idx].is_ascii_whitespace() {
        *idx += 1;
    }
    if *idx >= bytes.len() {
        return None;
    }
    let start = *idx;
    while *idx < bytes.len() && !bytes[*idx].is_ascii_whitespace() {
        *idx += 1;
    }
    if start == *idx {
        return None;
    }
    line.get(start..*idx)?.parse::<f64>().ok()
}
