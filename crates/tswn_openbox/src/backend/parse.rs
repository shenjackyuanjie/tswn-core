//! 输入文本解析工具。
//!
//! 将多行文本解析为玩家列表（`parse_line_list`）、加号分组（`parse_plus_separated_groups`）、
//! 带标签的玩家分组（`parse_player_groups_with_labels`）等格式，供各工具后端调用。

use std::collections::HashSet;

use serde::Deserialize;
use tswn_core::player::{Player, overlay::PlayerOverlay};

pub fn parse_line_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn parse_plus_separated_groups(content: &str) -> Vec<String> { parse_separated_groups(content, "+") }

pub fn parse_target_groups(content: &str, double_plus: bool) -> Vec<String> {
    let separator = if double_plus { "++" } else { "+" };
    parse_separated_groups(content, separator)
}

#[derive(Debug, Deserialize)]
struct FactoredTargetFile {
    targets: Vec<FactoredTarget>,
}

#[derive(Debug, Deserialize)]
struct FactoredTarget {
    factor: f64,
    players: Vec<String>,
}

pub fn parse_factored_target_groups(content: &str) -> Result<(Vec<String>, Vec<f64>), String> {
    let parsed: FactoredTargetFile = toml::from_str(content).map_err(|err| format!("解析带权靶子 TOML 失败: {err}"))?;
    if parsed.targets.is_empty() {
        return Err("带权靶子 TOML 中的 targets 不能为空。".to_string());
    }

    let mut groups = Vec::with_capacity(parsed.targets.len());
    let mut factors = Vec::with_capacity(parsed.targets.len());
    for (index, target) in parsed.targets.into_iter().enumerate() {
        if !target.factor.is_finite() || target.factor <= 0.0 {
            return Err(format!("带权靶子 targets[{}].factor 必须是有限正数。", index));
        }
        let players = target.players.into_iter().map(|player| player.trim().to_string()).collect::<Vec<_>>();
        if players.is_empty() || players.iter().any(String::is_empty) {
            return Err(format!("带权靶子 targets[{}].players 不能为空或包含空名字。", index));
        }
        groups.push(players.join("\n"));
        factors.push(target.factor);
    }
    Ok((groups, factors))
}

fn parse_separated_groups(content: &str, separator: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split(separator).map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

pub fn parse_player_groups_with_labels(content: &str, double_plus: bool) -> (Vec<String>, Vec<String>) {
    let mut groups = Vec::new();
    let mut labels = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        labels.push(line.to_string());
        let separator = if double_plus { "++" } else { "+" };
        groups.push(line.split(separator).map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    (groups, labels)
}

pub fn parse_namer_pf_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_namer_pf_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

pub fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
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

pub fn groups_have_same_players(left: &str, right: &str) -> bool {
    let mut left = normalized_group_players(left);
    let mut right = normalized_group_players(right);
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

fn normalized_group_players(group: &str) -> Vec<String> {
    group
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(Player::raw_namerena_to_idname)
        .collect()
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
    use super::{
        groups_have_same_players, parse_factored_target_groups, parse_namer_pf_groups, parse_player_groups_with_labels,
        parse_target_groups,
    };

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

    #[test]
    fn double_plus_target_list_preserves_overlay_plus() {
        let groups = parse_target_groups("mario++ol:{\"skills\":\"40+30\"}", true);
        assert_eq!(groups, vec!["mario\nol:{\"skills\":\"40+30\"}".to_string()]);
    }

    #[test]
    fn normal_target_list_still_splits_single_plus() {
        let groups = parse_target_groups("mario+luigi", false);
        assert_eq!(groups, vec!["mario\nluigi".to_string()]);
    }

    #[test]
    fn parses_factored_target_toml() {
        let raw = r#"
            [[targets]]
            factor = 1.5
            players = ["mario", "luigi"]

            [[targets]]
            factor = 0.25
            players = ["peach", "fire"]
        "#;
        let (groups, factors) = parse_factored_target_groups(raw).expect("valid factored targets");
        assert_eq!(groups, vec!["mario\nluigi", "peach\nfire"]);
        assert_eq!(factors, vec![1.5, 0.25]);
    }

    #[test]
    fn parses_embedded_factored_target_file() {
        let raw = include_str!("../../assets/targets/newTarget2.toml");
        let (groups, factors) = parse_factored_target_groups(raw).expect("embedded factored targets are valid");
        assert_eq!(groups.len(), 50);
        assert_eq!(factors.len(), 50);
        assert!(groups.iter().all(|group| group.lines().count() == 2));
    }

    #[test]
    fn rejects_non_positive_target_factor() {
        let raw = "[[targets]]\nfactor = 0\nplayers = [\"mario\", \"luigi\"]";
        assert!(parse_factored_target_groups(raw).is_err());
    }

    #[test]
    fn same_players_ignores_team_order() {
        assert!(groups_have_same_players("mario\nluigi", "luigi\nmario"));
        assert!(!groups_have_same_players("mario\nluigi", "mario\npeach"));
    }
}
