use std::collections::HashMap;

use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::{engine, Runner};

pub fn run(raw: String, out_raw: bool) {
    let mut runner = match Runner::new_from_namerena_raw(raw) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };

    if out_raw {
        print_fight_raw(&mut runner);
        return;
    }

    print_all_players(&runner);

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    let mut total_score = 0u64;
    let mut score_by_caster: HashMap<usize, u64> = HashMap::new();

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

        println!("=== 回合 {round} ===");
        for update in updates.updates {
            match update.update_type {
                UpdateType::NextLine => println!(),
                _ => {
                    if update.score > 0 {
                        total_score += update.score as u64;
                        *score_by_caster.entry(update.caster).or_insert(0) += update.score as u64;
                    }
                    println!("{}", fmt_update(&runner, &update));
                }
            }
        }
        round += 1;
    }

    println!("\n=== 对局结果 ===");
    if let Some(winners) = runner.world.winner.clone() {
        println!("赢家:");
        for winner in winners {
            if let Some(plr) = runner.storage.get_player(&winner) {
                let battle_score = score_by_caster.get(&winner).copied().unwrap_or(0);
                println!(
                    "- {} (id={}, all_sum={}, battle_score={}, hp={})",
                    plr.display_name(),
                    winner,
                    plr.get_status().all_sum,
                    battle_score,
                    plr.get_status().hp
                );
            }
        }
    } else {
        println!("未分出胜负（达到安全轮次或连续空更新）。");
    }
    println!("总战斗分: {total_score}");
}

fn print_all_players(runner: &Runner) {
    println!("=== 玩家状态 ===");
    let player_ids = runner.storage.all_player_ids();
    for id in player_ids {
        if let Some(plr) = runner.storage.get_player(&id) {
            let status = plr.get_status();
            println!(
                "- {} (id={}): HP={}/{}, move_point:{} ATK={}, DEF={}, SPD={}, AGI={}, MAG={}, MP={}, MDF={}, ITL={}, all_sum={} 系数: {}",
                plr.display_name(),
                id,
                status.hp,
                status.max_hp,
                status.move_point,
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.mp,
                status.resistance,
                status.wisdom,
                status.all_sum,
                plr.get_name_factor()
            );
        }
    }
    println!();
}

#[derive(Default)]
struct TraceNameState {
    assigned: HashMap<usize, String>,
    next_index: HashMap<usize, usize>,
    summon_name: HashMap<usize, String>,
}

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

fn format_trace_minion_name(owner: &tswn_core::player::Player, index: usize) -> String {
    let base = format!("{}?{}", owner.id_name(), index);
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

fn plr_name(runner: &Runner, id: usize) -> String {
    runner
        .storage
        .get_player_or_pending(&id)
        .map(|plr| plr.id_key_name())
        .unwrap_or_else(|| format!("#{id}"))
}

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

fn fmt_update(runner: &Runner, update: &RunUpdate) -> String { fmt_update_with_mode(runner, update, true) }

fn plr_name_raw(runner: &Runner, id: usize, trace_names: &mut TraceNameState) -> String {
    if let Some(name) = trace_names.assigned.get(&id) {
        return name.clone();
    }

    let Some(plr) = runner.storage.get_player_or_pending(&id) else {
        return format!("#{id}");
    };

    use tswn_core::player::skill::act::minion::MinionRuntimeState;
    use tswn_core::player::PlayerType;

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

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

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

fn print_fight_raw(runner: &mut Runner) {
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
}
