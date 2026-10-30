//! 非计时诊断：流式散列公开事件、行动、RNG与选定实体状态，不输出角色全名。
//! 不散列全局updates.id、指针或环境；消息关键词计数不是hook调用次数。
use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};
use tswn_core::namerena::{PlayerClass, PlayerSpec, eval_name::WIN_RATE_EVAL_RQ};
use tswn_core::runtime::{PROFILE_START, RuntimeRunner, UpdateType, prepared_runtime_win_rate_range};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(long, default_value_t = 2_000_000)]
    start: usize,
    #[arg(short = 'n', long, default_value_t = 32)]
    runs: usize,
}

fn number(hash: &mut Sha256, value: u64) { hash.update(value.to_le_bytes()); }

fn bytes(hash: &mut Sha256, value: &[u8]) {
    number(hash, value.len() as u64);
    hash.update(value);
}

fn optional(hash: &mut Sha256, value: Option<usize>) {
    hash.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        number(hash, value as u64);
    }
}

fn boundary(hash: &mut Sha256, runner: &RuntimeRunner) {
    let runtime = runner.runtime();
    bytes(hash, b"state");
    hash.update(runtime.rng.i.to_le_bytes());
    hash.update(runtime.rng.j.to_le_bytes());
    hash.update(runtime.rng.main_val);
    number(hash, runtime.round);
    number(hash, runtime.entities.iter().count() as u64);
    for (index, entity) in runtime.entities.iter() {
        hash.update(index.0.to_le_bytes());
        hash.update([u8::from(entity.runtime.alive)]);
        hash.update(entity.runtime.hp.to_le_bytes());
        number(hash, entity.runtime.team as u64);
        hash.update(entity.runtime.owner.0.to_le_bytes());
        hash.update(entity.runtime.root_owner.0.to_le_bytes());
        hash.update(entity.runtime.shield.to_le_bytes());
        hash.update(entity.runtime.magic_point.to_le_bytes());
    }
    number(hash, runtime.world.flat_alive().len() as u64);
    for index in runtime.world.flat_alive() {
        hash.update(index.0.to_le_bytes());
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.runs == 0 {
        return Err("局数必须大于零".into());
    }
    let end = args.start.checked_add(args.runs).ok_or("区间溢出")?;
    end.checked_add(PROFILE_START as usize).ok_or("seed溢出")?;
    let mut files = if args.input.is_dir() {
        std::fs::read_dir(&args.input)?
            .map(|item| item.map(|item| item.path()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![args.input]
    };
    files.retain(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "txt"));
    files.sort();
    if files.is_empty() {
        return Err("没有输入".into());
    }
    for file in files {
        let (groups, seeds) = RuntimeRunner::split_namerena_into_groups(std::fs::read_to_string(&file)?);
        if groups.is_empty() || !seeds.is_empty() {
            return Err("输入必须非空且不带seed行".into());
        }
        let mut bosses = 0;
        for raw in groups.iter().flatten() {
            let spec = PlayerSpec::parse(raw)?;
            if spec.weapon.is_some() || !matches!(spec.class, PlayerClass::Normal | PlayerClass::Boss) {
                return Err("本诊断仅接受普通或Boss角色且无武器".into());
            }
            bosses += usize::from(spec.class == PlayerClass::Boss);
        }
        let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, WIN_RATE_EVAL_RQ)?;
        let mut hash = Sha256::new();
        bytes(&mut hash, b"public-trace-recovery-v2");
        let mut wins = 0;
        let mut guards = 0;
        let mut actions = 0_u64;
        let mut messages = 0_u64;
        let mut ticks = 0_u64;
        let mut events = BTreeMap::<&str, u64>::new();
        for round in args.start..end {
            bytes(&mut hash, b"battle");
            number(&mut hash, round as u64);
            let seeds = if round == 0 {
                vec![]
            } else {
                vec![format!("seed:{}@!", PROFILE_START as usize + round)]
            };
            let mut runner = prepared.new_with_seed(&seeds).map_err(|error| format!("初始化失败: {error:?}"))?;
            runner.validate_ready()?;
            boundary(&mut hash, &runner);
            let mut winner = {
                let runtime = runner.runtime_mut();
                runtime.world.sync_winner(&runtime.entities)
            };
            let mut turns = 0;
            while winner.is_none() && turns < 100_000 {
                let outcome = runner.run_round();
                turns += 1;
                ticks += 1;
                bytes(&mut hash, b"round");
                number(&mut hash, turns);
                optional(&mut hash, outcome.winner_team);
                hash.update([u8::from(outcome.action.is_some())]);
                if let Some(action) = outcome.action {
                    actions += 1;
                    hash.update(action.actor.0.to_le_bytes());
                    hash.update(action.target.0.to_le_bytes());
                    hash.update(action.amount.to_le_bytes());
                }
                hash.update([u8::from(outcome.frame.is_some())]);
                if let Some(frame) = outcome.frame {
                    number(&mut hash, frame.updates.updates.len() as u64);
                    for update in frame.updates.updates {
                        messages += 1;
                        bytes(&mut hash, update.message.as_bytes());
                        number(&mut hash, update.caster as u64);
                        number(&mut hash, update.target as u64);
                        hash.update(update.score.to_le_bytes());
                        optional(&mut hash, update.param.map(|value| value as usize));
                        hash.update(update.delay0.to_le_bytes());
                        hash.update(update.delay1.to_le_bytes());
                        hash.update([match update.update_type {
                            UpdateType::Win => 0,
                            UpdateType::None => 1,
                            UpdateType::NextLine => 2,
                        }]);
                        number(&mut hash, update.targets.len() as u64);
                        for target in update.targets {
                            number(&mut hash, target as u64);
                        }
                        // 只输出固定类别；这些是可见文本关键词命中，不是机制调用计数。
                        for (category, keyword) in [
                            ("summon", "召唤"),
                            ("merge", "吞噬"),
                            ("revive", "复活"),
                            ("clone", "分身"),
                            ("zombie", "僵尸"),
                            ("death", "死亡"),
                            ("freeze", "冰冻"),
                            ("counter", "反击"),
                        ] {
                            if update.message.contains(keyword) {
                                *events.entry(category).or_default() += 1;
                            }
                        }
                    }
                    number(&mut hash, frame.updates.on_update_end.len() as u64);
                    for id in frame.updates.on_update_end {
                        number(&mut hash, id as u64);
                    }
                }
                boundary(&mut hash, &runner);
                winner = outcome.winner_team;
            }
            guards += usize::from(winner.is_none());
            wins += usize::from(runner.input_group_won(0));
            bytes(&mut hash, b"end");
            optional(&mut hash, winner);
        }
        let summary = prepared_runtime_win_rate_range(&prepared, args.start, end)?;
        if summary.total != args.runs || summary.wins != wins || summary.guard_exhausted != guards || summary.errors != 0 {
            return Err("capture与no-capture摘要不一致".into());
        }
        let digest = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        println!(
            "{}",
            serde_json::json!({"case": file.file_name().ok_or("输入路径没有文件名")?.to_string_lossy(), "trace_schema": "public-trace-recovery-v2", "start_inclusive": args.start, "end_exclusive": end, "group_sizes": groups.iter().map(Vec::len).collect::<Vec<_>>(), "boss_count": bosses, "weapon_count": 0, "total": args.runs, "wins": wins, "errors": 0, "guard_exhausted": guards, "rounds": ticks, "actions": actions, "messages": messages, "message_keyword_counts": events, "trace_sha256": digest, "no_capture_summary_equal": true})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_prefix_separates_strings() {
        let mut left = Sha256::new();
        bytes(&mut left, b"a");
        bytes(&mut left, b"bc");
        let mut right = Sha256::new();
        bytes(&mut right, b"ab");
        bytes(&mut right, b"c");
        assert_ne!(left.finalize().as_slice(), right.finalize().as_slice());
    }

    #[test]
    fn absent_is_not_zero() {
        let mut left = Sha256::new();
        optional(&mut left, None);
        let mut right = Sha256::new();
        optional(&mut right, Some(0));
        assert_ne!(left.finalize().as_slice(), right.finalize().as_slice());
    }
}
