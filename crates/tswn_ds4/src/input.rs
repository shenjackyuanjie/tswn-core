//! 带分数的输入记录。
//!
//! [`InputRecord`] 读取两列浮点分数与其后的名称，第二列是潜力分而非归一化分数。
//! 为兼容旧 C++ 输入，每列数字后只消耗一个分隔字节；名称额外前导空格与尾随 CR
//! 都会保留。缺数字、缺分隔符或空名称返回 None，浮点值不额外限制为有限数。

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct InputRecord {
    pub raw_score: f64,
    pub raw_score_ptt: f64,
    pub name: String,
}

#[allow(dead_code)]
impl InputRecord {
    pub fn parse_scored_line(line: &str) -> Option<Self> {
        // C++ two_*.exe 解析器行为：
        // 1) `cin >> score; getchar(); cin >> ptt; getchar();`
        // 2) 接着将本行剩余内容读取为原始名称内容。
        //
        // 这会有意保留尾随的 '\r'（用于 CRLF 行）和第二个数字字段后的任何额外空格。
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
