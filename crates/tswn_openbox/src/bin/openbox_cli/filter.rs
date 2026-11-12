//! 按单个名字构建的属性和技能筛选名单。
use std::path::PathBuf;
use tswn_core::namerena::{BuiltinSkillRef, NamerenaInput, PreparedPlayer, PreparedRoster};
use tswn_core::runtime::BuiltinActiveSkill;

#[derive(Debug)]
pub(super) struct FilterPlan {
    pub raw: String,
    pub output: PathBuf,
    pub conditions: Vec<Condition>,
}

#[derive(Debug)]
enum Field {
    Skill(usize),
    EffectiveSkill(usize),
    Attr(usize),
    All7,
    All8,
}

#[derive(Debug)]
pub(super) struct Condition {
    field: Field,
    threshold: f64,
    less: bool,
}

/// 带 skl 的名称精确解析，其余名称按去掉 skl 后的前缀匹配，拒绝歧义。
fn parse_skill(skill: &str) -> Result<usize, String> {
    if skill.starts_with("skl") {
        return tswn_core::namerena::skill_name_to_id(skill).ok_or_else(|| format!("未知技能: {skill}"));
    }
    if skill.is_empty() {
        return Err("技能名称不能为空".to_owned());
    }
    let matches: Vec<_> = (0..=35)
        .filter_map(|id| {
            let name = tswn_core::namerena::skill_name_for_export(id);
            name.strip_prefix("skl").is_some_and(|name| name.starts_with(skill)).then_some((id, name))
        })
        .collect();
    match matches.as_slice() {
        [(id, _)] => Ok(*id),
        [] => Err(format!("未知技能前缀: {skill}")),
        _ => Err(format!(
            "技能前缀 {skill} 不唯一，请使用更长的前缀或完整名称：{}",
            matches.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>().join("、")
        )),
    }
}

impl Condition {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (name, value) = spec.split_once('=').ok_or_else(|| format!("无效筛选条件: {spec}"))?;
        let field = if let Some(skill) = name.strip_prefix("skill-") {
            Field::Skill(parse_skill(skill)?)
        } else if let Some(skill) = name.strip_prefix("skills-") {
            let id = parse_skill(skill)?;
            if BuiltinActiveSkill::from_legacy_key(id).is_none() {
                return Err(format!("{skill} 不是主动技能，不能按等效熟练度筛选"));
            }
            Field::EffectiveSkill(id)
        } else if let Some(attr) = name.strip_prefix("addr-") {
            match attr {
                "atk" => Field::Attr(0),
                "def" => Field::Attr(1),
                "spd" => Field::Attr(2),
                "agi" => Field::Attr(3),
                "mag" => Field::Attr(4),
                "res" => Field::Attr(5),
                "int" => Field::Attr(6),
                "hp" => Field::Attr(7),
                "all7" => Field::All7,
                "all8" => Field::All8,
                _ => return Err(format!("未知属性: {attr}")),
            }
        } else {
            return Err(format!("未知筛选条件: {name}"));
        };
        let number: f64 = value.parse().map_err(|_| format!("无效阈值: {value}"))?;
        if !number.is_finite() {
            return Err(format!("无效阈值: {value}"));
        }
        Ok(Self {
            field,
            threshold: number.abs(),
            less: number.is_sign_negative(),
        })
    }

    fn matches(&self, player: &PreparedPlayer) -> bool {
        let sum: u32 = player.attrs[..7].iter().map(|v| v + 36).sum();
        let actual = match self.field {
            Field::Skill(id) => player
                .skills
                .entries
                .iter()
                .filter(|entry| entry.skill == BuiltinSkillRef::Normal(id))
                .map(|entry| entry.level)
                .max()
                .unwrap_or(0) as f64,
            Field::EffectiveSkill(id) => effective_skill_level(player, id),
            Field::Attr(index) => (player.attrs[index] + if index < 7 { 36 } else { 0 }) as f64,
            Field::All7 => sum as f64,
            Field::All8 => sum as f64 + player.attrs[7] as f64 / 3.0,
        };
        if self.less {
            actual < self.threshold
        } else {
            actual >= self.threshold
        }
    }
}

/// 按发动顺序消耗剩余概率；仅考虑技能熟练度，不引入战局、目标或状态条件。
fn effective_skill_level(player: &PreparedPlayer, id: usize) -> f64 {
    let mut remaining = 1.0;
    let mut effective = 0.0;
    for key in &player.skills.active_order {
        let Some(entry) = player.skills.entries.iter().find(|entry| entry.key == *key) else {
            continue;
        };
        let active = match entry.skill {
            BuiltinSkillRef::Normal(skill) => BuiltinActiveSkill::from_legacy_key(skill).is_some(),
            BuiltinSkillRef::SummonFire | BuiltinSkillRef::SummonExplode | BuiltinSkillRef::Possess => true,
            BuiltinSkillRef::SummonShareDamage => false,
            // 武器与身份专属技能没有普通技能键，暂不参与有效熟练度计算；如需纳入，按实际发动规则补分支。
            _ => false,
        };
        if !active {
            continue;
        }
        let probability = f64::from(entry.level.min(128)) / 128.0;
        if entry.skill == BuiltinSkillRef::Normal(id) {
            effective += remaining * probability * 128.0;
        }
        remaining *= 1.0 - probability;
    }
    effective
}

pub(super) fn run(plan: FilterPlan) -> Result<(), String> {
    let mut output = String::new();
    for line in plan.raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let input =
            NamerenaInput::from_raw_groups(&[vec![line.to_owned()]]).map_err(|err| format!("构建玩家失败: {line}: {err}"))?;
        let roster = match PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
            Ok(roster) => roster,
            Err(error) => match error {},
        };
        let player = roster.players.first().ok_or_else(|| format!("无有效玩家: {line}"))?;
        if plan.conditions.iter().all(|condition| condition.matches(player)) {
            output.push_str(line);
            output.push('\n');
        }
    }
    std::fs::write(&plan.output, output).map_err(|err| format!("写入 {} 失败: {err}", plan.output.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_use_exported_attributes_and_exact_eight_sum() {
        let input = NamerenaInput::from_raw_groups(&[vec![
            r#"mario+ol:{"attrs":[40,50,60,70,80,90,99,100],"skills":{"sklfire":60}}"#.to_owned(),
        ]])
        .unwrap();
        let roster = PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        let player = &roster.players[0];
        for spec in [
            "addr-def=50",
            "addr-def=-51",
            "addr-all7=489",
            "addr-all8=522.3",
            "skill-sklfire=60",
            "skill-fi=60",
            "skill-fire=60",
            "skill-ice=-1",
            "skill-sklice=-1",
        ] {
            assert!(Condition::parse(spec).unwrap().matches(player), "{spec}");
        }
        for spec in ["addr-def=-50", "addr-all8=522.4", "skill-sklfire=-60", "skill-sklice=1"] {
            assert!(!Condition::parse(spec).unwrap().matches(player), "{spec}");
        }
    }

    #[test]
    fn skill_abbreviations_reject_unknown_and_ambiguous_names() {
        assert!(Condition::parse("skill-zzz=60").is_err());
        assert!(Condition::parse("skill-=60").is_err());
        for prefix in ["f", "fi", "fir", "fire", "sklfire"] {
            assert_eq!(parse_skill(prefix).unwrap(), 0, "{prefix}");
        }
        for kind in ["skill", "skills"] {
            let error = Condition::parse(&format!("{kind}-char=60")).unwrap_err();
            assert!(error.contains("sklcharm") && error.contains("sklcharge"));
            assert!(Condition::parse(&format!("{kind}-charm=60")).is_ok());
            assert!(Condition::parse(&format!("{kind}-charge=60")).is_ok());
        }
        assert!(Condition::parse("skill-sklfi=60").is_err());
        assert!(Condition::parse("skill-sklcharm=60").is_ok());
        assert!(Condition::parse("skill-sklcharge=60").is_ok());
    }

    #[test]
    fn effective_skills_follow_action_order_and_preserve_threshold_boundaries() {
        let input = NamerenaInput::from_raw_groups(&[vec![
            r#"mario+ol:{"skills":{"skldefend":128,"sklice":64,"sklfire":64,"sklthunder":128,"sklquake":64}}"#.to_owned(),
        ]])
        .unwrap();
        let roster = PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        let player = &roster.players[0];
        for spec in [
            "skills-sklice=64",
            "skills-fire=32",
            "skills-fi=32",
            "skills-thun=32",
            "skills-quak=-0.01",
            "skills-heal=-1",
            "skill-sklfire=64",
        ] {
            assert!(Condition::parse(spec).unwrap().matches(player), "{spec}");
        }
        for spec in ["skills-ice=-64", "skills-fire=-32", "skills-fire=32.01", "skills-quak=0.01"] {
            assert!(!Condition::parse(spec).unwrap().matches(player), "{spec}");
        }
        assert!(Condition::parse("skills-skldefend=1").is_err());
        assert!(Condition::parse("skills-char=1").is_err());
    }

    #[test]
    fn effective_skills_keep_fractions_and_cap_probability_at_one() {
        let input = NamerenaInput::from_raw_groups(&[vec![
            r#"mario+ol:{"skills":{"sklfire":1,"sklice":1,"sklthunder":256,"sklquake":128}}"#.to_owned(),
        ]])
        .unwrap();
        let roster = PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        let player = &roster.players[0];
        assert_eq!(effective_skill_level(player, 1), 127.0 / 128.0);
        assert_eq!(effective_skill_level(player, 2), 127.0 * 127.0 / 128.0);
        assert_eq!(effective_skill_level(player, 3), 0.0);
        assert!(Condition::parse("skills-ice=0.99").unwrap().matches(player));
        assert!(!Condition::parse("skills-ice=0.993").unwrap().matches(player));
    }

    #[test]
    fn writes_only_matching_names_in_order() {
        let path = std::env::temp_dir().join(format!("openbox-filter-{}.txt", std::process::id()));
        let first = r#"first+ol:{"attrs":[40,90,60,70,80,90,99,100],"skills":{"sklfire":60}}"#;
        let second = r#"second+ol:{"attrs":[40,89,60,70,80,90,99,100],"skills":{"sklfire":60}}"#;
        let third = r#"third+ol:{"attrs":[40,90,60,70,80,90,99,100],"skills":{"sklfire":59}}"#;
        run(FilterPlan {
            raw: format!("{first}\n\n{second}\n{third}\n{first}\n"),
            output: path.clone(),
            conditions: ["addr-def=90", "skill-sklfire=60"].iter().map(|s| Condition::parse(s).unwrap()).collect(),
        })
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{first}\n{first}\n"));
        std::fs::remove_file(path).unwrap();
    }
}
