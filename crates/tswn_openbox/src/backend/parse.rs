use std::collections::HashSet;

use tswn_core::player::{Player, overlay::PlayerOverlay};

pub(crate) fn parse_line_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn parse_plus_separated_groups(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('+').map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

pub(crate) fn parse_player_groups_with_labels(content: &str, double_plus: bool) -> (Vec<String>, Vec<String>) {
    let mut groups = Vec::new();
    let mut labels = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        labels.push(line.to_string());
        let separator = if double_plus { "++" } else { "+" };
        groups.push(line.split(separator).map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    (groups, labels)
}

pub(crate) fn parse_namer_pf_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_namer_pf_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

pub(crate) fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
    let mut seen = HashSet::new();
    for group in groups {
        for name in group.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let id_name = Player::raw_namerena_to_idname(name);
            if !seen.insert(id_name.clone()) {
                return Some(id_name);
            }
        }
    }
    None
}

fn parse_namer_pf_group_line(line: &str) -> Vec<String> {
    let mut group: Vec<String> = Vec::new();
    for segment in split_plus_outside_quotes(line) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if PlayerOverlay::parse_inline(segment).is_some()
            && let Some(previous) = group.last_mut()
        {
            previous.push('+');
            previous.push_str(segment);
            continue;
        }
        group.push(segment.to_string());
    }
    group
}

fn split_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in raw.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if ch == '+' {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
            if ch == '"' {
                in_string = true;
            }
        }
    }

    segments.push(current);
    segments
}

#[cfg(test)]
mod tests {
    use super::{parse_namer_pf_groups, parse_player_groups_with_labels};

    #[test]
    fn namer_pf_keeps_overlay_suffix() {
        let raw = "mario+ol:{\"attrs\":[58,87,82,78,89,93,99,343],\"skills\":{\"skldefend\":13,\"sklheal\":\"40+30\"},\"name_factor_enabled\":true}+fire";
        let groups = parse_namer_pf_groups(raw);
        assert_eq!(groups, vec![vec![raw[..raw.len() - 5].to_string(), "fire".to_string()]]);
    }

    #[test]
    fn double_plus_player_list_preserves_overlay_plus() {
        let (groups, labels) = parse_player_groups_with_labels("mario++ol:{\"skills\":\"40+30\"}", true);
        assert_eq!(labels, vec!["mario++ol:{\"skills\":\"40+30\"}".to_string()]);
        assert_eq!(groups, vec!["mario\nol:{\"skills\":\"40+30\"}".to_string()]);
    }
}
