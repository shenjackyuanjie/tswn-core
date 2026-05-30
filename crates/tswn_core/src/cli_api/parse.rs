use crate::engine::storage::Storage;
use crate::player::Player;

use super::CliApiResult;

pub(super) fn export_player(raw: &str, old: bool, minions: bool) -> CliApiResult<String> {
    if old && minions {
        return Err(super::invalid_input("old and minions are mutually exclusive"));
    }
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| super::invalid_input(format!("failed to build player from {raw}: {err}")))?;
    player.build();
    Ok(if old {
        player.to_diy_compact()
    } else if minions {
        player.to_ol_json_with_minions()
    } else {
        player.to_ol_json()
    })
}

pub(super) fn parse_plus_separated_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_plus_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

fn parse_plus_group_line(line: &str) -> Vec<String> {
    let mut group = Vec::new();
    let mut current = String::new();
    let mut idx = 0usize;

    while idx < line.len() {
        let rest = &line[idx..];
        if rest.starts_with('+') {
            let after_plus = &line[idx + 1..];
            if let Some(overlay_end) = overlay_suffix_end(after_plus) {
                current.push('+');
                current.push_str(&after_plus[..overlay_end]);
                idx += 1 + overlay_end;
                continue;
            }

            push_group_segment(&mut group, &mut current);
            idx += 1;
            continue;
        }

        let ch = rest.chars().next().expect("slice should contain a character");
        current.push(ch);
        idx += ch.len_utf8();
    }

    push_group_segment(&mut group, &mut current);
    group
}

fn overlay_suffix_end(rest: &str) -> Option<usize> {
    if rest.starts_with("ol:") {
        let mut idx = 3usize;
        skip_ascii_ws(rest, &mut idx);
        return consume_balanced_ascii(rest, idx, b'{', b'}');
    }
    if rest.starts_with("diy[") {
        let mut idx = consume_balanced_ascii(rest, 3, b'[', b']')?;
        skip_ascii_ws(rest, &mut idx);
        if rest.as_bytes().get(idx).copied() == Some(b'{') {
            idx = consume_balanced_ascii(rest, idx, b'{', b'}')?;
        }
        return Some(idx);
    }
    None
}

fn skip_ascii_ws(raw: &str, idx: &mut usize) {
    let bytes = raw.as_bytes();
    while *idx < bytes.len() && bytes[*idx].is_ascii_whitespace() {
        *idx += 1;
    }
}

fn consume_balanced_ascii(raw: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = raw.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut idx = start;

    while idx < bytes.len() {
        let byte = bytes[idx];
        if in_string {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(idx + 1);
                    }
                }
                _ => {}
            }
        }

        idx += 1;
    }

    None
}

fn push_group_segment(group: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        group.push(trimmed.to_string());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::parse_plus_separated_groups;

    #[test]
    fn parse_plus_groups_keeps_ol_overlay_with_whitespace() {
        let raw = r#"alice+ol: {"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklheal":"40+30"}}+bob"#;

        assert_eq!(
            parse_plus_separated_groups(raw),
            vec![vec![
                r#"alice+ol: {"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklheal":"40+30"}}"#.to_string(),
                "bob".to_string(),
            ]]
        );
    }

    #[test]
    fn parse_plus_groups_keeps_diy_overlay_with_trailing_json() {
        let raw = r#"alice+diy[86,86,86,86,86,86,86,300]{"sklheal":"40+30"}+bob"#;

        assert_eq!(
            parse_plus_separated_groups(raw),
            vec![vec![
                r#"alice+diy[86,86,86,86,86,86,86,300]{"sklheal":"40+30"}"#.to_string(),
                "bob".to_string(),
            ]]
        );
    }
}
