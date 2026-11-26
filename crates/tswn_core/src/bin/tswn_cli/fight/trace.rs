//! 主 Runtime 的对战日志格式化与 trace 归一化逻辑。
//!
//! 收集入口会推进传入 runner，直到出现胜者或用尽回合预算；返回消息行、实际推进轮数
//! 和累计战斗分。名字缺失时用 `#ID` 显示，归一化模式移除展示标签及空白装饰，
//! 只用于文本诊断，不是机器可读事件或机制检查点。

use tswn_core::runtime::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, RuntimeRunner};

/// 把实体 id 解析为 diff 中使用的显示名；找不到时退化为 `#id`。
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

/// 把单条 `RunUpdate` 格式化为 diff 行文本：施法者、目标，以及参数或目标列表。
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

/// 规整 diff 行：去掉内部计数器与方括号标记后 trim。
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

/// 推进最多 max_rounds 个回合并收集 diff 行，返回 (行, 回合数, 总分数)；normalize 控制是否规整行文本。
pub fn collect_runtime_diff_lines(runner: &mut RuntimeRunner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize, u64) {
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
