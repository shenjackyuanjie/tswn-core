use std::collections::HashSet;

use anyhow::{anyhow, bail};

#[derive(Debug, Clone)]
pub struct ParsedGroup {
    pub canonical: String,
    pub display_raw: String,
    pub members: Vec<String>,
    pub lane_size: usize,
    pub team_name: String,
}

pub fn parse_group(raw: &str) -> anyhow::Result<ParsedGroup> {
    let display_raw = raw.trim().to_string();
    if display_raw.is_empty() {
        bail!("empty group");
    }

    let mut members: Vec<String> = display_raw
        .split('+')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if members.is_empty() {
        bail!("empty group");
    }

    let mut seen = HashSet::new();
    for member in &members {
        if !seen.insert(member.clone()) {
            bail!("duplicated member in the same group: {member}");
        }
    }

    let mut teams = Vec::with_capacity(members.len());
    for member in &members {
        let (_, team) = parse_member_team(member).ok_or_else(|| anyhow!("member has no team suffix: {member}"))?;
        teams.push(team.to_string());
    }

    let first_team = teams[0].clone();
    // 注意：这里必须检查“原始战队名完全一致”。
    // 即使 A 和 B 后来被 merge，aaa@A+bbb@B 仍然非法。
    if teams.iter().any(|team| team != &first_team) {
        bail!("members belong to different raw teams");
    }

    // 顺序等价：A+B 与 B+A 视为同一个组号。
    members.sort();

    let canonical = members.join("+");
    let lane_size = members.len();

    Ok(ParsedGroup {
        canonical,
        display_raw,
        members,
        lane_size,
        team_name: first_team,
    })
}

pub fn parse_member_team(member: &str) -> Option<(&str, &str)> {
    let (name, team) = member.rsplit_once('@')?;
    if name.is_empty() || team.is_empty() {
        return None;
    }
    Some((name, team))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_equivalent() {
        let a = parse_group("b@T+a@T").unwrap();
        assert_eq!(a.canonical, "a@T+b@T");
    }

    #[test]
    fn different_raw_teams_are_invalid() {
        assert!(parse_group("a@A+b@B").is_err());
    }
}
