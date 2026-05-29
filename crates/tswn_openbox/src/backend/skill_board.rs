use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use tswn_core::engine::storage::Storage;
use tswn_core::player::{Player, skill::skill_name_to_id};

use super::tasks::NamerPfScores;

#[derive(Debug, Clone)]
pub struct SkillBoardConfig {
    entries: HashMap<String, SkillBoardThreshold>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SkillBoardThreshold {
    pub pp: Option<u64>,
    pub pd: Option<u64>,
    pub qp: Option<u64>,
    pub qd: Option<u64>,
    pub all: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SkillBoardLine {
    pub title: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
struct BuiltSkill {
    key: String,
    cn_name: &'static str,
    level: u64,
}

const SKILL_CN_NAMES: [&str; 35] = [
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击", "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速", "诅咒",
    "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气", "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹", "护符", "护盾",
    "反击", "吞噬", "召灵", "垂死", "隐匿",
];

impl SkillBoardConfig {
    pub fn load_default() -> Result<Self, String> {
        let path = current_dir()?.join("setting").join("score_now.toml");
        let raw = fs::read_to_string(&path).map_err(|err| format!("读取技能榜配置失败: {}: {err}", path.display()))?;
        let entries = toml::from_str::<HashMap<String, SkillBoardThreshold>>(&raw)
            .map_err(|err| format!("解析技能榜配置失败: {}: {err}", path.display()))?;
        Ok(Self { entries })
    }

    fn threshold_for(&self, skill_key: &str) -> Option<SkillBoardThreshold> {
        self.entries
            .get(skill_key)
            .copied()
            .or_else(|| self.entries.get(&skill_key.to_ascii_lowercase()).copied())
    }
}

pub fn evaluate_skill_board(name_group: &[String], scores: &NamerPfScores, config: &SkillBoardConfig) -> Vec<SkillBoardLine> {
    let top_skills = highest_skills(name_group);
    let mut lines = Vec::new();
    for skill in top_skills {
        let Some(threshold) = config.threshold_for(&skill.key) else {
            continue;
        };
        push_metric_line(&mut lines, &skill, "pp", scores.pp, threshold.pp);
        push_metric_line(&mut lines, &skill, "pd", scores.pd, threshold.pd);
        push_metric_line(&mut lines, &skill, "qp", scores.qp, threshold.qp);
        push_metric_line(&mut lines, &skill, "qd", scores.qd, threshold.qd);
        push_all_line(&mut lines, &skill, scores, threshold.all);
    }
    lines
}

fn push_metric_line(
    lines: &mut Vec<SkillBoardLine>,
    skill: &BuiltSkill,
    metric_label: &'static str,
    score: f64,
    threshold: Option<u64>,
) {
    if threshold.is_some_and(|limit| score >= limit as f64) {
        lines.push(SkillBoardLine {
            title: format!("{}{}", skill.cn_name, metric_label),
            score,
        });
    }
}

fn push_all_line(lines: &mut Vec<SkillBoardLine>, skill: &BuiltSkill, scores: &NamerPfScores, threshold: Option<u64>) {
    if threshold.is_some_and(|limit| {
        scores.sum >= limit as f64 && scores.pp >= 8000.0 && scores.pd >= 9000.0 && scores.qp >= 6000.0 && scores.qd >= 7000.0
    }) {
        lines.push(SkillBoardLine {
            title: format!("{}全能", skill.cn_name),
            score: scores.sum,
        });
    }
}

fn highest_skills(name_group: &[String]) -> Vec<BuiltSkill> {
    let mut skills = Vec::new();
    for raw in name_group {
        skills.extend(skills_from_name(raw));
    }
    let Some(max_level) = skills.iter().map(|skill| skill.level).max() else {
        return Vec::new();
    };
    skills.retain(|skill| skill.level == max_level && skill.level > 0);
    skills.sort_by(|lhs, rhs| lhs.key.cmp(&rhs.key));
    skills.dedup_by(|lhs, rhs| lhs.key == rhs.key);
    skills
}

fn skills_from_name(raw: &str) -> Vec<BuiltSkill> {
    let storage = Storage::new_arc();
    let mut player = match Player::new_from_namerena_raw(raw.to_string(), storage) {
        Ok(player) => player,
        Err(_) => return Vec::new(),
    };
    player.build();
    let diy = player.to_diy_compact();
    extract_skill_object(&diy).map(parse_skill_object).unwrap_or_default()
}

fn extract_skill_object(diy: &str) -> Option<&str> {
    let start = diy.find("]{")? + 1;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;
    for (offset, ch) in diy[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = Some(start + offset + ch.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }
    end.map(|end| &diy[start..end])
}

fn parse_skill_object(raw: &str) -> Vec<BuiltSkill> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let mut skills = Vec::new();
    for (key, value) in object {
        let Some(id) = skill_name_to_id(key) else {
            continue;
        };
        if id >= SKILL_CN_NAMES.len() {
            continue;
        }
        let level = parse_skill_level(value);
        if level == 0 {
            continue;
        }
        skills.push(BuiltSkill {
            key: canonical_skill_key(key),
            cn_name: SKILL_CN_NAMES[id],
            level,
        });
    }
    skills
}

fn parse_skill_level(value: &serde_json::Value) -> u64 {
    if let Some(level) = value.as_u64() {
        return level;
    }
    let Some(raw) = value.as_str() else {
        return 0;
    };
    if let Some((lhs, rhs)) = raw.split_once('+') {
        return parse_u64(lhs).saturating_add(parse_u64(rhs));
    }
    if let Some((lhs, rhs)) = raw.split_once('*') {
        return parse_u64(lhs).saturating_mul(parse_u64(rhs));
    }
    parse_u64(raw)
}

fn parse_u64(raw: &str) -> u64 { raw.trim().parse().unwrap_or(0) }

fn canonical_skill_key(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.starts_with("skl") {
        lower
    } else {
        format!("skl{lower}")
    }
}

fn current_dir() -> Result<PathBuf, String> { std::env::current_dir().map_err(|err| format!("读取当前目录失败: {err}")) }
