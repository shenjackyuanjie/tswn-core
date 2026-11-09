//! 规范 core BattleSession 和 DTO 之上的轻量 Python 适配器。
//!
//! 会话独占运行时，raw 只在构造时读取；DTO 经 JSON 转为新的 Python dict/list，
//! 不暴露 Rust 借用。领域错误由 map_cli_error 转换，JSON/Python 对象转换也可能失败；
//! next_frame 已推进后发生转换错误不会回滚运行时，因此不要据此假定该帧可重试。
use crate::cli_api::map_cli_error;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use tswn_core::cli_api::{BattleOptions, BattleSession};
use tswn_core::runtime::{BuiltinActiveSkill, PassiveDecision, PlannedAction, PlannedSkill};

pub(crate) fn options(
    eval_rq: Option<f64>,
    include_icons: bool,
    include_skills: bool,
    controlled_teams: Option<Vec<usize>>,
    allow_future_actors: bool,
    max_rounds: Option<usize>,
    passive_decisions: Option<Vec<(usize, String, bool, bool, Option<usize>)>>,
    passive_decision_mode: bool,
    controlled_minions: bool,
) -> BattleOptions {
    let defaults = BattleOptions::default();
    BattleOptions {
        eval_rq: eval_rq.unwrap_or(defaults.eval_rq),
        include_icons,
        include_skills,
        controlled_teams: controlled_teams.unwrap_or_default(),
        allow_future_actors,
        max_rounds: max_rounds.unwrap_or(defaults.max_rounds),
        passive_decisions: passive_decisions
            .unwrap_or_default()
            .into_iter()
            .map(|(actor, skill, activate, success, target)| PassiveDecision {
                actor,
                skill,
                activate,
                success,
                target,
            })
            .collect(),
        passive_decision_mode,
        controlled_minions,
        ..defaults
    }
}

/// 把 Python 侧的技能名转换成 core 的行动技能。
///
/// 接受 `basic` / `magic` 以及内置主动技能的导出名（`core.skill.fire`）。
pub(crate) fn parse_planned_skill(value: &str) -> PyResult<PlannedSkill> {
    match value.trim().to_ascii_lowercase().as_str() {
        "basic" | "basic_attack" | "attack" => Ok(PlannedSkill::BasicAttack),
        "magic" | "magic_attack" => Ok(PlannedSkill::MagicAttack),
        name => BuiltinActiveSkill::from_export_name(name)
            .or_else(|| BuiltinActiveSkill::ALL.iter().copied().find(|skill| skill.local_name() == name))
            .map(PlannedSkill::Builtin)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err(format!("unknown skill: {value}"))),
    }
}

pub(crate) fn parse_planned_action(actor: usize, skill: &str, magic_cost: Option<u8>) -> PyResult<PlannedAction> {
    Ok(PlannedAction {
        actor,
        skill: parse_planned_skill(skill)?,
        magic_cost,
    })
}

pub(crate) fn dto_to_python<T: serde::Serialize + ?Sized>(py: Python<'_>, dto: &T) -> PyResult<Py<PyAny>> {
    let json = serde_json::to_string(dto)
        .map_err(|error| map_cli_error(tswn_core::cli_api::CliApiError::Internal(error.to_string())))?;
    Ok(PyModule::import(py, "json")?.getattr("loads")?.call1((json,))?.unbind())
}

#[pyclass(name = "BattleSession")]
/// 可迭代的同步战斗会话，返回 core 规范 DTO 的 dict/list 副本。
///
/// Python 管理对象释放，无需显式 free；迭代耗尽与失败不同，Runtime 失败会抛出
/// 带 code 的异常并保留 sticky failure，此后停止推进并丢弃会话。
pub struct PyBattleSession {
    inner: BattleSession,
}

#[pymethods]
impl PyBattleSession {
    /// 解析空行分队的 raw；max_rounds 为正数回合预算，None 使用 core 默认值。
    /// include_icons 会把图标 data URL 加入快照，eval_rq 仅在创建时确定。
    /// include_skills 会附带技能熟练度与主动技能行动顺序。
    /// team_upgrades 控制初始输入角色的同组同 clan 强化，不影响名字系数。
    /// controlled_teams 中的队伍会在缺指令时暂停；actions 为 `(actor, skill, magic_cost)`。
    #[new]
    #[pyo3(signature = (
        raw,
        eval_rq=None,
        include_icons=false,
        include_skills=false,
        controlled_teams=None,
        actions=None,
        max_rounds=None,
        allow_future_actors=false,
        passive_decisions=None,
        passive_decision_mode=false,
        controlled_minions=false,
        *,
        team_upgrades=true,
        skill_multiplier=1.0
    ))]
    fn new(
        raw: &str,
        eval_rq: Option<f64>,
        include_icons: bool,
        include_skills: bool,
        controlled_teams: Option<Vec<usize>>,
        actions: Option<Vec<(usize, String, Option<u8>)>>,
        max_rounds: Option<usize>,
        allow_future_actors: bool,
        passive_decisions: Option<Vec<(usize, String, bool, bool, Option<usize>)>>,
        passive_decision_mode: bool,
        controlled_minions: bool,
        team_upgrades: bool,
        skill_multiplier: f64,
    ) -> PyResult<Self> {
        let mut options = options(
            eval_rq,
            include_icons,
            include_skills,
            controlled_teams,
            allow_future_actors,
            max_rounds,
            passive_decisions,
            passive_decision_mode,
            controlled_minions,
        );
        options.team_upgrades = team_upgrades;
        options.skill_multiplier = skill_multiplier;
        let planned = actions
            .unwrap_or_default()
            .into_iter()
            .map(|(actor, skill, magic_cost)| parse_planned_action(actor, &skill, magic_cost))
            .collect::<PyResult<Vec<_>>>()?;
        let inner = if planned.is_empty() {
            BattleSession::new(raw, options).map_err(map_cli_error)?
        } else {
            BattleSession::new_with_actions(raw, options, &planned).map_err(map_cli_error)?
        };
        Ok(Self { inner })
    }
    /// 追加外部指令；下一次轮到对应实体时生效。
    fn queue_actions(&mut self, actions: Vec<(usize, String, Option<u8>)>) -> PyResult<()> {
        let planned = actions
            .into_iter()
            .map(|(actor, skill, magic_cost)| parse_planned_action(actor, &skill, magic_cost))
            .collect::<PyResult<Vec<_>>>()?;
        self.inner.queue_actions(&planned).map_err(map_cli_error)
    }
    /// 受控单位暂停时等待指令的实体 ID；非暂停边界为 None。
    fn paused_actor(&self) -> Option<usize> { self.inner.paused_actor() }
    /// 缺指令暂停仍需引擎强制狂暴行动时返回 "berserk"；其他边界为 None，不推进会话。
    fn paused_forced_action(&self) -> Option<&'static str> {
        self.inner.paused_forced_action().map(tswn_core::runtime::ForcedAction::as_str)
    }
    /// 预读下一条主动技的 MP 消耗（8~23），不推进随机数。
    fn peek_magic_cost(&self) -> Option<u8> { self.inner.peek_magic_cost() }
    /// 被动 hook 待确认请求；没有时为 None。
    fn pending_passive(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let Some(request) = self.inner.pending_passive() else {
            return Ok(None);
        };
        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("actor", request.actor)?;
        dict.set_item("initiator", request.initiator)?;
        dict.set_item("mp_costs", request.mp_costs.clone())?;
        dict.set_item("target", request.target)?;
        dict.set_item("skill", request.skill.as_str())?;
        dict.set_item("level", request.level)?;
        dict.set_item("candidates", request.candidates.clone())?;
        dict.set_item("unavoidable", request.unavoidable)?;
        dict.set_item("intercept", request.intercept)?;
        dict.set_item("batch_id", request.batch_id)?;
        let phase = request.phase.map(|phase| match phase {
            tswn_core::runtime::DamageDecisionPhase::Evade => "evade",
            tswn_core::runtime::DamageDecisionPhase::Reflect => "reflect",
            tswn_core::runtime::DamageDecisionPhase::Protect => "protect",
            tswn_core::runtime::DamageDecisionPhase::Defend => "defend",
        });
        dict.set_item("phase", phase)?;
        if let Some(thunder) = self.inner.pending_thunder() {
            let context = pyo3::types::PyDict::new(py);
            context.set_item("id", thunder.id)?;
            context.set_item("attacker", thunder.attacker)?;
            context.set_item("target", thunder.target)?;
            context.set_item("hit", thunder.hit)?;
            context.set_item("is_evade", thunder.is_evade)?;
            context.set_item("previous_level", thunder.previous_level)?;
            dict.set_item("thunder", context)?;
        }
        Ok(Some(dict.into_any().unbind()))
    }
    /// 预览指定实体使用某个技能会选中的目标 ID；不推进随机数。
    fn preview_targets(&mut self, actor: usize, skill: &str) -> PyResult<Vec<usize>> {
        let skill = parse_planned_skill(skill)?;
        self.inner.preview_targets(actor, skill).map_err(map_cli_error)
    }
    /// 返回创建时快照的新副本；修改 Python 结果不会修改会话。
    fn initial_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.initial_states()) }
    /// 返回当前回合边界的快照副本，不推进运行时。
    fn current_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.current_states()) }
    /// 推进到下一可见帧；正常终止返回 None，Runtime 失败抛异常而不是 StopIteration。
    fn next_frame(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner
            .next_frame()
            .map_err(map_cli_error)?
            .map(|frame| dto_to_python(py, &frame))
            .transpose()
    }
    /// 重演到暂停/终局，仅把指定前缀之后的帧转换为 Python 对象。
    /// 历史仍完整执行，随机流、错误和暂停边界与逐次 next_frame 完全一致。
    #[pyo3(signature = (start_frame=0))]
    fn drain_frames(&mut self, py: Python<'_>, start_frame: usize) -> PyResult<(usize, Vec<Py<PyAny>>)> {
        let mut frames = Vec::new();
        while let Some(frame) = self.inner.next_frame().map_err(map_cli_error)? {
            if frame.frame_index >= start_frame {
                frames.push(dto_to_python(py, &frame)?);
            }
        }
        Ok((self.inner.frames_emitted(), frames))
    }
    /// 正常结束或截断后返回结果副本；运行中/失败时为 None，重复读取不消费结果。
    fn result(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner.result().map(|result| dto_to_python(py, &result)).transpose()
    }
    fn status(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.status()) }
    fn stop_reason(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.stop_reason()) }
    /// 只表示正常结束或截断；Runtime 失败时仍为 false，应同时检查 is_failed。
    fn is_done(&self) -> bool { self.inner.is_done() }
    fn is_failed(&self) -> bool { self.inner.is_failed() }
    fn is_finished(&self) -> bool { self.inner.is_finished() }
    fn is_truncated(&self) -> bool { self.inner.is_truncated() }
    fn rounds_advanced(&self) -> usize { self.inner.rounds_advanced() }
    fn frames_emitted(&self) -> usize { self.inner.frames_emitted() }
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> { self.next_frame(py) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draining_suffix_preserves_frames_and_final_state() {
        Python::initialize();
        Python::attach(|py| {
            let open = || {
                PyBattleSession::new(
                    "alpha@red\n\nbeta@blue\n\nseed:instant-frame-suffix@!",
                    None,
                    false,
                    true,
                    None,
                    None,
                    Some(100),
                    false,
                    None,
                    false,
                    false,
                    true,
                    1.0,
                )
                .unwrap()
            };
            let mut full = open();
            let mut expected = Vec::new();
            while let Some(frame) = full.next_frame(py).unwrap() {
                expected.push(frame);
            }
            assert!(!expected.is_empty());
            for start in [0, expected.len() / 2, expected.len(), expected.len() + 3] {
                let mut suffix = open();
                let (count, actual) = suffix.drain_frames(py, start).unwrap();
                assert_eq!(count, expected.len());
                let tail = &expected[start.min(expected.len())..];
                assert_eq!(actual.len(), tail.len());
                for (a, b) in actual.iter().zip(tail) {
                    assert!(a.bind(py).eq(b.bind(py)).unwrap());
                }
                assert!(
                    suffix
                        .current_states(py)
                        .unwrap()
                        .bind(py)
                        .eq(full.current_states(py).unwrap().bind(py))
                        .unwrap()
                );
                assert_eq!(suffix.paused_actor(), full.paused_actor());
                assert_eq!(suffix.peek_magic_cost(), full.peek_magic_cost());
                assert_eq!(suffix.inner.status(), full.inner.status());
            }
        });
    }

    #[test]
    fn python_session_exposes_real_sticky_runtime_failure() {
        Python::initialize();
        Python::attach(|py| {
            let mut session = PyBattleSession::new(
                "alpha@red+bed2[3000]\n\nbeta@blue",
                None,
                false,
                false,
                None,
                None,
                None,
                false,
                None,
                false,
                false,
                true,
                1.0,
            )
            .unwrap();
            session.inner.invalidate_runtime_for_test();
            let object = Py::new(py, session).unwrap();
            let object = object.bind(py);
            assert!(!object.call_method0("is_failed").unwrap().extract::<bool>().unwrap());
            let mut previous_error = None;
            for _ in 0..2 {
                let error = object.call_method0("next_frame").unwrap_err();
                let code = error.value(py).getattr("code").unwrap().extract::<String>().unwrap();
                assert_eq!(code, "RUNTIME_FAILED");
                let current = (code, error.to_string());
                if let Some(previous) = &previous_error {
                    assert_eq!(&current, previous);
                }
                previous_error = Some(current);
                assert!(object.call_method0("is_failed").unwrap().extract::<bool>().unwrap());
                assert!(!object.call_method0("is_done").unwrap().extract::<bool>().unwrap());
                assert!(object.call_method0("result").unwrap().is_none());
                assert!(object.call_method0("paused_forced_action").unwrap().is_none());
                assert!(object.call_method0("stop_reason").unwrap().is_none());
                assert_eq!(object.call_method0("status").unwrap().extract::<String>().unwrap(), "running");
            }
        });
    }
}
