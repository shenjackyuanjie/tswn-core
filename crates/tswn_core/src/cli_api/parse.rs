use crate::namerena::{
    BuiltinSkillRef, MinionKind, NamerenaInput, PreparedMinionBlueprint, PreparedPlayer, PreparedRoster, SkillBoost,
    SkillEntrySpec, SkillLoadoutSpec, classified_player_skill_name_for_export, classified_summon_minion_skill_name_for_export,
    skill_name_for_export,
};

use super::CliApiResult;

pub(super) fn export_player(raw: &str, old: bool, minions: bool) -> CliApiResult<String> {
    if old && minions {
        return Err(super::invalid_input("old and minions are mutually exclusive"));
    }
    let groups = parse_to_diy_groups(raw);
    if groups.is_empty() {
        return Err(super::invalid_input("to_diy requires at least one player"));
    }
    groups
        .iter()
        .map(|group| export_group(group, old, minions))
        .collect::<CliApiResult<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

fn parse_to_diy_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let parsed = parse_plus_group_line(line);
            let explicit_double_plus = has_double_plus_outside_quotes(line);
            let unambiguous_teamed_group = parsed.len() > 1 && parsed.iter().all(|player| player_identity_has_team(player));
            if explicit_double_plus || unambiguous_teamed_group {
                parsed
            } else {
                vec![line.to_owned()]
            }
        })
        .collect()
}

fn player_identity_has_team(raw: &str) -> bool { raw.split_once('+').map_or(raw, |(identity, _)| identity).contains('@') }

fn has_double_plus_outside_quotes(raw: &str) -> bool {
    let mut chars = raw.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == '+' && chars.peek() == Some(&'+') {
            return true;
        }
    }
    false
}

fn export_group(group: &[String], old: bool, minions: bool) -> CliApiResult<String> {
    if group.is_empty() {
        return Err(super::invalid_input("to_diy group is empty"));
    }
    let input = NamerenaInput::from_raw_groups(&[group.to_vec()])
        .map_err(|error| super::invalid_input(format!("failed to parse player group: {error}")))?;
    let roster = match PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    Ok(roster
        .players
        .iter()
        .map(|player| export_built_player(player, old, minions))
        .collect::<Vec<_>>()
        .join("+"))
}

fn export_built_player(player: &PreparedPlayer, old: bool, minions: bool) -> String {
    if old {
        return format!(
            "{}{}+diy[{}]{}",
            player.name,
            team_name_for_export(player),
            attrs_to_overlay_json(player.attrs),
            skills_to_json(&player.skills, SkillExportContext::Player)
        );
    }

    let mut fields = vec![
        format!("\"attrs\":[{}]", attrs_to_overlay_json(player.attrs)),
        format!("\"skills\":{}", skills_to_json(&player.skills, SkillExportContext::Player)),
        format!(
            "\"name_factor_enabled\":{}",
            player.overlay.as_ref().is_none_or(|overlay| overlay.name_factor_enabled)
        ),
    ];
    if minions {
        append_minion_export(player, MinionKind::Shadow, "shadow", &mut fields);
        append_minion_export(player, MinionKind::Summon, "summon", &mut fields);
        append_minion_export(player, MinionKind::Zombie, "zombie", &mut fields);
    }
    format!("{}{}+ol:{{{}}}", player.name, team_name_for_export(player), fields.join(","))
}

fn team_name_for_export(player: &PreparedPlayer) -> String {
    if player.clan_name == player.name {
        String::new()
    } else {
        format!("@{}", player.clan_name)
    }
}

fn attrs_to_overlay_json(attrs: [u32; 8]) -> String {
    attrs
        .into_iter()
        .enumerate()
        .map(|(index, value)| (if index < 7 { value + 36 } else { value }).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Copy)]
enum SkillExportContext {
    Player,
    GenericMinion,
    SummonMinion,
}

fn skills_to_json(skills: &SkillLoadoutSpec, context: SkillExportContext) -> String {
    let mut fields = Vec::new();
    for key in &skills.active_order {
        let Some(entry) = skills.entries.iter().find(|entry| entry.key == *key && entry.level > 0) else {
            continue;
        };
        if entry.skill == BuiltinSkillRef::SummonShareDamage {
            continue;
        }
        let name = skill_export_name(entry, context);
        if fields.iter().any(|field: &String| field.starts_with(&format!("\"{name}\":"))) {
            continue;
        }
        fields.push(format!("\"{name}\":{}", skill_level_json(entry)));
    }
    format!("{{{}}}", fields.join(","))
}

fn skill_export_name(entry: &SkillEntrySpec, context: SkillExportContext) -> String {
    match context {
        SkillExportContext::Player => classified_player_skill_name_for_export(entry.key).unwrap_or_else(|| match entry.skill {
            BuiltinSkillRef::Normal(skill_id) => skill_name_for_export(skill_id),
            BuiltinSkillRef::SummonFire => format!("summon:sklfire{}", usize::from(entry.key != 40) + 1),
            BuiltinSkillRef::SummonExplode => "summon:sklexplode".to_owned(),
            BuiltinSkillRef::Possess => "phantom:sklpossess".to_owned(),
            BuiltinSkillRef::SummonShareDamage => unreachable!(),
        }),
        SkillExportContext::SummonMinion => {
            classified_summon_minion_skill_name_for_export(entry.key).unwrap_or_else(|| match entry.skill {
                BuiltinSkillRef::Normal(skill_id) => format!("normal:{}", skill_name_for_export(skill_id)),
                BuiltinSkillRef::SummonFire => format!("sklfire{}", usize::from(entry.key != 0) + 1),
                BuiltinSkillRef::SummonExplode => "sklexplode".to_owned(),
                BuiltinSkillRef::Possess => "phantom:sklpossess".to_owned(),
                BuiltinSkillRef::SummonShareDamage => unreachable!(),
            })
        }
        SkillExportContext::GenericMinion => match entry.skill {
            BuiltinSkillRef::Normal(skill_id) => format!("normal:{}", skill_name_for_export(skill_id)),
            BuiltinSkillRef::SummonFire => "summon:sklfire1".to_owned(),
            BuiltinSkillRef::SummonExplode => "summon:sklexplode".to_owned(),
            BuiltinSkillRef::Possess => "phantom:sklpossess".to_owned(),
            BuiltinSkillRef::SummonShareDamage => unreachable!(),
        },
    }
}

fn skill_level_json(entry: &SkillEntrySpec) -> String {
    match &entry.boost {
        Some(SkillBoost::SlotBoost { base, boost }) => format!("\"{base}+{boost}\""),
        Some(SkillBoost::LastBoost(base)) => format!("\"2*{base}\""),
        _ => entry.level.to_string(),
    }
}

fn append_minion_export(player: &PreparedPlayer, kind: MinionKind, field: &str, fields: &mut Vec<String>) {
    let overlay_present = player.overlay.as_ref().is_some_and(|overlay| match kind {
        MinionKind::Shadow => overlay.shadow.is_some(),
        MinionKind::Summon => overlay.summon.is_some(),
        MinionKind::Zombie => overlay.zombie.is_some(),
    });
    let skill_id = match kind {
        MinionKind::Shadow => 24,
        MinionKind::Summon => 22,
        MinionKind::Zombie => 32,
    };
    let skill_present = player
        .skills
        .entries
        .iter()
        .any(|entry| entry.skill == BuiltinSkillRef::Normal(skill_id) && entry.level > 0);
    if !overlay_present && !skill_present {
        return;
    }
    let blueprint = player.minion_blueprint(kind, crate::namerena::eval_name::DEFAULT_EVAL_RQ);
    fields.push(format!("\"{field}\":{}", minion_to_json(&blueprint)));
}

fn minion_to_json(blueprint: &PreparedMinionBlueprint) -> String {
    let context = if blueprint.kind == MinionKind::Summon {
        SkillExportContext::SummonMinion
    } else {
        SkillExportContext::GenericMinion
    };
    let mut fields = vec![
        format!("\"attrs\":[{}]", attrs_to_overlay_json(blueprint.player.attrs)),
        format!("\"skills\":{}", skills_to_json(&blueprint.player.skills, context)),
    ];
    if blueprint.kind == MinionKind::Summon {
        fields.push("\"reuse_skills_on_recast\":true".to_owned());
        if blueprint.inherit_owner_def_res {
            fields.push("\"inherit_owner_def_res\":true".to_owned());
        }
    }
    format!("{{{}}}", fields.join(","))
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
    use crate::namerena::{NamerenaInput, PreparedRoster};

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

    #[test]
    fn export_player_keeps_single_plus_weapon_suffix_on_one_player() {
        let raw = "mario@red+fire";
        let original_input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]]).unwrap();
        let original = PreparedRoster::build(&original_input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();

        let exported = export_player(raw, false, true).unwrap();
        assert!(exported.starts_with("mario@red+ol:"));
        assert!(!exported.contains("+fire+ol:"));

        let exported_input = NamerenaInput::from_raw_groups(&[vec![exported]]).unwrap();
        let reparsed = PreparedRoster::build(&exported_input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        assert_eq!(reparsed.players.len(), 1);
        assert_eq!(reparsed.players[0].attrs, original.players[0].attrs);
        assert_eq!(reparsed.players[0].status, original.players[0].status);
        let original_entries = original.players[0]
            .skills
            .entries
            .iter()
            .filter(|entry| entry.level > 0)
            .cloned()
            .collect::<Vec<_>>();
        let reparsed_entries = reparsed.players[0]
            .skills
            .entries
            .iter()
            .filter(|entry| entry.level > 0)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(reparsed_entries, original_entries);
        let original_order = original.players[0]
            .skills
            .active_order
            .iter()
            .filter(|key| original_entries.iter().any(|entry| entry.key == **key))
            .collect::<Vec<_>>();
        let reparsed_order = reparsed.players[0]
            .skills
            .active_order
            .iter()
            .filter(|key| reparsed_entries.iter().any(|entry| entry.key == **key))
            .collect::<Vec<_>>();
        assert_eq!(reparsed_order, original_order);
    }

    #[test]
    fn export_player_accepts_double_plus_for_unteamed_group() {
        let exported = export_player("mario++luigi", true, false).unwrap();
        let groups = parse_plus_separated_groups(&exported);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert!(groups[0][0].starts_with("mario+diy["));
        assert!(groups[0][1].starts_with("luigi+diy["));
    }
}
