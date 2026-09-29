//! 名字导出与属性、技能详情。

use super::format::SKILL_CN_NAMES;
use super::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::finish_output;
use super::parse::{parse_line_list, parse_namer_pf_groups};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tswn_core::cli_api;
use tswn_core::namerena::{BuiltinSkillRef, NamerenaInput, PreparedPlayer, PreparedRoster};

/// 导出 `to-diy` 结果。
///
/// `details` 只影响日志：每一行都按 `+` 语义整行构队，再把这一行里的每个玩家各输出
/// 一个“原始信息”详情块（组队行也输出，单玩家行就是一个块）。输出文件内容、
/// `--old` / `--minions` 导出格式都不受它影响。
pub fn run_to_diy(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String, String> {
    run_to_diy_observed(raw, old, minions, details, output_file, cancel, None)
}

#[allow(clippy::too_many_arguments)]
pub fn run_to_diy_observed(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
    observer: ResultObserver<'_>,
) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    // 详情属于日志产物：选了输出文件时只写导出行，避免把详情混进文件。
    let details = details && output_file.is_none();
    let needs_text = observer.is_none() || output_file.is_some();
    let mut out = String::new();
    for (index, name) in names.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Ok("已停止。".to_string());
        }
        // 每行结果之间空一行：否则上一行的详情块会和下一行的导出行贴在一起。
        if needs_text && !out.is_empty() {
            let _ = writeln!(out);
        }
        let mut update = observer.map(|_| ResultUpdate::new(index, name, ResultKind::Diy, 0));
        let export = cli_api::to_diy(name, old, minions).map_err(|err| format!("导出 DIY 失败: {name}: {err}"))?;
        if needs_text {
            let _ = writeln!(out, "{export}");
        }
        if let Some(update) = &mut update {
            update.entries.push(ResultEntry {
                index: 0,
                label: "导出".into(),
                value: None,
                text: export,
                kind: EntryKind::Plain,
            });
        }

        if details {
            for detail in to_diy_details(name)? {
                if needs_text {
                    let _ = writeln!(out);
                    append_to_diy_details(&mut out, &detail);
                }
                if let Some(update) = &mut update {
                    for (attr_index, label) in ["攻", "防", "速", "敏", "魔", "抗", "智", "HP"].iter().enumerate() {
                        let delta = i64::from(detail.attrs[attr_index]) - i64::from(detail.solo_attrs[attr_index]);
                        update.entries.push(ResultEntry {
                            index: update.entries.len(),
                            label: format!("{} · {label}", detail.name),
                            value: Some(f64::from(detail.attrs[attr_index])),
                            text: format_delta(i64::from(detail.attrs[attr_index]), i64::from(detail.solo_attrs[attr_index])),
                            kind: if delta != 0 { EntryKind::Highlight } else { EntryKind::Plain },
                        });
                    }
                    update.entries.push(ResultEntry {
                        index: update.entries.len(),
                        label: format!("{} · 八围", detail.name),
                        value: None,
                        text: format_eight_ring(detail.attrs),
                        kind: EntryKind::Plain,
                    });
                    update.entries.push(ResultEntry::number(
                        update.entries.len(),
                        format!("{} · 嘲讽", detail.name),
                        taunt_value(detail.attrs) as f64,
                    ));
                    for skill in &detail.skills {
                        update.entries.push(ResultEntry {
                            index: update.entries.len(),
                            label: format!("{} · {}", detail.name, skill.name),
                            value: Some(f64::from(skill.level)),
                            text: format_delta(i64::from(skill.level), i64::from(skill.level) - skill.delta),
                            kind: EntryKind::Plain,
                        });
                    }
                }
            }
        }
        if let (Some(observer), Some(mut update)) = (observer, update) {
            update.finish = Some(ResultFinish {
                score: None,
                visible: true,
                highlight: false,
            });
            observer(update);
        }
    }

    let result = finish_output(output_file.as_deref(), out)?;
    Ok(if observer.is_some() && output_file.is_none() {
        "完成。".to_owned()
    } else {
        result
    })
}

/// 详情块里的一行技能：中文名、整队后的熟练度，以及相对单独构建的变化量。
#[derive(Debug)]
struct ToDiySkill {
    name: &'static str,
    level: u32,
    delta: i64,
}

/// 一个玩家的详情块内容。
///
/// `attrs` / `skills` 取自本行整队构建的结果（与导出行一致）；`solo_attrs` 是同一个
/// 成员单独构建的结果，用来标出“组队后与原属性”的差额。
struct ToDiyDetails {
    name: String,
    attrs: [u32; 8],
    solo_attrs: [u32; 8],
    skills: Vec<ToDiySkill>,
}

/// 为一行输入构建详情块：整行按 `+` 组成一队后再逐个玩家取值。
///
/// 必须按整行构队而不是单独构建某个名字：同队成员之间会互相升级属性
/// （例如 `1@team` 单独构队 HP 243，和 `2@team` 同队后是 245），
/// 只有整行构队才能和导出行逐项对齐。每个成员再各自单独构建一次（同 overlay、
/// 只是没有队友），两者之差就是该成员的“组队加成”，用来输出 `(+N)`。
fn to_diy_details(name: &str) -> Result<Vec<ToDiyDetails>, String> {
    let groups = parse_namer_pf_groups(name);
    let Some(group) = groups.first().filter(|group| !group.is_empty()) else {
        return Ok(Vec::new());
    };
    let group_input = NamerenaInput::from_raw_groups(std::slice::from_ref(group))
        .map_err(|err| format!("构建玩家失败: {}: {err}", group.join("+")))?;
    let roster = match PreparedRoster::build(&group_input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    let mut details = Vec::with_capacity(roster.players.len());
    for player in &roster.players {
        // `player.id` 就是该成员在本行 group 里的下标：单独构建时保留原字符串
        // （overlay 等后缀都在里面），只是没有队友。
        let solo_source = group.get(player.id).cloned().unwrap_or_else(|| player.id_key_name.clone());
        let solo = solo_player_of(&solo_source).map_err(|err| format!("构建玩家失败: {solo_source}: {err}"))?;
        let solo_levels = skill_levels(&solo);
        details.push(ToDiyDetails {
            name: player.id_key_name.clone(),
            attrs: export_attrs(player.attrs),
            solo_attrs: export_attrs(solo.attrs),
            skills: action_order_skills(player, &solo_levels),
        });
    }
    Ok(details)
}

/// 单独构建一个成员（保留它的 overlay 等后缀），返回它的玩家数据。
fn solo_player_of(raw: &str) -> Result<PreparedPlayer, String> {
    let input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]]).map_err(|err| err.to_string())?;
    let roster = match PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    roster.players.into_iter().next().ok_or_else(|| "无有效玩家".to_string())
}

/// 玩家实际生效的技能等级：key 是导出名（`skl*` / `summon:skl*` / `phantom:skl*`），
/// 与 `+ol` 导出用的是同一套命名，因此两次构建可以直接对齐。
fn skill_levels(player: &PreparedPlayer) -> Vec<(String, u32)> {
    player
        .skills
        .active_order
        .iter()
        .filter_map(|key| {
            let entry = player.skills.entries.iter().find(|entry| entry.key == *key && entry.level > 0)?;
            if entry.skill == BuiltinSkillRef::SummonShareDamage {
                return None;
            }
            Some((export_skill_name(entry), entry.level))
        })
        .collect()
}

/// 详情块的技能行：逐项与单独构建的同名技能比差额。
fn action_order_skills(player: &PreparedPlayer, solo_levels: &[(String, u32)]) -> Vec<ToDiySkill> {
    let mut skills = Vec::new();
    for (key, level) in skill_levels(player) {
        let Some(name) = cn_skill_name(&key) else {
            continue;
        };
        let solo = solo_levels
            .iter()
            .find_map(|(solo_key, solo_level)| (solo_key == &key).then_some(i64::from(*solo_level)))
            .unwrap_or(0);
        skills.push(ToDiySkill {
            name,
            level,
            delta: i64::from(level) - solo,
        });
    }
    skills
}

/// 导出名到中文名：只认技能榜同一张表里的常规技能。
fn cn_skill_name(export_name: &str) -> Option<&'static str> {
    let id = tswn_core::namerena::skill_name_to_id(export_name)?;
    SKILL_CN_NAMES.get(id).copied()
}

/// 技能导出名，与 `cli_api` 的 `+ol` 导出（`skill_export_name` 的 Player 分支）一致。
fn export_skill_name(entry: &tswn_core::namerena::SkillEntrySpec) -> String {
    tswn_core::namerena::classified_player_skill_name_for_export(entry.key).unwrap_or_else(|| match entry.skill {
        BuiltinSkillRef::Normal(skill_id) => tswn_core::namerena::skill_name_for_export(skill_id),
        _ => entry.key.to_string(),
    })
}

/// 把内部属性换算成导出口径：前七围 +36（与 `attrs_to_overlay_json` 一致），HP 原样。
fn export_attrs(attrs: [u32; 8]) -> [u32; 8] {
    let mut out = attrs;
    for value in &mut out[..7] {
        *value += 36;
    }
    out
}

/// 追加“单名详情”块。
///
/// 块内只描述构建后的玩家本体：名字、HP 与七围、八围、嘲讽，以及按行动顺序排列的
/// 技能行。属性与技能都按“组队后相对单独构建”的差额标注 `(+N)` / `(-N)`。
/// 整块只走日志，不写入输出文件。
fn append_to_diy_details(out: &mut String, details: &ToDiyDetails) {
    let attrs = details.attrs;
    let _ = writeln!(out, "=== 原始信息 ===");
    let _ = writeln!(out, "{}", details.name);
    let _ = writeln!(
        out,
        "HP {} 攻 {} 防 {} 速 {} 敏 {} 魔 {} 抗 {} 智 {} 八围 {} 嘲讽{}",
        format_delta(i64::from(attrs[7]), i64::from(details.solo_attrs[7])),
        format_delta(i64::from(attrs[0]), i64::from(details.solo_attrs[0])),
        format_delta(i64::from(attrs[1]), i64::from(details.solo_attrs[1])),
        format_delta(i64::from(attrs[2]), i64::from(details.solo_attrs[2])),
        format_delta(i64::from(attrs[3]), i64::from(details.solo_attrs[3])),
        format_delta(i64::from(attrs[4]), i64::from(details.solo_attrs[4])),
        format_delta(i64::from(attrs[5]), i64::from(details.solo_attrs[5])),
        format_delta(i64::from(attrs[6]), i64::from(details.solo_attrs[6])),
        format_eight_ring(attrs),
        taunt_value(attrs),
    );
    for skill in &details.skills {
        let solo = i64::from(skill.level) - skill.delta;
        let _ = writeln!(out, "  {} {}", skill.name, format_delta(i64::from(skill.level), solo));
    }
}

/// 一个数值：整队后的值，与单独构建有差额时追加 `(+N)` / `(-N)`。
fn format_delta(teamed: i64, solo: i64) -> String {
    match teamed.cmp(&solo) {
        std::cmp::Ordering::Equal => teamed.to_string(),
        std::cmp::Ordering::Greater => format!("{teamed}(+{})", teamed - solo),
        std::cmp::Ordering::Less => format!("{teamed}(-{})", solo - teamed),
    }
}

/// 八围 = 七围之和 + HP / 3，四舍五入到一位小数。
///
/// 用整数定点取到 0.1：`(total * 10 + 1) / 3`，其中 `total = 七围之和 * 3 + HP`
/// 是 `(七围之和 + HP / 3)` 的 3 倍；除数 3 用 `+1` 做四舍五入。全程整数，
/// 避免浮点格式化在边界上的抖动。
fn format_eight_ring(attrs: [u32; 8]) -> String {
    let seven = attrs[..7].iter().map(|value| u64::from(*value)).sum::<u64>();
    let total = seven * 3 + u64::from(attrs[7]);
    let tenths = (total * 10 + 1) / 3;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// 嘲讽值 = 防 * 2 + 抗 * 2 - 攻 * 2 - 魔 * 2 - 速 * 2 - 敏 - 智。
///
/// 公式本身给出负数（属性越强嘲讽越低），面板按数值大小显示，这里取绝对值。
fn taunt_value(attrs: [u32; 8]) -> u64 {
    let read = |index: usize| i64::from(attrs[index]);
    let raw = read(1) * 2 + read(5) * 2 - read(0) * 2 - read(4) * 2 - read(2) * 2 - read(3) - read(6);
    raw.unsigned_abs()
}

#[cfg(test)]
mod tests;
