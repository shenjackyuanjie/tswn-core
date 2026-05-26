//! 对战日志格式化与 trace 归一化逻辑。
//!
//! 这是 `fight.rs` 里噪声最多、但又必须精确保留行为的一块：
//! - 普通文本输出需要把 update 模板替换成人名；
//! - diff 输出要做一套更保守的归一化；
//! - raw 输出还要给幻影、召唤物分配稳定名字，并把连续事件重新聚合成 runner 兼容格式。
//!
//! 把这些细节单独关进一个模块，目的是让入口函数重新短小，同时把“为什么要这样归一化”
//! 的注释放在真正相关的实现旁边，而不是散落在命令驱动代码之间。

use std::collections::HashMap;

use tswn_core::engine;
use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::Runner;

use super::driver::fmt_winner_input_indices;

/// raw trace 下为召唤物分配稳定名字时维护的状态。
///
/// 之所以需要单独状态机，而不是每次现算，是因为同一个召唤物在多条 update 中都要映射到
/// 同一个名字；尤其是 summon / shadow 这种对象，如果名字不稳定，diff 会完全对不上。
#[derive(Default)]
struct TraceNameState {
    assigned: HashMap<usize, String>,
    next_index: HashMap<usize, usize>,
    summon_name: HashMap<usize, String>,
}

/// 追溯召唤物的根 owner。
fn root_trace_owner_id(storage: &engine::storage::Storage, start_id: usize) -> usize {
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let mut current = start_id;
    loop {
        let Some(plr) = storage.get_player_or_pending(&current) else {
            return current;
        };
        let Some(minion) = plr.get_state::<MinionRuntimeState>() else {
            return current;
        };
        let Some(owner) = minion.owner else {
            return current;
        };
        current = owner;
    }
}

/// 为同一根 owner 的第 N 个 minion 生成稳定展示名。
fn format_trace_minion_name(owner: &tswn_core::player::Player, index: usize) -> String {
    let base = format!("{}?{}", owner.id_name(), index);
    let team = owner.clan_name();
    if !team.is_empty() && team != owner.id_name() {
        format!("{base}@{team}")
    } else {
        base
    }
}

/// 分配一个新的 minion 稳定名，并推进 owner 的计数器。
fn alloc_trace_minion_name(trace_names: &mut TraceNameState, root_owner_id: usize, owner: &tswn_core::player::Player) -> String {
    let index = trace_names.next_index.entry(root_owner_id).or_insert(0);
    let name = format_trace_minion_name(owner, *index);
    *index += 1;
    name
}

/// 普通输出里将玩家 ID 解析成展示名。
fn plr_name(runner: &Runner, id: usize) -> String {
    runner
        .storage
        .get_player_or_pending(&id)
        .map(|plr| plr.id_key_name())
        .unwrap_or_else(|| format!("#{id}"))
}

/// 按普通文本输出的口径格式化一条 update。
fn fmt_update_with_mode(runner: &Runner, update: &RunUpdate, append_score: bool) -> String {
    let caster = plr_name(runner, update.caster);
    let target = plr_name(runner, update.target);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update.targets.iter().map(|id| plr_name(runner, *id)).collect::<Vec<String>>().join(",")
    };

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg = msg.replace("[2]", &targets);
    if append_score && update.score > 0 {
        format!("{msg}  (+{})", update.score)
    } else {
        msg
    }
}

/// 普通 `fight` 输出使用的格式化函数。
pub(super) fn fmt_update(runner: &Runner, update: &RunUpdate) -> String { fmt_update_with_mode(runner, update, true) }

/// diff 输出使用更保守的名字替换规则。
fn fmt_update_diff(runner: &Runner, update: &RunUpdate) -> String {
    let caster = runner
        .storage
        .get_player(&update.caster)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.caster));
    let target = runner
        .storage
        .get_player(&update.target)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.target));
    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    let param = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| {
                runner
                    .storage
                    .get_player(id)
                    .map(|plr| plr.display_name())
                    .unwrap_or_else(|| format!("#{id}"))
            })
            .collect::<Vec<String>>()
            .join(",")
    };
    msg.replace("[2]", &param)
}

/// raw trace 的名字解析器。
///
/// 这里和普通输出最大的区别，是 summon / shadow 需要分配稳定的人工名字，否则两次输出
/// 很难做 diff。该函数一旦给某个 ID 分配了名字，后续都会复用同一项缓存。
fn plr_name_raw(runner: &Runner, id: usize, trace_names: &mut TraceNameState) -> String {
    if let Some(name) = trace_names.assigned.get(&id) {
        return name.clone();
    }

    let Some(plr) = runner.storage.get_player_or_pending(&id) else {
        return format!("#{id}");
    };

    use tswn_core::player::PlayerType;
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let name = if plr.player_type() == PlayerType::Boss {
        plr.display_name()
    } else if let Some(minion) = plr.get_state::<MinionRuntimeState>() {
        if let Some(owner_id) = minion.owner {
            let root_owner_id = root_trace_owner_id(&runner.storage, owner_id);
            if let Some(owner) = runner.storage.get_player_or_pending(&root_owner_id) {
                use tswn_core::player::skill::act::minion::MinionKind;
                if minion.kind == MinionKind::Summon {
                    if let Some(name) = trace_names.summon_name.get(&owner_id) {
                        name.clone()
                    } else {
                        let name = alloc_trace_minion_name(trace_names, root_owner_id, owner);
                        trace_names.summon_name.insert(owner_id, name.clone());
                        name
                    }
                } else {
                    alloc_trace_minion_name(trace_names, root_owner_id, owner)
                }
            } else {
                plr.id_key_name()
            }
        } else {
            plr.id_key_name()
        }
    } else {
        plr.id_key_name()
    };

    trace_names.assigned.insert(id, name.clone());
    name
}

/// raw 输出模式下格式化一条 update。
///
/// “召唤出幻影” 这类消息需要特殊处理，因为目标 ID 往往是延迟挂进 storage / pending 列表里的。
fn fmt_update_raw_with_state(runner: &Runner, update: &RunUpdate, trace_names: &mut TraceNameState) -> String {
    let caster = plr_name_raw(runner, update.caster, trace_names);
    let mut target = plr_name_raw(runner, update.target, trace_names);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| plr_name_raw(runner, *id, trace_names))
            .collect::<Vec<String>>()
            .join(",")
    };

    if update.message == "召唤出幻影" {
        use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};

        let root_owner_id = root_trace_owner_id(&runner.storage, update.caster);
        let pending_id = runner
            .storage
            .all_player_ids()
            .into_iter()
            .chain(runner.storage.pending_spawn_ids_for_owner(update.caster))
            .find(|id| {
                !trace_names.assigned.contains_key(id)
                    && runner
                        .storage
                        .get_player_or_pending(id)
                        .and_then(|plr| plr.get_state::<MinionRuntimeState>())
                        .map(|state| {
                            state.kind == MinionKind::Shadow
                                && root_trace_owner_id(&runner.storage, state.owner.unwrap_or(*id)) == root_owner_id
                        })
                        .unwrap_or(false)
            });
        if let Some(pending_id) = pending_id {
            target = plr_name_raw(runner, pending_id, trace_names);
        }
        return format!("召唤出{target}");
    }

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg.replace("[2]", &targets)
}

/// 去掉控制字符、零宽字符和多余空白，保证 trace 更稳定。
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

/// raw trace 的归一化规则。
///
/// 这一步会去掉若干 UI 标记、全角标点和可变装饰，目的是让输出更接近 namerena/runner 对比时
/// 真正关心的“事件内容”。
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

/// diff 输出的归一化规则比 raw 更保守，尽量只删纯装饰信息。
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

/// 收集 diff 模式的全部输出行。
///
/// `normalize=true` 时会额外做一层 diff 专用归一化，尽量让输出在不同实现之间更可比较。
pub(super) fn collect_diff_lines(runner: &mut Runner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize, u64) {
    let mut lines = Vec::new();
    let mut guard = 0usize;
    let mut total_score = 0u64;
    while !runner.have_winner() && guard < max_rounds {
        let updates = runner.main_round();
        let mut parts = Vec::new();
        for update in updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                if !parts.is_empty() {
                    lines.push(parts.join(", "));
                    parts.clear();
                }
                continue;
            }
            if update.score > 0 {
                total_score += update.score as u64;
            }
            let mut msg = fmt_update_diff(runner, &update);
            if normalize {
                msg = normalize_diff_trace_line(msg);
            }
            if !msg.is_empty() {
                parts.push(msg);
            }
        }
        if !parts.is_empty() {
            lines.push(parts.join(", "));
        }
        guard += 1;
    }
    (lines, guard, total_score)
}

/// 判断一条 raw trace 是否应该作为“动作起点”单独起行。
fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

/// 在 raw 输出里把当前动作块刷入最终输出缓冲。
fn emit_current_turn(output_lines: &mut Vec<String>, pending_action_line: &mut String, pending_misc_lines: &mut Vec<String>) {
    if !pending_action_line.is_empty() {
        output_lines.push(std::mem::take(pending_action_line));
        output_lines.push(String::new());
        pending_misc_lines.clear();
        return;
    }
    if !pending_misc_lines.is_empty() {
        output_lines.push(pending_misc_lines.join(", "));
        output_lines.push(String::new());
        pending_misc_lines.clear();
    }
}

/// 打印 raw 聚合战斗日志。
///
/// 这里会把多个 update 重新折叠成更接近 runner raw diff 的格式，并在末尾补一行 `win_idx=...`。
pub(super) fn print_fight_raw(runner: &mut Runner, input_player_ids: &[usize]) {
    let mut output_lines: Vec<String> = Vec::new();
    let mut pending_action_line = String::new();
    let mut pending_misc_lines: Vec<String> = Vec::new();
    let mut trace_names = TraceNameState::default();
    let debug_raw_seq = std::env::var_os("TSWN_DEBUG_RAW_SEQ").is_some();
    let mut raw_seq_idx = 0usize;

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    while !runner.have_winner() && round <= 100_000 {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        for update in updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                if debug_raw_seq {
                    eprintln!("[raw_seq/{raw_seq_idx}] <NextLine>");
                    raw_seq_idx += 1;
                }
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                continue;
            }

            let line = normalize_trace_line(fmt_update_raw_with_state(runner, &update, &mut trace_names));
            if line.is_empty() {
                continue;
            }

            if debug_raw_seq {
                eprintln!("[raw_seq/{raw_seq_idx}] {line}");
                raw_seq_idx += 1;
            }

            if is_action_line(&line) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                pending_action_line = line;
                continue;
            }

            if pending_action_line.is_empty() {
                pending_misc_lines.push(line);
            } else {
                pending_action_line.push_str(", ");
                pending_action_line.push_str(&line);
            }
        }
        round += 1;
    }

    emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
    while matches!(output_lines.last(), Some(line) if line.is_empty()) {
        output_lines.pop();
    }

    if !output_lines.is_empty() {
        println!("{}", output_lines.join("\n"));
    }
    if let Some(win_idx_line) = fmt_winner_input_indices(runner, input_player_ids) {
        println!("{win_idx_line}");
    }
}