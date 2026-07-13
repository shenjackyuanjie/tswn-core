use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn json_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(&mut out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

pub fn build_failed_diff(ts_output: &str, rust_output: &str) -> String {
    let ts_lines = split_output_lines(ts_output);
    let rust_lines = split_output_lines(rust_output);
    let mismatch_idx = first_mismatch_idx(&ts_lines, &rust_lines);
    let start = mismatch_idx.saturating_sub(3);
    let end = (mismatch_idx + 4).max(start + 1);

    let mut out = String::new();
    let _ = writeln!(&mut out, "--- expected(ts)");
    let _ = writeln!(&mut out, "+++ actual(rust)");
    let _ = writeln!(&mut out, "@@ mismatch_idx={mismatch_idx} @@");

    for idx in start..end {
        let ts_line = ts_lines.get(idx);
        let rust_line = rust_lines.get(idx);
        match (ts_line, rust_line) {
            (Some(left), Some(right)) if left == right => {
                let _ = writeln!(&mut out, " {:>4} {}", idx, left);
            }
            (Some(left), Some(right)) => {
                let _ = writeln!(&mut out, "-{:>4} {}", idx, left);
                let _ = writeln!(&mut out, "+{:>4} {}", idx, right);
            }
            (Some(left), None) => {
                let _ = writeln!(&mut out, "-{:>4} {}", idx, left);
                let _ = writeln!(&mut out, "+{:>4} <EOF>", idx);
            }
            (None, Some(right)) => {
                let _ = writeln!(&mut out, "-{:>4} <EOF>", idx);
                let _ = writeln!(&mut out, "+{:>4} {}", idx, right);
            }
            (None, None) => break,
        }
    }
    out
}

pub fn json_string_array(values: &[String]) -> String {
    let items = values.iter().map(|value| format!("\"{}\"", json_escape(value))).collect::<Vec<_>>();
    format!("[{}]", items.join(", "))
}

pub fn json_btreemap(values: &BTreeMap<String, usize>) -> String {
    let items = values
        .iter()
        .map(|(key, value)| format!("\"{}\": {value}", json_escape(key)))
        .collect::<Vec<_>>();
    format!("{{{}}}", items.join(", "))
}

fn split_output_lines(output: &str) -> Vec<String> {
    if output.is_empty() {
        Vec::new()
    } else {
        output.split('\n').map(str::to_owned).collect()
    }
}

fn first_mismatch_idx(left: &[String], right: &[String]) -> usize {
    let min_len = left.len().min(right.len());
    left.iter().zip(right).position(|(left, right)| left != right).unwrap_or(min_len)
}
