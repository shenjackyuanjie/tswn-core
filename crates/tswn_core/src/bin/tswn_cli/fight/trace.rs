//! 主 Runtime 的对战日志格式化与 trace 归一化逻辑。

use tswn_core::runtime::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, PlayerKindFlags, RuntimeRunner};

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
