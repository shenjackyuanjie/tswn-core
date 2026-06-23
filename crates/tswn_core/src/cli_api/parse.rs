use crate::engine::storage::Storage;
use crate::player::Player;
use crate::player::PlayerType;

use super::CliApiResult;

pub(super) fn export_player(raw: &str, old: bool, minions: bool) -> CliApiResult<String> {
    if old && minions {
        return Err(super::invalid_input("old and minions are mutually exclusive"));
    }
    let groups = parse_plus_separated_groups(raw);
    if groups.is_empty() {
        return Err(super::invalid_input("to_diy requires at least one player"));
    }
    groups
        .iter()
        .map(|group| export_group(group, old, minions))
        .collect::<CliApiResult<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

fn export_group(group: &[String], old: bool, minions: bool) -> CliApiResult<String> {
    match group {
        [] => Err(super::invalid_input("to_diy group is empty")),
        [raw] => export_single_player(raw, old, minions),
        _ => export_player_group(group, old, minions),
    }
}

fn export_single_player(raw: &str, old: bool, minions: bool) -> CliApiResult<String> {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| super::invalid_input(format!("failed to build player from {raw}: {err}")))?;
    player.build();
    Ok(export_built_player(&player, old, minions))
}

fn export_player_group(group: &[String], old: bool, minions: bool) -> CliApiResult<String> {
    let storage = Storage::new_arc();
    let mut ids = Vec::with_capacity(group.len());
    for raw in group {
        let player = Player::new_from_namerena_raw(raw.to_string(), storage.clone())
            .map_err(|err| super::invalid_input(format!("failed to build player from {raw}: {err}")))?;
        ids.push(storage.just_insert_player(player));
    }

    let mut local_plrs = ids
        .iter()
        .map(|id| storage.just_get_player_mut(*id).expect("player not found when exporting to_diy group"))
        .collect::<Vec<&mut Player>>();
    local_plrs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    for i in 0..local_plrs.len() {
        let (left, right) = local_plrs.split_at_mut(i + 1);
        let plr_p = &mut left[i];
        for plr_q in right.iter_mut() {
            if plr_p.clan_name() == plr_q.clan_name() {
                plr_p.upgrade(plr_q);
                plr_q.upgrade(plr_p);
            }
        }
    }

    let mut sorted_ids = ids.clone();
    sorted_ids.sort_by(|a, b| {
        let plr_a = storage.get_player(a).expect("player not found when sorting to_diy group");
        let plr_b = storage.get_player(b).expect("player not found when sorting to_diy group");
        plr_a.cmp_by_id_name(plr_b)
    });
    for id in sorted_ids {
        let player = storage.just_get_player_mut(id).expect("player not found when building to_diy group");
        player.build();
        if player.player_type() == PlayerType::Boss {
            crate::player::boss::init_boss_state(player);
        }
    }

    Ok(ids
        .iter()
        .map(|id| {
            let player = storage.get_player(id).expect("player not found when exporting to_diy group");
            export_built_player(player, old, minions)
        })
        .collect::<Vec<_>>()
        .join("+"))
}

fn export_built_player(player: &Player, old: bool, minions: bool) -> String {
    if old {
        player.to_diy_compact()
    } else if minions {
        player.to_ol_json_with_minions()
    } else {
        player.to_ol_json()
    }
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
    use super::{export_player, parse_plus_separated_groups};

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

    #[test]
    fn export_player_uses_plus_as_to_diy_group_separator() {
        let exported = export_player("1@a+2@a", true, false).unwrap();
        let groups = parse_plus_separated_groups(&exported);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert!(groups[0][0].starts_with("1@a+diy["));
        assert!(groups[0][1].starts_with("2@a+diy["));
    }

    #[test]
    fn export_player_keeps_overlay_suffix_with_player() {
        let raw = r#"1@a+diy[86,86,86,86,86,86,86,300]{"sklheal":"40+30"}+2@a"#;
        let exported = export_player(raw, true, false).unwrap();
        let groups = parse_plus_separated_groups(&exported);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert!(groups[0][0].starts_with("1@a+diy["));
        assert!(groups[0][1].starts_with("2@a+diy["));
    }
}
