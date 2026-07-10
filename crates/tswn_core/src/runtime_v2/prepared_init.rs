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
}

#[derive(Debug, Clone)]
pub struct PreparedBattleInit {
    players: Vec<PreparedPlayerInit>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    rng: RC4,
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
        let storage = Storage::new_arc();
        let mut players = Vec::new();
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

        Self::apply_team_upgrades(&mut players, &mut input_groups);
        Self::build_players(&mut players);

        let base_names_sorted = Self::base_names_sorted(raw_groups);
        let key = Self::rc4_key_with_seed(&base_names_sorted, seed);
        let mut rng = RC4::new(key.as_bytes(), 1);
        rng.js_xor_str(&key);

        let id_key_names = players.iter().map(Player::id_key_name).collect::<Vec<_>>();
        let mut sort_ints = vec![0; players.len()];
        let mut sorted_by_id_name = (0..players.len()).collect::<Vec<_>>();
        sorted_by_id_name.sort_by(|left, right| id_key_names[*left].cmp(&id_key_names[*right]));
        for id in sorted_by_id_name {
            let sort_int = rng.rFFFFFF() as i32;
            sort_ints[id] = sort_int;
            players[id].set_sort_int(sort_int);
        }

        for group in &mut input_groups {
            group.sort_by(|left, right| Self::cmp_player_keys(&sort_ints, &id_key_names, *left, *right));
        }
        input_groups.sort_by(|left, right| match (left.first(), right.first()) {
            (Some(left), Some(right)) => Self::cmp_player_keys(&sort_ints, &id_key_names, *left, *right),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        for group in &input_groups {
            for player in group {
                rng.encrypt_bytes_no_change(&id_key_names[*player]);
            }
            rng.encrypt_bytes(&mut [0]);
        }

        let mut round_order = input_groups.iter().flatten().copied().collect::<Vec<_>>();
        round_order.sort_by(|left, right| Self::cmp_player_keys(&sort_ints, &id_key_names, *left, *right));
        for player in &round_order {
            players[*player].set_move_point(rng.r255() as i32);
        }

        for player in &players {
            storage.just_insert_player(player.clone());
        }

        let mut team_by_player = vec![0; players.len()];
        for (team, group) in input_groups.iter().enumerate() {
            for player in group {
                team_by_player[*player] = team;
            }
        }

        let prepared_players = players
            .iter()
            .enumerate()
            .map(|(id, player)| Self::prepare_player(player, id, team_by_player[id], &storage, registry))
            .collect();
        let team_roster = input_groups.iter().map(|group| Self::entity_order(group)).collect::<Vec<_>>();
        let team_alive = input_groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .copied()
                    .filter(|id| players[*id].alive())
                    .map(Self::entity_idx)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let flat_alive = team_alive.iter().flatten().copied().collect();

        Ok(Self {
            players: prepared_players,
            round_order: Self::entity_order(&round_order),
            team_roster,
            team_alive,
            flat_alive,
            rng,
        })
    }

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

    fn prepare_player(
        player: &Player,
        id: PlrId,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
    ) -> PreparedPlayerInit {
        let status = player.get_status();
        let skills = import_plain_legacy_skill_loadout(registry, &player.skill_loadout_snapshot());
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
        let clone_build = CloneBuildData::from_legacy(clone_attrs, clone_weapon_attr_bonus, clone_name_factor, status);
        let mut template = Self::template_from_player(player, id, team, skills.clone());
        template.kind = kind;
        template.clone_build = Some(clone_build);
        let shadow_blueprint = registry
            .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
            .filter(|skill| skills.skills().contains(skill))
            .map(|_| {
                let shadow = crate::player::skill::act::shadow::build_shadow_minion(id, storage);
                let shadow_skills = import_plain_legacy_skill_loadout(registry, &shadow.skill_loadout_snapshot());
                let shadow_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
                    .expect("runtime v2 registry importing shadow must register core shadow kind");
                let mut template = Self::template_from_player(&shadow, 0, team, shadow_skills);
                template.kind = shadow_kind;
                template
            });
        let summon_blueprint = registry
            .skill_id_by_export_name(BuiltinActiveSkill::Summon.export_name())
            .filter(|skill| skills.skills().contains(skill))
            .map(|_| {
                let summon = crate::player::skill::act::summon::build_summon_minion(id, storage, true);
                let summon_skills = import_plain_legacy_skill_loadout(registry, &summon.skill_loadout_snapshot());
                let summon_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
                    .expect("runtime v2 registry importing summon must register core summon kind");
                let mut template = Self::template_from_player(&summon, 0, team, summon_skills);
                template.kind = summon_kind;
                template
            });
        let boss_state = match crate::player::boss::boss_kind(&player.id_name()) {
            crate::player::boss::BossKind::Covid => PreparedBossState::Covid,
            crate::player::boss::BossKind::Lazy => PreparedBossState::Lazy,
            crate::player::boss::BossKind::Saitama => PreparedBossState::Saitama,
            _ => PreparedBossState::None,
        };
        PreparedPlayerInit {
            template,
            hp: status.hp,
            alive: status.alive(),
            boss_state,
            shadow_blueprint,
            summon_blueprint,
        }
    }

    fn template_from_player(player: &Player, id: PlrId, team: usize, skills: SkillLoadout) -> PlayerTemplate {
        let status = player.get_status();
        PlayerTemplate::new(id, player.id_name(), team, status.max_hp, status.attack)
            .with_display_name(player.display_name())
            .with_magic(status.magic)
            .with_magic_point(status.magic_point)
            .with_wisdom(status.wisdom)
            .with_speed(status.speed)
            .with_def_res(status.defense, status.resistance)
            .with_agility(status.agility)
            .with_at_boost_millionths((status.at_boost * 1_000_000.0).round() as i64)
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
