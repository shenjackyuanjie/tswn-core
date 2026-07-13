use super::*;
use crate::engine::storage::Storage;
use crate::player::utils::trim_js_line_end;
use crate::player::{Player, PlayerType, PlrId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeV2BattleInitError {
    Player {
        team_index: usize,
        player_index: usize,
        raw: String,
        message: String,
    },
    EntityCountMismatch {
        prepared: usize,
        runtime: usize,
    },
}

impl std::fmt::Display for RuntimeV2BattleInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Player {
                team_index,
                player_index,
                raw,
                message,
            } => write!(
                f,
                "runtime v2 battle init failed at team {team_index}, player {player_index} ({raw:?}): {message}"
            ),
            Self::EntityCountMismatch { prepared, runtime } => {
                write!(
                    f,
                    "runtime v2 battle init entity count mismatch: prepared={prepared}, runtime={runtime}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeV2BattleInitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedBossState {
    None,
    Covid,
    Lazy,
    Saitama,
}

#[derive(Debug, Clone)]
struct PreparedPlayerInit {
    template: PlayerTemplate,
    hp: i32,
    alive: bool,
    boss_state: PreparedBossState,
    shadow_blueprint: Option<PlayerTemplate>,
    summon_blueprint: Option<PlayerTemplate>,
    zombie_blueprint: Option<PlayerTemplate>,
}

/// 与 seed 无关的 Runtime v2 对局初始化模板。
///
/// 名字解析、组队加成、玩家 build 和召唤物蓝图只执行一次；每场对局只需根据
/// seed 重建随机排序、初始移动点和 world 视图，供批量评分/胜率路径复用。
#[derive(Debug, Clone)]
pub struct PreparedBattleRoster {
    players: Vec<PreparedPlayerInit>,
    input_groups: Vec<Vec<PlrId>>,
    base_names_sorted: Vec<String>,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
}

#[derive(Debug, Clone)]
pub struct PreparedBattleInit {
    players: Vec<PreparedPlayerInit>,
    input_groups: Vec<Vec<EntityIdx>>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    rng: RC4,
}

/// 只包含 seed 会改变的对局初始状态。
///
/// 固定 roster 的批量胜率路径可直接把这份轻量状态应用到已经复位的 prototype，
/// 无需为每局深拷贝玩家模板、技能表和召唤物蓝图。
#[derive(Debug, Clone)]
pub struct PreparedBattleSeed {
    input_groups: Vec<Vec<EntityIdx>>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    teams: Vec<usize>,
    speed_points: Vec<i32>,
    rng: RC4,
}

impl PreparedBattleRoster {
    pub fn from_groups(raw_groups: &[Vec<String>], registry: &ExtensionRegistry) -> Result<Self, RuntimeV2BattleInitError> {
        Self::from_groups_with_eval_rq(raw_groups, crate::player::eval_name::DEFAULT_EVAL_RQ, registry)
    }

    pub fn from_groups_with_eval_rq(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
    ) -> Result<Self, RuntimeV2BattleInitError> {
        let skill_import = PlainLegacySkillImportMap::new(registry);
        Self::from_groups_with_eval_rq_and_skill_import(raw_groups, eval_rq, registry, &skill_import)
    }

    pub fn from_groups_with_eval_rq_and_skill_import(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
    ) -> Result<Self, RuntimeV2BattleInitError> {
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let player_capacity = raw_groups.iter().map(Vec::len).sum();
        let storage = Storage::new_arc_with_eval_rq_and_capacities(eval_rq, player_capacity, raw_groups.len());
        let mut players = Vec::with_capacity(player_capacity);
        let mut input_groups = Vec::with_capacity(raw_groups.len());

        for (team_index, raw_group) in raw_groups.iter().enumerate() {
            let mut group = Vec::with_capacity(raw_group.len());
            for (player_index, raw) in raw_group.iter().enumerate() {
                if Player::check_is_seed(raw) {
                    continue;
                }
                let player = Player::new_from_namerena_raw(raw.clone(), storage.clone()).map_err(|error| {
                    RuntimeV2BattleInitError::Player {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                        message: format!("{error:?}"),
                    }
                })?;
                let id: usize = player.id().try_into().expect("runtime v2 prepared player id overflow");
                assert_eq!(id, players.len(), "runtime v2 prepared player ids must be dense");
                players.push(player);
                group.push(id);
            }
            if !group.is_empty() {
                input_groups.push(group);
            }
        }
        #[cfg(test)]
        let parsed_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();

        PreparedBattleInit::apply_team_upgrades(&mut players, &mut input_groups);
        PreparedBattleInit::build_players(&mut players);
        #[cfg(test)]
        let built_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();

        let player_count = players.len();
        for player in players {
            storage.just_insert_player(player);
        }

        let id_key_names = (0..player_count)
            .map(|id| storage.get_player(&id).expect("runtime v2 prepared player disappeared").id_key_name())
            .collect::<Vec<_>>();
        let mut sorted_by_id_name = (0..player_count).collect::<Vec<_>>();
        sorted_by_id_name.sort_by(|left, right| id_key_names[*left].cmp(&id_key_names[*right]));

        let mut team_by_player = vec![0; player_count];
        for (team, group) in input_groups.iter().enumerate() {
            for player in group {
                team_by_player[*player] = team;
            }
        }
        #[cfg(test)]
        let indexed_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let players = (0..player_count)
            .map(|id| {
                let player = storage.get_player(&id).expect("runtime v2 prepared player disappeared");
                PreparedBattleInit::prepare_player(player, id, team_by_player[id], &storage, registry, skill_import)
            })
            .collect::<Vec<_>>();
        #[cfg(test)]
        if std::env::var_os("TSWN_PROBE_PREPARED_INIT").is_some() {
            eprintln!(
                "[v2_prepared_init] players={player_count} parse={}ns build={}ns index={}ns convert={}ns",
                parsed_elapsed.as_nanos(),
                built_elapsed.as_nanos(),
                indexed_elapsed.as_nanos(),
                phase_started.elapsed().as_nanos(),
            );
        }

        Ok(Self {
            players,
            input_groups,
            base_names_sorted: PreparedBattleInit::base_names_sorted(raw_groups),
            id_key_names,
            sorted_by_id_name,
        })
    }

    pub fn with_seed(&self, seed: &[String]) -> PreparedBattleInit { self.seed_state(seed).into_init(self.players.clone()) }

    pub fn into_with_seed(self, seed: &[String]) -> PreparedBattleInit {
        let seed_state = self.seed_state(seed);
        seed_state.into_init(self.players)
    }

    pub fn seed_state(&self, seed: &[String]) -> PreparedBattleSeed {
        let key = PreparedBattleInit::rc4_key_with_seed(&self.base_names_sorted, seed);
        let mut rng = RC4::new(key.as_bytes(), 1);
        rng.js_xor_str(&key);

        let mut sort_ints = vec![0; self.players.len()];
        for &id in &self.sorted_by_id_name {
            sort_ints[id] = rng.rFFFFFF() as i32;
        }

        let mut battle_groups = self.input_groups.clone();
        for group in &mut battle_groups {
            group.sort_by(|left, right| PreparedBattleInit::cmp_player_keys(&sort_ints, &self.id_key_names, *left, *right));
        }
        let input_groups = battle_groups.iter().map(|group| PreparedBattleInit::entity_order(group)).collect();
        battle_groups.sort_by(|left, right| match (left.first(), right.first()) {
            (Some(left), Some(right)) => PreparedBattleInit::cmp_player_keys(&sort_ints, &self.id_key_names, *left, *right),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        for group in &battle_groups {
            for player in group {
                rng.encrypt_bytes_no_change(&self.id_key_names[*player]);
            }
            rng.encrypt_bytes(&mut [0]);
        }

        let mut teams = vec![0; self.players.len()];
        for (team, group) in battle_groups.iter().enumerate() {
            for &player in group {
                teams[player] = team;
            }
        }

        let mut round_order = battle_groups.iter().flatten().copied().collect::<Vec<_>>();
        round_order.sort_by(|left, right| PreparedBattleInit::cmp_player_keys(&sort_ints, &self.id_key_names, *left, *right));
        let mut speed_points = vec![0; self.players.len()];
        for &player in &round_order {
            speed_points[player] = rng.r255() as i32;
        }

        let team_roster = battle_groups
            .iter()
            .map(|group| PreparedBattleInit::entity_order(group))
            .collect::<Vec<_>>();
        let team_alive = battle_groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .copied()
                    .filter(|id| self.players[*id].alive)
                    .map(PreparedBattleInit::entity_idx)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let flat_alive = team_alive.iter().flatten().copied().collect();

        PreparedBattleSeed {
            input_groups,
            round_order: PreparedBattleInit::entity_order(&round_order),
            team_roster,
            team_alive,
            flat_alive,
            teams,
            speed_points,
            rng,
        }
    }

    pub fn input_groups(&self) -> Vec<Vec<EntityIdx>> {
        self.input_groups.iter().map(|group| PreparedBattleInit::entity_order(group)).collect()
    }
}

impl PreparedBattleSeed {
    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    fn into_init(self, mut players: Vec<PreparedPlayerInit>) -> PreparedBattleInit {
        for (index, player) in players.iter_mut().enumerate() {
            PreparedBattleInit::set_prepared_team(player, self.teams[index]);
            player.template.move_state.speed_points = self.speed_points[index];
        }
        PreparedBattleInit {
            players,
            input_groups: self.input_groups,
            round_order: self.round_order,
            team_roster: self.team_roster,
            team_alive: self.team_alive,
            flat_alive: self.flat_alive,
            rng: self.rng,
        }
    }

    pub fn apply(self, runtime: &mut CombatRuntime) -> Result<(), RuntimeV2BattleInitError> {
        if self.teams.len() != runtime.entities.len() {
            return Err(RuntimeV2BattleInitError::EntityCountMismatch {
                prepared: self.teams.len(),
                runtime: runtime.entities.len(),
            });
        }

        let shadow_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT);
        let summon_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT);
        let zombie_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT);
        for (index, (&team, &speed_points)) in self.teams.iter().zip(&self.speed_points).enumerate() {
            let entity_idx = PreparedBattleInit::entity_idx(index);
            let entity = runtime
                .entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime v2 entity disappeared during seed reset: {}", entity_idx.0));
            entity.template.team = team;
            entity.runtime.team = team;
            if entity.template.kind != PlayerTemplate::DEFAULT_KIND {
                continue;
            }
            entity.template.move_state.speed_points = speed_points;
            entity.runtime.move_state.speed_points = speed_points;
            for slot in [shadow_blueprint_slot, summon_blueprint_slot, zombie_blueprint_slot]
                .into_iter()
                .flatten()
            {
                if let Some(SlotValue::PlayerTemplate(template)) = entity.slots.get_mut(slot) {
                    template.team = team;
                }
            }
        }

        runtime.world.sync_initial_views(
            &runtime.entities,
            self.round_order,
            self.team_roster,
            self.team_alive,
            self.flat_alive,
        );
        runtime.rng = self.rng;
        runtime.scheduler.reset_action_mode_from_entities(&runtime.entities);
        Ok(())
    }
}

impl PreparedBattleInit {
    pub fn split_namerena_raw(raw_input: String) -> (Vec<Vec<String>>, Vec<String>) {
        let raw_input = raw_input.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = raw_input.split('\n').map(trim_js_line_end).map(str::to_owned).collect::<Vec<_>>();
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }

        let seed = lines.iter().filter(|line| Player::check_is_seed(line)).cloned().collect::<Vec<_>>();
        if !lines.iter().any(|line| line.is_empty()) {
            return (lines.into_iter().map(|line| vec![line]).collect(), seed);
        }

        let mut groups = Vec::new();
        let mut current_group = Vec::new();
        for line in lines {
            if line.is_empty() {
                if !current_group.is_empty() {
                    groups.push(std::mem::take(&mut current_group));
                }
            } else {
                current_group.push(line);
            }
        }
        if !current_group.is_empty() {
            groups.push(current_group);
        }

        let is_seed_only = |group: &Vec<String>| !group.is_empty() && group.iter().all(|name| Player::check_is_seed(name));
        let mut index = 0;
        while index < groups.len() {
            if !is_seed_only(&groups[index]) {
                index += 1;
                continue;
            }

            let previous = (0..index).rev().find(|candidate| !is_seed_only(&groups[*candidate]));
            let next = ((index + 1)..groups.len()).find(|candidate| !is_seed_only(&groups[*candidate]));
            let Some(target_index) = previous.or(next) else {
                index += 1;
                continue;
            };

            let seed_group = groups.remove(index);
            if target_index < index {
                groups[target_index].extend(seed_group);
                index = target_index + 1;
            } else {
                let adjusted = target_index - 1;
                let mut merged = seed_group;
                merged.extend(groups[adjusted].clone());
                groups[adjusted] = merged;
                index = adjusted + 1;
            }
        }
        (groups, seed)
    }

    pub fn from_groups(
        raw_groups: &[Vec<String>],
        seed: &[String],
        registry: &ExtensionRegistry,
    ) -> Result<Self, RuntimeV2BattleInitError> {
        Self::from_groups_with_eval_rq(raw_groups, seed, crate::player::eval_name::DEFAULT_EVAL_RQ, registry)
    }

    pub fn from_groups_with_eval_rq(
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
        registry: &ExtensionRegistry,
    ) -> Result<Self, RuntimeV2BattleInitError> {
        Ok(PreparedBattleRoster::from_groups_with_eval_rq(raw_groups, eval_rq, registry)?.with_seed(seed))
    }

    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    pub fn apply(self, runtime: &mut CombatRuntime) -> Result<(), RuntimeV2BattleInitError> {
        if self.players.len() != runtime.entities.len() {
            return Err(RuntimeV2BattleInitError::EntityCountMismatch {
                prepared: self.players.len(),
                runtime: runtime.entities.len(),
            });
        }

        let shadow_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT);
        let summon_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT);
        let zombie_blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT);
        for (index, prepared) in self.players.into_iter().enumerate() {
            let entity_idx = Self::entity_idx(index);
            let entity = runtime
                .entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime v2 entity disappeared during battle init: {}", entity_idx.0));
            entity.template.team = prepared.template.team;
            entity.runtime.team = prepared.template.team;
            if entity.template.kind != PlayerTemplate::DEFAULT_KIND {
                continue;
            }

            let prepared_template = prepared.template;
            entity.template.display_name = prepared_template.display_name;
            entity.template.id_key_name = prepared_template.id_key_name;
            entity.template.clan_name = prepared_template.clan_name;
            entity.template.kind = prepared_template.kind;
            entity.template.max_hp = prepared_template.max_hp;
            entity.template.attack = prepared_template.attack;
            entity.template.magic = prepared_template.magic;
            entity.template.magic_point = prepared_template.magic_point;
            entity.template.wisdom = prepared_template.wisdom;
            entity.template.speed = prepared_template.speed;
            entity.template.defense = prepared_template.defense;
            entity.template.resistance = prepared_template.resistance;
            entity.template.agility = prepared_template.agility;
            entity.template.at_boost_bits = prepared_template.at_boost_bits;
            entity.template.at_boost_millionths = prepared_template.at_boost_millionths;
            entity.template.attr_sum = prepared_template.attr_sum;
            entity.template.atk_sum = prepared_template.atk_sum;
            entity.template.attract_bits = prepared_template.attract_bits;
            entity.template.move_state = prepared_template.move_state;
            entity.template.skills = prepared_template.skills;
            entity.template.clone_build = prepared_template.clone_build;
            entity.runtime = PlayerRuntime::from_template(&entity.template, &runtime.registry, entity_idx, entity_idx);
            entity.runtime.hp = prepared.hp;
            entity.runtime.alive = prepared.alive;

            match prepared.boss_state {
                PreparedBossState::None => {}
                PreparedBossState::Covid => {
                    entity.states.add_entry(StateEntry::covid_boss(PLAIN_COVID_BOSS_STATE_KEY, 40));
                }
                PreparedBossState::Lazy => {
                    entity.states.add_entry(StateEntry::lazy_boss(PLAIN_LAZY_BOSS_STATE_KEY, 1.0));
                }
                PreparedBossState::Saitama => {
                    let state_id = runtime
                        .registry
                        .state_id_by_export_name(DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT)
                        .expect("default runtime v2 profile must register saitama boss state");
                    entity.states.add_entry(StateEntry::saitama_boss(
                        PLAIN_SAITAMA_BOSS_STATE_KEY,
                        state_id,
                        SkillPriority(i32::MAX),
                    ));
                }
            }

            if let Some(template) = prepared.shadow_blueprint {
                let slot = shadow_blueprint_slot.expect("runtime v2 shadow skill requires the core shadow blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                    .expect("runtime v2 core shadow blueprint slot must exist");
            }
            if let Some(template) = prepared.summon_blueprint {
                let slot = summon_blueprint_slot.expect("runtime v2 summon skill requires the core summon blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                    .expect("runtime v2 core summon blueprint slot must exist");
            }
            if let Some(template) = prepared.zombie_blueprint {
                let slot = zombie_blueprint_slot.expect("runtime v2 zombie skill requires the core zombie blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                    .expect("runtime v2 core zombie blueprint slot must exist");
            }
        }

        runtime.world.sync_initial_views(
            &runtime.entities,
            self.round_order,
            self.team_roster,
            self.team_alive,
            self.flat_alive,
        );
        runtime.rng = self.rng;
        runtime.scheduler.reset_action_mode_from_entities(&runtime.entities);
        Ok(())
    }

    fn apply_team_upgrades(players: &mut [Player], groups: &mut [Vec<PlrId>]) {
        for group in groups {
            group.sort_by(|left, right| players[*left].partial_cmp(&players[*right]).unwrap_or(std::cmp::Ordering::Equal));
            for left_index in 0..group.len() {
                for right_index in (left_index + 1)..group.len() {
                    let left_id = group[left_index];
                    let right_id = group[right_index];
                    if players[left_id].clan_name() != players[right_id].clan_name() {
                        continue;
                    }
                    let (left, right) = Self::two_players_mut(players, left_id, right_id);
                    left.upgrade(right);
                    right.upgrade(left);
                }
            }
        }
    }

    fn build_players(players: &mut [Player]) {
        let mut order = (0..players.len()).collect::<Vec<_>>();
        order.sort_by(|left, right| players[*left].cmp_by_id_name(&players[*right]));
        for id in order {
            let player = &mut players[id];
            player.build();
            if player.player_type() == PlayerType::Boss {
                crate::player::boss::init_boss_state(player);
            }
        }
    }

    fn set_prepared_team(prepared: &mut PreparedPlayerInit, team: usize) {
        prepared.template.team = team;
        for blueprint in [
            &mut prepared.shadow_blueprint,
            &mut prepared.summon_blueprint,
            &mut prepared.zombie_blueprint,
        ] {
            if let Some(template) = blueprint.as_mut() {
                template.team = team;
            }
        }
    }

    fn prepare_player(
        player: &Player,
        id: PlrId,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
    ) -> PreparedPlayerInit {
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let status = player.get_status();
        let skills = skill_import.import_storage(player.skill_storage());
        #[cfg(test)]
        let skill_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let kind = match player.player_type() {
            PlayerType::Boss => registry
                .player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT)
                .expect("default runtime v2 profile must register core boss kind"),
            PlayerType::Boost => registry
                .player_kind_id_by_export_name(DEFAULT_CORE_BOOST_KIND_EXPORT)
                .expect("default runtime v2 profile must register core boost kind"),
            _ => PlayerTemplate::DEFAULT_KIND,
        };
        let (clone_attrs, clone_weapon_attr_bonus, clone_name_factor) = player.clone_build_inputs();
        let child_clone_name_factor = Self::child_clone_name_factor(player, storage.eval_rq());
        let clone_build = CloneBuildData::from_legacy(clone_attrs, clone_weapon_attr_bonus, clone_name_factor, status)
            .with_child_name_factor(child_clone_name_factor);
        let mut template = Self::template_from_player(player, id, team, skills.clone());
        template.kind = kind;
        template.clone_build = Some(clone_build);
        #[cfg(test)]
        let template_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let shadow_blueprint = registry
            .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
            .filter(|skill| skills.skills().contains(skill))
            .map(|_| {
                let shadow = crate::player::skill::act::shadow::build_shadow_minion(id, storage);
                let shadow_skills = skill_import.import_storage(shadow.skill_storage());
                let shadow_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
                    .expect("runtime v2 registry importing shadow must register core shadow kind");
                let mut template = Self::template_from_player(&shadow, 0, team, shadow_skills);
                template.kind = shadow_kind;
                template.clone_build = Some(Self::clone_build_from_player(&shadow, child_clone_name_factor));
                template
            });
        #[cfg(test)]
        let shadow_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let summon_blueprint = registry
            .skill_id_by_export_name(BuiltinActiveSkill::Summon.export_name())
            .filter(|skill| skills.skills().contains(skill))
            .map(|_| {
                let summon_overlay = crate::player::skill::act::minion::owner_minion_overlay(
                    storage,
                    id,
                    crate::player::skill::act::minion::MinionKind::Summon,
                );
                let summon = crate::player::skill::act::summon::build_summon_minion(id, storage, true);
                let summon_skills = skill_import.import_storage(summon.skill_storage());
                let summon_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
                    .expect("runtime v2 registry importing summon must register core summon kind");
                let mut template = Self::template_from_player(&summon, 0, team, summon_skills);
                template.kind = summon_kind;
                template.reserved_player_ids_before_spawn = 1;
                template.clone_build = Some(Self::clone_build_from_player(&summon, child_clone_name_factor));
                template.reuse_skills_on_recast = summon_overlay.as_ref().map_or(true, |overlay| overlay.reuse_skills_on_recast);
                let has_overlay_attrs = summon_overlay.as_ref().is_some_and(|overlay| overlay.attrs.is_some());
                template.reuse_stats_on_recast = !has_overlay_attrs;
                template.inherit_owner_def_res =
                    !has_overlay_attrs || summon_overlay.as_ref().is_some_and(|overlay| overlay.inherit_owner_def_res);
                template
            });
        #[cfg(test)]
        let summon_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let zombie_blueprint = registry
            .skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
            .filter(|skill| skills.skills().contains(skill))
            .map(|_| Self::build_zombie_blueprint(id, team, storage, registry, skill_import, child_clone_name_factor));
        #[cfg(test)]
        let zombie_elapsed = phase_started.elapsed();
        let boss_state = match crate::player::boss::boss_kind(&player.id_name()) {
            crate::player::boss::BossKind::Covid => PreparedBossState::Covid,
            crate::player::boss::BossKind::Lazy => PreparedBossState::Lazy,
            crate::player::boss::BossKind::Saitama => PreparedBossState::Saitama,
            _ => PreparedBossState::None,
        };
        #[cfg(test)]
        if std::env::var_os("TSWN_PROBE_PREPARED_PLAYER").is_some() {
            eprintln!(
                "[v2_prepared_player] id={id} name={:?} skills={} skill={}ns template={}ns shadow={}ns summon={}ns zombie={}ns blueprints={}/{}/{}",
                player.id_name(),
                skills.skills().len(),
                skill_elapsed.as_nanos(),
                template_elapsed.as_nanos(),
                shadow_elapsed.as_nanos(),
                summon_elapsed.as_nanos(),
                zombie_elapsed.as_nanos(),
                shadow_blueprint.is_some(),
                summon_blueprint.is_some(),
                zombie_blueprint.is_some(),
            );
        }
        PreparedPlayerInit {
            template,
            hp: status.hp,
            alive: status.alive(),
            boss_state,
            shadow_blueprint,
            summon_blueprint,
            zombie_blueprint,
        }
    }

    fn build_zombie_blueprint(
        id: PlrId,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        child_clone_name_factor: f64,
    ) -> PlayerTemplate {
        let zombie = crate::player::skill::zombie::build_zombie_minion_blueprint(id, storage);
        let zombie_skills = skill_import.import_storage(zombie.skill_storage());
        let zombie_kind = registry
            .player_kind_id_by_export_name(DEFAULT_CORE_ZOMBIE_KIND_EXPORT)
            .expect("runtime v2 registry importing zombie must register core zombie kind");
        let mut template = Self::template_from_player(&zombie, 0, team, zombie_skills);
        template.kind = zombie_kind;
        template.reserved_player_ids_before_spawn = 1;
        template.clone_build = Some(Self::clone_build_from_player(&zombie, child_clone_name_factor));
        template
    }

    fn clone_build_from_player(player: &Player, child_clone_name_factor: f64) -> CloneBuildData {
        let status = player.get_status();
        let (clone_attrs, clone_weapon_attr_bonus, clone_name_factor) = player.clone_build_inputs();
        CloneBuildData::from_legacy(clone_attrs, clone_weapon_attr_bonus, clone_name_factor, status)
            .with_child_name_factor(child_clone_name_factor)
    }

    fn child_clone_name_factor(player: &Player, eval_rq: f64) -> f64 {
        let factor_name = crate::player::eval_name::eval_str_common_with_rq(player.base_name().as_str(), true, eval_rq);
        let factor_team = crate::player::eval_name::eval_str_common_with_rq(player.clan_name().as_str(), true, eval_rq);
        factor_name.max(factor_team - 6.0)
    }

    fn template_from_player(player: &Player, id: PlrId, team: usize, skills: SkillLoadout) -> PlayerTemplate {
        let status = player.get_status();
        PlayerTemplate::new(id, player.id_name(), team, status.max_hp, status.attack)
            .with_identity_names(player.id_key_name(), player.clan_name())
            .with_display_name(player.display_name())
            .with_magic(status.magic)
            .with_magic_point(status.magic_point)
            .with_wisdom(status.wisdom)
            .with_speed(status.speed)
            .with_def_res(status.defense, status.resistance)
            .with_agility(status.agility)
            .with_at_boost(status.at_boost)
            .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
            .with_speed_points(player.move_point())
            .with_skill_loadout(skills)
    }

    fn base_names_sorted(raw_groups: &[Vec<String>]) -> Vec<String> {
        let mut names = raw_groups
            .iter()
            .flatten()
            .filter(|raw| !Player::check_is_seed(raw))
            .map(|raw| Player::raw_namerena_to_idname(raw))
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn rc4_key_with_seed(base_names_sorted: &[String], seed: &[String]) -> String {
        let mut names = base_names_sorted.iter().collect::<Vec<_>>();
        names.extend(seed);
        names.sort_unstable();
        names.dedup();
        names.into_iter().map(String::as_str).collect::<Vec<_>>().join("\r")
    }

    fn cmp_player_keys(sort_ints: &[i32], id_key_names: &[String], left: PlrId, right: PlrId) -> std::cmp::Ordering {
        sort_ints[left]
            .cmp(&sort_ints[right])
            .then_with(|| id_key_names[left].cmp(&id_key_names[right]))
            .then_with(|| left.cmp(&right))
    }

    fn two_players_mut(players: &mut [Player], left: PlrId, right: PlrId) -> (&mut Player, &mut Player) {
        assert_ne!(left, right, "runtime v2 battle init requested the same player twice");
        if left < right {
            let (before_right, from_right) = players.split_at_mut(right);
            (&mut before_right[left], &mut from_right[0])
        } else {
            let (before_left, from_left) = players.split_at_mut(left);
            (&mut from_left[0], &mut before_left[right])
        }
    }

    fn entity_order(players: &[PlrId]) -> Vec<EntityIdx> { players.iter().copied().map(Self::entity_idx).collect() }

    fn entity_idx(player: PlrId) -> EntityIdx {
        EntityIdx(player.try_into().expect("runtime v2 prepared player id overflowed entity index"))
    }
}
