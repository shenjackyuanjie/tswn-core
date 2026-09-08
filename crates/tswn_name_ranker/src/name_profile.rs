use tswn_core::namerena::{BuiltinSkillRef, NamerenaInput, PreparedPlayer, PreparedRoster, eval_name::DEFAULT_EVAL_RQ};

pub fn build_player(raw: &str) -> anyhow::Result<PreparedPlayer> {
    let input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]])?;
    let mut roster = match PreparedRoster::build(&input, DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    roster.players.pop().ok_or_else(|| anyhow::anyhow!("没有构建出角色"))
}

pub fn export_player(raw: &str) -> anyhow::Result<String> { Ok(tswn_core::cli_api::to_diy(raw, false, true)?) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_order_boosts_and_passive_scaling_are_preserved() {
        let player =
            build_player(r#"alice@A+ol:{"skills":{"sklfire":"40+30","sklice":64,"sklshield":70,"skldefend":30}}"#).unwrap();
        let values = player_effective_skill_values(&player);
        assert_eq!(values[0], 70.0);
        assert_eq!(values[1], 29.0);
        assert_eq!(values[29], 59.5);
        assert_eq!(values[25], 25.5);
        assert_eq!(player_text_type(&player), "火球护盾冰冻防御");
    }

    #[test]
    fn empty_skills_use_attribute_label() {
        let player = build_player(r#"alice@A+ol:{"skills":{}}"#).unwrap();
        assert_eq!(player_text_type(&player), "高八维");
    }

    #[test]
    fn export_roundtrip_keeps_skills_attributes_and_minions() {
        let raw = r#"alice@A+ol:{"skills":{"sklclone":60,"sklsummon":55},"shadow":{"attrs":[80,81,82,83,84,85,86,300],"skills":{"sklfire":50}}}"#;
        let before = build_player(raw).unwrap();
        let exported = export_player(raw).unwrap();
        let after = build_player(&exported).unwrap();
        assert_eq!(before.attrs, after.attrs);
        assert_eq!(player_effective_skill_values(&before), player_effective_skill_values(&after));
        let before_shadow = before.minion_blueprint(tswn_core::namerena::MinionKind::Shadow, DEFAULT_EVAL_RQ);
        let after_shadow = after.minion_blueprint(tswn_core::namerena::MinionKind::Shadow, DEFAULT_EVAL_RQ);
        assert_eq!(before_shadow.player.attrs, after_shadow.player.attrs);
        assert_eq!(
            player_effective_skill_values(&before_shadow.player),
            player_effective_skill_values(&after_shadow.player)
        );
        assert!(exported.contains("\"shadow\""));
        assert!(exported.contains("\"summon\""));
    }
}

pub fn player_text_type(player: &PreparedPlayer) -> String {
    const LABELS: [&str; 35] = [
        "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击", "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速", "诅咒",
        "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气", "背刺", "血祭", "分身", "幻术", "防御", "守护", "反弹", "护符", "护盾",
        "反击", "吞噬", "召灵", "垂死", "隐匿",
    ];
    let effective = player_effective_skill_values(player);
    let mut skills = (0..35)
        .filter_map(|id| (effective[id] >= 25.0).then_some((id, effective[id])))
        .collect::<Vec<_>>();
    skills.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if skills.is_empty() {
        "高八维".into()
    } else {
        skills.into_iter().map(|(id, _)| LABELS[id]).collect::<Vec<_>>().join("")
    }
}

fn player_effective_skill_values(player: &PreparedPlayer) -> [f64; 35] {
    let storage = &player.skills;
    let mut values = [0.0; 35];
    let mut action_mass = 1.0;
    let mut kill_mass = 1.0;

    for key in storage.active_order.iter().take(16) {
        let Some(entry) = storage.entries.iter().find(|entry| entry.key == *key) else {
            continue;
        };
        let BuiltinSkillRef::Normal(skill_id) = entry.skill else {
            continue;
        };
        if skill_id >= 35 {
            continue;
        }
        let level = entry.level as f64;
        if skill_id == 9 || skill_id == 16 {
            values[skill_id] = action_mass * level;
            action_mass *= 1.0 - level * 0.3 / 128.0;
        } else if skill_id == 18 {
            values[skill_id] = action_mass * level;
            action_mass *= 1.0 - level * 0.35 / 128.0;
        } else if skill_id == 19 || skill_id == 23 {
            values[skill_id] = action_mass * level;
            action_mass *= 1.0 - level * 0.6 / 128.0;
        } else if skill_id == 20 || skill_id == 22 {
            values[skill_id] = action_mass * level;
            action_mass *= 1.0 - level * 0.7 / 128.0;
        } else if skill_id < 25 {
            values[skill_id] = action_mass * level;
            action_mass *= 1.0 - level / 128.0;
        } else if skill_id == 31 || skill_id == 32 {
            values[skill_id] = kill_mass * level;
            kill_mass *= 1.0 - level / 128.0;
        } else {
            values[skill_id] = level;
        }
    }

    values[29] = if values[29] <= 70.0 {
        values[29] * values[29] / 70.0
    } else {
        values[29] * 2.0 - 70.0
    };
    // Text-Type 的被动技能采用等效熟练度的 85%，再与统一的 25 阈值比较。
    for value in &mut values[25..35] {
        *value *= 0.85;
    }
    values
}
