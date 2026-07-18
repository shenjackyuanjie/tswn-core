use std::collections::HashMap;

use tswn_core::LegacyRunner as Runner;
use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, PlayerKindFlags, RuntimeRunner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustRuntime {
    Main,
    Legacy,
}

impl RustRuntime {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "main" => Ok(Self::Main),
            "legacy" => Ok(Self::Legacy),
            _ => Err(format!("未知 --runtime: {raw}，可选 main|legacy")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Legacy => "legacy",
        }
    }
}

pub fn run_rust_trace(input: &str, runtime: RustRuntime) -> Result<String, String> {
    let lines = match runtime {
        RustRuntime::Main => {
            let mut runner = tswn_core::cli_api::default_custom_runtime_mixed_runner(input)
                .map_err(|error| format!("构建 Runtime 对局失败: {error:?}"))?;
            collect_runtime_fight_raw_lines(&mut runner)
        }
        RustRuntime::Legacy => {
            let mut runner =
                Runner::new_from_namerena_raw(input.to_owned()).map_err(|error| format!("构建 legacy 对局失败: {error}"))?;
            collect_legacy_fight_raw_lines(&mut runner)
        }
    };
    Ok(lines.join("\n"))
}

#[derive(Default)]
struct TraceNameState {
    assigned: HashMap<usize, String>,
    next_index: HashMap<usize, usize>,
    /// 血祭召唤物按直接施法者复用同一个追踪名称。
    summon_name: HashMap<usize, String>,
}

fn root_trace_owner_id(storage: &tswn_core::engine::storage::Storage, start_id: usize) -> usize {
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let mut current = start_id;
    loop {
        let Some(player) = storage.get_player_or_pending(&current) else {
            return current;
        };
        let Some(minion) = player.get_state::<MinionRuntimeState>() else {
            return current;
        };
        let Some(owner) = minion.owner else {
            return current;
        };
        current = owner;
    }
}

fn format_trace_minion_name(owner: &tswn_core::player::Player, index: usize) -> String {
    let base = format!("{}?{index}", owner.id_name());
    let team = owner.clan_name();
    if !team.is_empty() && team != owner.id_name() {
        format!("{base}@{team}")
    } else {
        base
    }
}

fn alloc_trace_minion_name(trace_names: &mut TraceNameState, root_owner_id: usize, owner: &tswn_core::player::Player) -> String {
    let index = trace_names.next_index.entry(root_owner_id).or_insert(0);
    let name = format_trace_minion_name(owner, *index);
    *index += 1;
    name
}

fn legacy_raw_name(runner: &Runner, id: usize, trace_names: &mut TraceNameState) -> String {
    if let Some(name) = trace_names.assigned.get(&id) {
        return name.clone();
    }

    let Some(player) = runner.storage.get_player_or_pending(&id) else {
        return format!("#{id}");
    };

    use tswn_core::player::PlayerType;
    use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let name = if player.player_type() == PlayerType::Boss {
        player.display_name()
    } else if let Some(minion) = player.get_state::<MinionRuntimeState>() {
        if let Some(owner_id) = minion.owner {
            let root_owner_id = root_trace_owner_id(&runner.storage, owner_id);
            if let Some(owner) = runner.storage.get_player_or_pending(&root_owner_id) {
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
                player.id_key_name()
            }
        } else {
            player.id_key_name()
        }
    } else {
        player.id_key_name()
    };

    trace_names.assigned.insert(id, name.clone());
    name
}

fn fmt_legacy_update_raw(runner: &Runner, update: &RunUpdate, trace_names: &mut TraceNameState) -> String {
    let caster = legacy_raw_name(runner, update.caster, trace_names);
    let mut target = legacy_raw_name(runner, update.target, trace_names);
    let targets = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| legacy_raw_name(runner, *id, trace_names))
            .collect::<Vec<_>>()
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
                        .and_then(|player| player.get_state::<MinionRuntimeState>())
                        .is_some_and(|state| {
                            state.kind == MinionKind::Shadow
                                && root_trace_owner_id(&runner.storage, state.owner.unwrap_or(*id)) == root_owner_id
                        })
            });
        if let Some(pending_id) = pending_id {
            target = legacy_raw_name(runner, pending_id, trace_names);
        }
        return format!("召唤出{target}");
    }

    update.message.replace("[0]", &caster).replace("[1]", &target).replace("[2]", &targets)
}

fn runtime_raw_name(runner: &RuntimeRunner, id: usize) -> String {
    let Ok(entity_idx) = u32::try_from(id).map(EntityIdx) else {
        return format!("#{id}");
    };
    let Some(entity) = runner.runtime().entities.get(entity_idx) else {
        return format!("#{id}");
    };
    if entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
        return entity.template.display_name.clone();
    }
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

fn fmt_runtime_update_raw(runner: &RuntimeRunner, update: &RunUpdate) -> String {
    let caster = runtime_raw_name(runner, update.caster);
    let target = runtime_raw_name(runner, update.target);
    let targets = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| runtime_raw_name(runner, *id))
            .collect::<Vec<_>>()
            .join(",")
    };
    update.message.replace("[0]", &caster).replace("[1]", &target).replace("[2]", &targets)
}

fn sanitize_output_line(line: &str) -> String {
    let filtered = line
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect::<String>();

    let mut normalized = String::with_capacity(filtered.len());
    let mut previous_space = false;
    for ch in filtered.chars() {
        if ch.is_whitespace() {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(ch);
            previous_space = false;
        }
    }
    normalized.trim().to_owned()
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

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

fn emit_current_turn(output: &mut Vec<String>, pending_action: &mut String, pending_misc: &mut Vec<String>) {
    if !pending_action.is_empty() {
        output.push(std::mem::take(pending_action));
        output.push(String::new());
        pending_misc.clear();
    } else if !pending_misc.is_empty() {
        output.push(pending_misc.join(", "));
        output.push(String::new());
        pending_misc.clear();
    }
}

fn push_updates(
    output: &mut Vec<String>,
    pending_action: &mut String,
    pending_misc: &mut Vec<String>,
    updates: impl IntoIterator<Item = (RunUpdate, String)>,
) {
    for (update, line) in updates {
        if matches!(update.update_type, UpdateType::NextLine) {
            emit_current_turn(output, pending_action, pending_misc);
            continue;
        }
        let line = normalize_trace_line(line);
        if line.is_empty() {
            continue;
        }
        if is_action_line(&line) {
            emit_current_turn(output, pending_action, pending_misc);
            *pending_action = line;
        } else if pending_action.is_empty() {
            pending_misc.push(line);
        } else {
            pending_action.push_str(", ");
            pending_action.push_str(&line);
        }
    }
}

fn finish_output(output: &mut Vec<String>, pending_action: &mut String, pending_misc: &mut Vec<String>) {
    emit_current_turn(output, pending_action, pending_misc);
    while output.last().is_some_and(String::is_empty) {
        output.pop();
    }
}

fn collect_legacy_fight_raw_lines(runner: &mut Runner) -> Vec<String> {
    let mut output = Vec::new();
    let mut pending_action = String::new();
    let mut pending_misc = Vec::new();
    let mut trace_names = TraceNameState::default();
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
        let lines = updates.updates.into_iter().map(|update| {
            let line = fmt_legacy_update_raw(runner, &update, &mut trace_names);
            (update, line)
        });
        push_updates(&mut output, &mut pending_action, &mut pending_misc, lines);
        round += 1;
    }
    finish_output(&mut output, &mut pending_action, &mut pending_misc);
    output
}

fn collect_runtime_fight_raw_lines(runner: &mut RuntimeRunner) -> Vec<String> {
    let mut output = Vec::new();
    let mut pending_action = String::new();
    let mut pending_misc = Vec::new();
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
        let lines = frame.updates.updates.into_iter().map(|update| {
            let line = fmt_runtime_update_raw(runner, &update);
            (update, line)
        });
        push_updates(&mut output, &mut pending_action, &mut pending_misc, lines);
        round += 1;
        if finished {
            break;
        }
    }
    finish_output(&mut output, &mut pending_action, &mut pending_misc);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_selector_rejects_unknown_values() {
        assert_eq!(RustRuntime::parse("main"), Ok(RustRuntime::Main));
        assert_eq!(RustRuntime::parse("legacy"), Ok(RustRuntime::Legacy));
        assert!(RustRuntime::parse("other").is_err());
    }

    #[test]
    fn runtime_trace_matches_legacy_for_minimal_grouped_raw() {
        let raw = "left@red\n\nright@blue";
        let legacy = run_rust_trace(raw, RustRuntime::Legacy).expect("legacy trace should run");
        let runtime = run_rust_trace(raw, RustRuntime::Main).expect("Runtime trace should run");
        assert!(!runtime.is_empty());
        assert_eq!(runtime, legacy);
    }
}
