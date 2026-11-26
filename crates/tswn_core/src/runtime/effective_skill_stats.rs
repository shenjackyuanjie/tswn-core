//! 传统主动技扫描的可选统计。观察器只读实际选技结果，不推进 RNG。

use std::cell::RefCell;

use serde::Serialize;
use smallvec::SmallVec;

use super::{BuiltinActiveSkill, CombatRuntime, EntityIdx};

thread_local! {
    static ACTIVE: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

/// 线程本地观察器：记录统计主体、根实体，以及按（技能、是否分身、是否聪明）三维的累计计数。
struct Observer {
    subject: String,
    roots: SmallVec<[EntityIdx; 2]>,
    counters: [[[SkillCounts; BuiltinActiveSkill::CORE.len()]; 2]; 2],
}

/// 单个技能在一次扫描中的累计计数，按本体/分身与聪明/普通分别统计。
#[derive(Clone, Copy, Default, Serialize)]
pub struct SkillCounts {
    pub reached: u64,
    /// 每次扫描按当前等级累加，以 128 为分母，包含门槛挡住的机会。
    pub nominal_128: u64,
    pub eligible_128: u64,
    pub gate_failed: u64,
    pub hit: u64,
    pub committed: u64,
    pub gate_to_skill: u64,
    pub no_target_to_skill: u64,
}

/// 一个（技能、是否分身、是否聪明）组合的观察结果，仅包含实际到达过的条目。
#[derive(Serialize)]
pub struct SkillObservation {
    skill: &'static str,
    descendant: bool,
    smart: bool,
    #[serde(flatten)]
    counts: SkillCounts,
}

/// 在当前线程的批量区间入口统计第一输入组中的指定名字及其分身。
pub fn observe<T>(subject: &str, run: impl FnOnce() -> T) -> (T, Vec<SkillObservation>) {
    /// [`observe`] 的作用域守卫：退出时清除线程本地观察器，保证统计不嵌套。
    struct Scope;
    impl Drop for Scope {
        /// 守卫析构：取出并丢弃当前线程的观察器。
        fn drop(&mut self) { ACTIVE.with(|active| active.borrow_mut().take()); }
    }

    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        assert!(active.is_none(), "技能统计不可嵌套");
        *active = Some(Observer {
            subject: crate::namerena::raw_namerena_to_id_name(subject),
            roots: SmallVec::new(),
            counters: [[[SkillCounts::default(); BuiltinActiveSkill::CORE.len()]; 2]; 2],
        });
    });
    let _scope = Scope;
    let result = run();
    let observer = ACTIVE.with(|active| active.borrow_mut().take().unwrap());
    let mut observations = Vec::new();
    for (descendant, modes) in observer.counters.into_iter().enumerate() {
        for (smart, counters) in modes.into_iter().enumerate() {
            for (skill, counts) in BuiltinActiveSkill::CORE.into_iter().zip(counters) {
                if counts.reached > 0 {
                    observations.push(SkillObservation {
                        skill: skill.export_name(),
                        descendant: descendant != 0,
                        smart: smart != 0,
                        counts,
                    });
                }
            }
        }
    }
    (result, observations)
}

pub fn set_first_group(group: &[EntityIdx]) {
    ACTIVE.with(|active| {
        if let Some(observer) = active.borrow_mut().as_mut() {
            observer.roots.clear();
            observer.roots.extend_from_slice(group);
        }
    });
}

#[derive(Clone, Copy)]
pub enum ScanResult {
    GateFailed,
    Miss,
    NoTarget,
    Committed,
}

pub struct Scan {
    actor_mode: Option<(usize, usize)>,
    attempts: SmallVec<[(BuiltinActiveSkill, u32, ScanResult); 8]>,
}

impl Scan {
    pub fn new(runtime: &CombatRuntime, actor: EntityIdx, smart: bool) -> Self {
        let actor_mode = ACTIVE.with(|active| {
            let active = active.borrow();
            let observer = active.as_ref()?;
            let entity = runtime.entities.get(actor)?;
            if entity.runtime.is_combat_minion() {
                return None;
            }
            let root = entity.runtime.root_owner;
            if observer.roots.contains(&root) && runtime.entities.get(root)?.template.id_key_name == observer.subject {
                return Some((usize::from(root != actor), usize::from(smart)));
            }
            None
        });
        Self {
            actor_mode,
            attempts: SmallVec::new(),
        }
    }

    pub fn enabled(&self) -> bool { self.actor_mode.is_some() }

    pub fn record(&mut self, skill: BuiltinActiveSkill, level: u32, result: ScanResult) {
        if self.enabled() {
            self.attempts.push((skill, level, result));
        }
    }

    pub fn finish(self, selected_skill: bool) {
        let Some((descendant, smart)) = self.actor_mode else { return };
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let counters = &mut active.as_mut().unwrap().counters[descendant][smart];
            for (skill, level, result) in self.attempts {
                let Some(counts) = counters.get_mut(skill.legacy_key()) else {
                    continue;
                };
                let level = u64::from(level.min(128));
                counts.reached += 1;
                counts.nominal_128 += level;
                if matches!(result, ScanResult::GateFailed) {
                    counts.gate_failed += 1;
                    counts.gate_to_skill += u64::from(selected_skill);
                    continue;
                }
                counts.eligible_128 += level;
                counts.hit += u64::from(matches!(result, ScanResult::NoTarget | ScanResult::Committed));
                counts.committed += u64::from(matches!(result, ScanResult::Committed));
                counts.no_target_to_skill += u64::from(matches!(result, ScanResult::NoTarget) && selected_skill);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
    use crate::runtime::runtime_score_range;

    #[test]
    fn observes_missing_revive_targets_without_changing_results() {
        let subject = r#"owner+ol:{"skills":{"sklrevive":128,"sklfire":128}}"#.to_owned();
        let team = [subject.clone(), subject];
        let score = || runtime_score_range(&team, "!", 1, 65, WIN_RATE_EVAL_RQ).unwrap();
        let baseline = score();
        let (with_stats, observations) = observe(&team[0], score);
        assert_eq!((with_stats.wins, with_stats.total), (baseline.wins, baseline.total));
        let revive = observations.iter().filter(|row| row.skill == "core.skill.revive");
        assert!(revive.clone().map(|row| row.counts.hit).sum::<u64>() > 0);
        assert_eq!(revive.clone().map(|row| row.counts.committed).sum::<u64>(), 0);
        assert!(revive.map(|row| row.counts.no_target_to_skill).sum::<u64>() > 0);
    }

    #[test]
    fn observes_first_input_member_even_when_seed_reorders_team() {
        use crate::runtime::{RuntimeRunner, prepared_runtime_win_rate_range};

        let groups = [
            vec![
                r#"owner+ol:{"skills":{"sklfire":128}}"#.to_owned(),
                r#"mate+ol:{"skills":{"sklice":128}}"#.to_owned(),
            ],
            vec!["enemy".to_owned()],
        ];
        let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, WIN_RATE_EVAL_RQ).unwrap();
        let (_, observations) = observe(&groups[0][0], || prepared_runtime_win_rate_range(&prepared, 1, 65).unwrap());
        assert!(observations.iter().any(|row| row.skill == "core.skill.fire"));
        assert!(!observations.iter().any(|row| row.skill == "core.skill.ice"));
    }
}
