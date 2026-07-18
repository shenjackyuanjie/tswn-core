//! 面向语言绑定的主 Runtime 会话查询层。
//!
//! 这里把 Python、C 和 WASM 都需要的只读状态统一投影为纯数据，避免绑定层
//! 重新依赖旧 `Player` / `Storage` / `WorldState` 对象模型。

use super::*;

/// 未显式提供 guard 的语言绑定统一使用的最大主回合数。
pub const BINDING_COMPLETION_MAX_ROUNDS: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMinionKind {
    Clone,
    Summon,
    Shadow,
    Zombie,
}

impl RuntimeMinionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clone => "clone",
            Self::Summon => "summon",
            Self::Shadow => "shadow",
            Self::Zombie => "zombie",
        }
    }
}

/// 一名 Runtime 实体对语言绑定公开的标准快照。
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePlayerSnapshot {
    pub id: PlrId,
    pub team_index: usize,
    pub input_team_index: Option<usize>,
    pub owner_id: Option<PlrId>,
    pub root_owner_id: Option<PlrId>,
    pub id_name: String,
    pub id_key_name: String,
    pub display_name: String,
    pub display_index: usize,
    pub base_name: String,
    pub player_type: &'static str,
    pub minion_kind: Option<RuntimeMinionKind>,
    pub hp: i32,
    pub max_hp: i32,
    pub magic_point: i32,
    pub move_point: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub agility: i32,
    pub magic: i32,
    pub resistance: i32,
    pub wisdom: i32,
    pub point: u32,
    pub all_sum: u32,
    pub name_factor: f64,
    pub at_boost: f64,
    pub attract: f64,
    pub frozen: bool,
    pub alive: bool,
    pub active: bool,
    pub status_labels: Vec<String>,
}

impl RuntimeRunner {
    /// 按 namerena 文本规则拆出原始队伍和 seed 行。
    pub fn split_namerena_into_groups(raw_input: String) -> (Vec<Vec<String>>, Vec<String>) {
        PreparedBattleInit::split_namerena_raw(raw_input)
    }

    pub fn new_from_groups_with_seed(groups: &[Vec<String>], seed: &[String]) -> Result<Self, RuntimeBuildError> {
        Self::new_from_groups_with_seed_and_eval_rq(groups, seed, crate::player::eval_name::DEFAULT_EVAL_RQ)
    }

    pub fn new_from_groups_with_seed_and_eval_rq(
        groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
    ) -> Result<Self, RuntimeBuildError> {
        let prepared = Self::prepare_groups_with_eval_rq(groups, eval_rq)?;
        prepared.new_with_seed(seed).map_err(Into::into)
    }

    pub fn prepare_groups(groups: &[Vec<String>]) -> Result<PreparedRuntimeRunner, RuntimeBuildError> {
        Self::prepare_groups_with_eval_rq(groups, crate::player::eval_name::DEFAULT_EVAL_RQ)
    }

    pub fn prepare_groups_with_eval_rq(groups: &[Vec<String>], eval_rq: f64) -> Result<PreparedRuntimeRunner, RuntimeBuildError> {
        let config = default_custom_runtime_import_config()?;
        PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(groups, eval_rq, config).map_err(Into::into)
    }

    pub fn new_from_prepared_with_seed(prepared: &PreparedRuntimeRunner, seed: &[String]) -> Result<Self, RuntimeBuildError> {
        prepared.new_with_seed(seed).map_err(Into::into)
    }

    /// 推进一个主 Runtime 回合并取出可见更新。
    pub fn main_round(&mut self) -> RunUpdates { self.run_round().frame.map_or_else(RunUpdates::new, |frame| frame.updates) }

    /// 使用语言绑定统一 guard 跑到结束，返回是否产生胜者。
    pub fn run_binding_to_completion(&mut self) -> bool {
        self.run_to_completion(BINDING_COMPLETION_MAX_ROUNDS).winner_team.is_some()
    }

    pub fn have_winner(&self) -> bool { self.runtime.world.winner_team().is_some() }

    pub fn winner_team_indices(&self) -> Vec<usize> {
        (0..self.input_groups.len()).filter(|index| self.input_group_won(*index)).collect()
    }

    pub fn winner_team_index(&self) -> Option<usize> { self.winner_team_indices().into_iter().next() }

    pub fn winner_ids(&self) -> Vec<PlrId> {
        self.runtime
            .world
            .winner_team()
            .and_then(|team| self.runtime.world.team_roster(team))
            .unwrap_or_default()
            .iter()
            .map(|entity| entity.0 as PlrId)
            .collect()
    }

    pub fn all_player_ids(&self) -> Vec<PlrId> { self.runtime.entities.iter().map(|(entity, _)| entity.0 as PlrId).collect() }

    pub fn alive_player_ids(&self) -> Vec<PlrId> {
        self.runtime.world.flat_alive().iter().map(|entity| entity.0 as PlrId).collect()
    }

    pub fn alive_player_groups(&self) -> Vec<Vec<PlrId>> {
        (0..self.input_groups.len())
            .map(|team| {
                self.runtime
                    .world
                    .team_alive(team)
                    .unwrap_or_default()
                    .iter()
                    .map(|entity| entity.0 as PlrId)
                    .collect()
            })
            .collect()
    }

    pub fn player_snapshot(&self, id: PlrId) -> Option<RuntimePlayerSnapshot> {
        let entity_idx = EntityIdx(id.try_into().ok()?);
        let entity = self.runtime.entities.get(entity_idx)?;
        Some(self.snapshot_entity(entity_idx, entity))
    }

    pub fn player_snapshots(&self) -> Vec<RuntimePlayerSnapshot> {
        self.runtime
            .entities
            .iter()
            .map(|(entity_idx, entity)| self.snapshot_entity(entity_idx, entity))
            .collect()
    }

    fn snapshot_entity(&self, entity_idx: EntityIdx, entity: &EntityRecord) -> RuntimePlayerSnapshot {
        let minion_kind = self.minion_kind(entity);
        let is_minion = minion_kind.is_some();
        let input_team_index = self.input_groups.iter().position(|group| group.contains(&entity_idx)).or_else(|| {
            is_minion
                .then(|| entity.runtime.root_owner)
                .and_then(|root_owner| self.input_groups.iter().position(|group| group.contains(&root_owner)))
        });
        let clone_build = entity.template.clone_build.as_ref();
        RuntimePlayerSnapshot {
            id: entity_idx.0 as PlrId,
            team_index: entity.runtime.team,
            input_team_index,
            owner_id: is_minion.then_some(entity.runtime.owner.0 as PlrId),
            root_owner_id: is_minion.then_some(entity.runtime.root_owner.0 as PlrId),
            id_name: entity.template.name.clone(),
            id_key_name: entity.template.id_key_name.clone(),
            display_name: entity.template.display_name.clone(),
            display_index: minion_display_index_for_entity(Some(entity)),
            base_name: entity.template.name.clone(),
            player_type: self.player_type(entity, minion_kind),
            minion_kind,
            hp: entity.runtime.hp,
            max_hp: entity.template.max_hp,
            magic_point: entity.runtime.magic_point,
            move_point: entity.runtime.move_state.speed_points,
            attack: entity.runtime.attack,
            defense: entity.runtime.defense,
            speed: entity.runtime.speed,
            agility: entity.runtime.agility,
            magic: entity.runtime.magic,
            resistance: entity.runtime.resistance,
            wisdom: entity.runtime.wisdom,
            point: 0,
            all_sum: clone_build.map_or(
                entity.template.attr_sum.saturating_mul(3) + entity.template.max_hp.max(0) as u32,
                CloneBuildData::all_sum,
            ),
            name_factor: clone_build.map_or(0.0, CloneBuildData::name_factor),
            at_boost: entity.runtime.at_boost(),
            attract: entity.runtime.attract(),
            frozen: entity.states.is_frozen(),
            alive: entity.runtime.alive,
            active: entity.is_active(),
            status_labels: status_labels(entity),
        }
    }

    fn minion_kind(&self, entity: &EntityRecord) -> Option<RuntimeMinionKind> {
        let export_name = self.runtime.registry.player_kind(entity.runtime.kind)?.export_name.as_str();
        match export_name {
            DEFAULT_CORE_CLONE_KIND_EXPORT => Some(RuntimeMinionKind::Clone),
            DEFAULT_CORE_SUMMON_KIND_EXPORT | DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT => Some(RuntimeMinionKind::Summon),
            DEFAULT_CORE_SHADOW_KIND_EXPORT | DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT => Some(RuntimeMinionKind::Shadow),
            DEFAULT_CORE_ZOMBIE_KIND_EXPORT | DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT => Some(RuntimeMinionKind::Zombie),
            _ => None,
        }
    }

    fn player_type(&self, entity: &EntityRecord, minion_kind: Option<RuntimeMinionKind>) -> &'static str {
        if minion_kind.is_some() {
            return "Clone";
        }
        if entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            return "Boss";
        }
        if entity.runtime.flags.contains(PlayerKindFlags::BOOST) {
            return "Boost";
        }
        match entity.template.clan_name.as_str() {
            "\u{0002}" => "Test1",
            "\u{0003}" => "Test2",
            "!" => "TestEx",
            _ => "Normal",
        }
    }
}

fn push_status_label(labels: &mut Vec<String>, label: impl Into<String>) {
    let label = label.into();
    if !labels.contains(&label) {
        labels.push(label);
    }
}

fn step_suffix(step: i32) -> String { if step > 0 { format!(" ({step})") } else { String::new() } }

fn status_labels(entity: &EntityRecord) -> Vec<String> {
    let mut labels = Vec::new();
    if entity.runtime.accumulate.active {
        push_status_label(&mut labels, "聚气");
    }
    if entity.runtime.charge.active {
        push_status_label(&mut labels, format!("蓄力{}", step_suffix(entity.runtime.charge.step)));
    }
    if entity.runtime.hide.is_some() {
        push_status_label(&mut labels, "隐匿");
    }
    if let Some(assassinate) = entity.runtime.assassinate {
        push_status_label(&mut labels, format!("潜行至 #{}", assassinate.target.0));
    }
    for entry in entity.states.entries() {
        match &entry.payload {
            StatePayload::Ice { .. } => push_status_label(&mut labels, "冰冻"),
            StatePayload::Curse { multiply, .. } => {
                let suffix = if *multiply > 0 {
                    format!(" x{multiply}")
                } else {
                    String::new()
                };
                push_status_label(&mut labels, format!("诅咒{suffix}"));
            }
            StatePayload::Poison { count, .. } => push_status_label(&mut labels, format!("中毒 {count}层")),
            StatePayload::Haste { faster, .. } => {
                let suffix = if *faster > 0 { format!(" +{faster}") } else { String::new() };
                push_status_label(&mut labels, format!("疾走{suffix}"));
            }
            StatePayload::Berserk { step } => push_status_label(&mut labels, format!("狂暴{}", step_suffix(*step))),
            StatePayload::Charm { step, .. } => push_status_label(&mut labels, format!("魅惑{}", step_suffix(*step))),
            StatePayload::Slow { step } => push_status_label(&mut labels, format!("迟缓{}", step_suffix(*step))),
            StatePayload::Iron { protect, .. } => {
                let suffix = if *protect > 0 { format!(" +{protect}") } else { String::new() };
                push_status_label(&mut labels, format!("铁壁{suffix}"));
            }
            _ => {}
        }
    }
    if let Some(link) = entity.runtime.protect_from.first() {
        push_status_label(&mut labels, format!("被 #{} 守护", link.owner.0));
    }
    if let Some(target) = entity.runtime.protect_to.filter(|target| *target != entity.runtime.owner) {
        push_status_label(&mut labels, format!("守护 #{} 中", target.0));
    }
    if entity.runtime.upgrade_active {
        push_status_label(&mut labels, "垂死");
    }
    if entity.states.is_frozen() {
        push_status_label(&mut labels, "冰冻");
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_snapshot_and_completion_use_main_runtime() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("left@red\n\nright@blue\n".to_owned()).unwrap();
        let snapshots = runner.player_snapshots();
        assert_eq!(snapshots.iter().map(|player| player.id).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(
            snapshots.iter().map(|player| player.input_team_index).collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        assert!(runner.run_binding_to_completion());
        assert!(runner.have_winner());
        assert!(!runner.winner_ids().is_empty());
    }

    #[test]
    fn binding_snapshot_preserves_legacy_numeric_contract() {
        let raw = "mario@red+fire\nluigi@red+heal\n\npeach@blue+shadow\nbowser@blue+poison\n";
        let legacy = crate::LegacyRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
        let runtime = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();

        for id in legacy.all_plrs() {
            let expected = legacy.storage.get_player(&id).unwrap();
            let expected_status = expected.get_status();
            let actual = runtime.player_snapshot(id).unwrap();
            assert_eq!(actual.hp, expected_status.hp, "player {id}: hp");
            assert_eq!(actual.max_hp, expected_status.max_hp, "player {id}: max hp");
            assert_eq!(actual.magic_point, expected_status.magic_point, "player {id}: magic point");
            assert_eq!(actual.move_point, expected_status.move_point, "player {id}: move point");
            assert_eq!(actual.attack, expected_status.attack, "player {id}: attack");
            assert_eq!(actual.defense, expected_status.defense, "player {id}: defense");
            assert_eq!(actual.speed, expected_status.speed, "player {id}: speed");
            assert_eq!(actual.agility, expected_status.agility, "player {id}: agility");
            assert_eq!(actual.magic, expected_status.magic, "player {id}: magic");
            assert_eq!(actual.resistance, expected_status.resistance, "player {id}: resistance");
            assert_eq!(actual.wisdom, expected_status.wisdom, "player {id}: wisdom");
            assert_eq!(actual.point, expected_status.point, "player {id}: point");
            assert_eq!(actual.all_sum, expected_status.all_sum, "player {id}: all sum");
            assert_eq!(
                actual.name_factor.to_bits(),
                expected.get_name_factor().to_bits(),
                "player {id}: name factor"
            );
            assert_eq!(
                actual.at_boost.to_bits(),
                expected_status.at_boost.to_bits(),
                "player {id}: at boost"
            );
            assert_eq!(
                actual.attract.to_bits(),
                expected_status.attract.to_bits(),
                "player {id}: attract"
            );
            assert_eq!(actual.frozen, expected_status.frozen, "player {id}: frozen");
        }
    }
}
