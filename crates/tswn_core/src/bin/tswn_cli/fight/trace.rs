//! 主 Runtime 的对战日志格式化与 trace 归一化逻辑。

use tswn_core::runtime::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, PlayerKindFlags, RuntimeRunner};

use super::driver::fmt_runtime_winner_input_indices;

fn runtime_plr_name_diff(runner: &RuntimeRunner, id: usize) -> String {
    let Ok(entity_id) = u32::try_from(id) else {
        return format!("#{id}");
    };
    runner
        .runtime()
        .entities
        .get(EntityIdx(entity_id))
        .map(|entity| entity.template.display_name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn runtime_plr_name(runner: &RuntimeRunner, id: usize) -> String {
    let Ok(entity_id) = u32::try_from(id) else {
        return format!("#{id}");
    };
    let Some(entity) = runner.runtime().entities.get(EntityIdx(entity_id)) else {
        return format!("#{id}");
    };
    if !entity.runtime.flags.contains(PlayerKindFlags::MINION) {
        return entity.template.id_key_name.clone();
    }

    let root = runner.runtime().entities.get(entity.runtime.root_owner).unwrap_or(entity);
    let id_name = &entity.template.name;
    let clan_name = &root.template.clan_name;
    if !clan_name.is_empty() && clan_name != id_name {
        format!("{id_name}@{clan_name}")
    } else {
        id_name.clone()
    }
}

pub(super) fn fmt_runtime_update(runner: &RuntimeRunner, update: &RunUpdate) -> String {
    let caster = runtime_plr_name(runner, update.caster);
    let target = runtime_plr_name(runner, update.target);
    let targets = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| runtime_plr_name(runner, *id))
            .collect::<Vec<_>>()
            .join(",")
    };

    let mut message = update.message.to_string();
    message = message.replace("[0]", &caster);
    message = message.replace("[1]", &target);
    message = message.replace("[2]", &targets);
    if update.score > 0 {
        format!("{message}  (+{})", update.score)
    } else {
        message
    }
}

fn fmt_runtime_update_diff(runner: &RuntimeRunner, update: &RunUpdate) -> String {
    let caster = runtime_plr_name_diff(runner, update.caster);
    let target = runtime_plr_name_diff(runner, update.target);
    let targets = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| runtime_plr_name_diff(runner, *id))
            .collect::<Vec<_>>()
            .join(",")
    };

    update.message.replace("[0]", &caster).replace("[1]", &target).replace("[2]", &targets)
}

fn fmt_runtime_update_raw(runner: &RuntimeRunner, update: &RunUpdate) -> String {
    let raw_name = |id: usize| {
        let Ok(entity_id) = u32::try_from(id) else {
            return format!("#{id}");
        };
        let Some(entity) = runner.runtime().entities.get(EntityIdx(entity_id)) else {
            return format!("#{id}");
        };
        if entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            entity.template.display_name.clone()
        } else {
            runtime_plr_name(runner, id)
        }
    };

    let caster = raw_name(update.caster);
    let target = raw_name(update.target);
    let targets = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update.targets.iter().map(|id| raw_name(*id)).collect::<Vec<_>>().join(",")
    };
    update.message.replace("[0]", &caster).replace("[1]", &target).replace("[2]", &targets)
}

fn sanitize_output_line(line: &str) -> String {
    let filtered = line
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect::<String>();
    let mut normalized = String::with_capacity(filtered.len());
    let mut prev_space = false;
    for ch in filtered.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                normalized.push(' ');
                prev_space = true;
            }
        } else {
            normalized.push(ch);
            prev_space = false;
        }
    }
    normalized.trim().to_string()
}

fn normalize_trace_line(line: String) -> String {
    let mut normalized = line
        .replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(' ', "")
        .replace('！', "!")
        .replace('？', "?")
        .replace('，', ",")
        .replace('：', ":")
        .replace('；', ";")
        .replace('（', "(")
        .replace('）', ")")
        .replace('²', "2");

    for (from, to) in [
        ("[回避]", "回避"),
        ("[反击]", "反击"),
        ("[吸血攻击]", "吸血攻击"),
        ("[聚气]", "聚气"),
        ("[潜行]", "潜行"),
        ("[背刺]", "背刺"),
        ("[狂暴攻击]", "狂暴攻击"),
        ("[狂暴术]", "狂暴术"),
        ("[狂暴]", "狂暴"),
        ("[蓄力]", "蓄力"),
        ("[隐匿]", "隐匿"),
        ("[魅惑]", "魅惑"),
        ("[防御]", "防御"),
        ("[吞噬]", "吞噬"),
        ("[分身]", "分身"),
        ("[会心一击]", "会心一击"),
        ("[伤害反弹]", "伤害反弹"),
        ("[净化]", "净化"),
        ("[护身符]", "护身符"),
        ("[诅咒]", "诅咒"),
        ("[守护]", "守护"),
        ("[生命之轮]", "生命之轮"),
        ("[垂死]", "垂死"),
        ("[火球术]", "火球术"),
        ("[瘟疫]", "瘟疫"),
        ("[加速术]", "加速术"),
        ("[疾走]", "疾走"),
        ("[治愈魔法]", "治愈魔法"),
        ("[迟缓]", "迟缓"),
        ("[中毒]", "中毒"),
        ("[冰冻术]", "冰冻术"),
        ("[冰冻]", "冰冻"),
        ("[铁壁]", "铁壁"),
        ("[投毒]", "投毒"),
        ("[毒性发作]", "毒性发作"),
        ("[附体]", "附体"),
        ("[地裂术]", "地裂术"),
        ("[连击]", "连击"),
        ("[苏生术]", "苏生术"),
        ("[复活]", "复活"),
        ("[幻术]", "幻术"),
        ("[减速术]", "减速术"),
        ("[雷击术]", "雷击术"),
        ("[血祭]", "血祭"),
        ("[召唤亡灵]", "召唤亡灵"),
        ("[自爆]", "自爆"),
    ] {
        normalized = normalized.replace(from, to);
    }
    sanitize_output_line(&normalized)
}

fn normalize_diff_trace_line(line: String) -> String {
    line.replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(['[', ']'], "")
        .replace(' ', "")
        .trim()
        .to_string()
}

pub(super) fn collect_runtime_diff_lines(
    runner: &mut RuntimeRunner,
    max_rounds: usize,
    normalize: bool,
) -> (Vec<String>, usize, u64) {
    let mut lines = Vec::new();
    let mut guard = 0usize;
    let mut total_score = 0u64;
    while guard < max_rounds {
        let outcome = runner.run_round();
        if let Some(frame) = outcome.frame {
            let mut parts = Vec::new();
            for update in frame.updates.updates {
                if matches!(update.update_type, UpdateType::NextLine) {
                    if !parts.is_empty() {
                        lines.push(parts.join(", "));
                        parts.clear();
                    }
                    continue;
                }
                total_score += u64::from(update.score);
                let mut message = fmt_runtime_update_diff(runner, &update);
                if normalize {
                    message = normalize_diff_trace_line(message);
                }
                if !message.is_empty() {
                    parts.push(message);
                }
            }
            if !parts.is_empty() {
                lines.push(parts.join(", "));
            }
        }
        guard += 1;
        if outcome.winner_team.is_some() {
            break;
        }
    }
    (lines, guard, total_score)
}

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

fn emit_current_turn(output: &mut Vec<String>, action: &mut String, misc: &mut Vec<String>) {
    if !action.is_empty() {
        output.push(std::mem::take(action));
        output.push(String::new());
        misc.clear();
    } else if !misc.is_empty() {
        output.push(misc.join(", "));
        output.push(String::new());
        misc.clear();
    }
}

pub(super) fn collect_runtime_fight_raw_lines(runner: &mut RuntimeRunner, input_player_count: usize) -> Vec<String> {
    let mut output = Vec::new();
    let mut action = String::new();
    let mut misc = Vec::new();
    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    while runner.runtime().world.winner_team().is_none() && round <= 100_000 {
        let outcome = runner.run_round();
        let finished = outcome.winner_team.is_some();
        let Some(frame) = outcome.frame else {
            idle_rounds += 1;
            if finished || idle_rounds > 16 {
                break;
            }
            continue;
        };
        if frame.updates.updates.is_empty() {
            idle_rounds += 1;
            if finished || idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;
        for update in frame.updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                emit_current_turn(&mut output, &mut action, &mut misc);
                continue;
            }
            let line = normalize_trace_line(fmt_runtime_update_raw(runner, &update));
            if line.is_empty() {
                continue;
            }
            if is_action_line(&line) {
                emit_current_turn(&mut output, &mut action, &mut misc);
                action = line;
            } else if action.is_empty() {
                misc.push(line);
            } else {
                action.push_str(", ");
                action.push_str(&line);
            }
        }
        round += 1;
        if finished {
            break;
        }
    }

    emit_current_turn(&mut output, &mut action, &mut misc);
    while matches!(output.last(), Some(line) if line.is_empty()) {
        output.pop();
    }
    if let Some(win_idx) = fmt_runtime_winner_input_indices(runner, input_player_count) {
        output.push(win_idx);
    }
    output
}
