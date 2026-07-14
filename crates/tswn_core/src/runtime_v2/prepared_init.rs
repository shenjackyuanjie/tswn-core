use super::*;
use crate::engine::storage::Storage;
use crate::player::skill::act::minion::MinionBlueprintOwner;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedPlayerInit {
    template: PlayerTemplate,
    hp: i32,
    alive: bool,
    boss_state: PreparedBossState,
    shadow_blueprint: Option<PlayerTemplate>,
    summon_blueprint: Option<PlayerTemplate>,
    zombie_blueprint: Option<PlayerTemplate>,
    lazy_blueprint_rq_bits: Option<u64>,
}

/// score 数字 profile 的紧凑构造态；只保留生成 Runtime v2 模板真正需要的数据。
struct ScoreProfileBuild {
    id: PlrId,
    name: String,
    clan_name: String,
    name_base: [u8; 128],
    raw_name_base: [u8; 128],
    skill_order: [u32; 40],
    test_ex: bool,
}

/// 连续 score 轮次之间回收的动态 profile 身份字符串。
#[derive(Debug, Default)]
pub(crate) struct ScoreIdentityBuffer {
    pub(crate) name: String,
    pub(crate) id_key_name: String,
    pub(crate) clan_name: String,
    pub(crate) display_name: String,
}

/// score 每轮构造动态 profile 时复用的临时向量。
#[derive(Default)]
pub(crate) struct ScoreRoundScratch {
    dynamic_inputs: Vec<(usize, usize, usize)>,
    dynamic_groups: Vec<Vec<usize>>,
    name_keys: Vec<[u8; crate::player::NAME_MAX_LEN + 1]>,
    name_lengths: Vec<usize>,
    profile_rngs: Vec<RC4>,
    dynamic_profiles: Vec<ScoreProfileBuild>,
    team_by_player: Vec<usize>,
}

/// score roster 构造与应用完成后交还下一轮的输出向量。
#[derive(Debug, Default, Clone)]
pub(crate) struct ScoreRosterBuffers {
    players: Vec<Option<PreparedPlayerInit>>,
    player_alive: Vec<bool>,
    input_groups: Vec<Vec<PlrId>>,
    base_names_sorted: Vec<String>,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
}

impl ScoreProfileBuild {
    fn from_rng(id: PlrId, name: String, clan_name: String, mut rand: RC4) -> Self {
        let mut name_base = [0u8; 128];
        let mut output = 0usize;
        for &value in &rand.main_val {
            let mapped = ((u32::from(value) * 181) + 160) & 255;
            if (89..217).contains(&mapped) {
                name_base[output] = (mapped & 63) as u8;
                output += 1;
            }
        }
        assert_eq!(output, 128, "score profile name base must contain 128 entries");
        let mut raw_name_base = name_base;
        let test_ex = clan_name == "!";
        if test_ex {
            for value in &mut name_base[6..50] {
                if *value < 41 {
                    *value = (*value & 15) + 41;
                }
            }
            for value in &mut name_base[50..] {
                if *value < 16 {
                    *value += 32;
                }
            }
            raw_name_base = name_base;
        } else {
            debug_assert_eq!(clan_name, "\u{0002}");
            for value in &mut name_base[..50] {
                if *value < 12 {
                    *value = 63 - *value;
                }
            }
        }

        let mut skill_order = std::array::from_fn(|index| index as u32);
        rand.sort_list(&mut skill_order);
        Self {
            id,
            name,
            clan_name,
            name_base,
            raw_name_base,
            skill_order,
            test_ex,
        }
    }

    fn upgrade_from(&mut self, other: &Self) {
        if self.test_ex {
            return;
        }
        for index in 7..128 {
            if other.raw_name_base[index - 1] == self.raw_name_base[index] && other.raw_name_base[index] > self.name_base[index] {
                self.name_base[index] = other.raw_name_base[index];
            }
        }
    }

    fn into_prepared(
        self,
        team: usize,
        eval_rq: f64,
        skill_import: &PlainLegacySkillImportMap,
        mut skills: SkillLoadout,
        mut id_key_name: String,
        mut display_name: String,
    ) -> PreparedPlayerInit {
        let mut sorted_head: [u8; 10] = self.name_base[..10].try_into().expect("score profile head length is fixed");
        sorted_head.sort_unstable();
        let mut attrs = [0u32; 8];
        for (attr, offset) in attrs[..7].iter_mut().zip((10..31).step_by(3)) {
            *attr = u32::from(crate::player::median(
                self.name_base[offset],
                self.name_base[offset + 1],
                self.name_base[offset + 2],
            ));
        }
        attrs[7] =
            154 + u32::from(sorted_head[3]) + u32::from(sorted_head[4]) + u32::from(sorted_head[5]) + u32::from(sorted_head[6]);

        let mut levels = [0u32; 35];
        let mut boosted = [false; 35];
        let mut boosts: [Option<crate::player::skill::SkillBoost>; 35] = std::array::from_fn(|_| None);
        let mut slot_skill_keys = [None; 16];
        for (slot, offset) in (64..128).step_by(4).enumerate() {
            let small = *self.name_base[offset..offset + 4].iter().min().unwrap();
            let key = self.skill_order[slot] as usize;
            if small <= 10 || key >= 35 {
                continue;
            }
            levels[key] = u32::from(small - 10);
            boosted[key] = self.raw_name_base[offset..offset + 4].iter().min().copied().unwrap() <= 10;
            slot_skill_keys[slot] = Some(key);
        }
        for &key in self.skill_order.iter().rev() {
            let key = key as usize;
            if key < 25 && levels[key] > 0 && !boosted[key] {
                let base = levels[key];
                levels[key] = base.saturating_mul(2);
                boosted[key] = true;
                boosts[key] = Some(crate::player::skill::SkillBoost::LastBoost(base));
                break;
            }
        }
        for (slot, left, right) in [(14usize, 60usize, 61usize), (15, 62, 63)] {
            let Some(key) = slot_skill_keys[slot] else {
                continue;
            };
            if levels[key] == 0 || boosted[key] {
                continue;
            }
            let base = levels[key];
            let amount = u32::from(self.name_base[left].min(self.name_base[right])).min(base);
            levels[key] = base.saturating_add(amount);
            boosted[key] = true;
            boosts[key] = Some(crate::player::skill::SkillBoost::SlotBoost { base, boost: amount });
        }

        let attack = attrs[0] as i32;
        let defense = attrs[1] as i32;
        let speed = attrs[2] as i32 + 160;
        let agility = attrs[3] as i32;
        let magic = attrs[4] as i32;
        let resistance = attrs[5] as i32;
        let wisdom = attrs[6] as i32;
        let max_hp = attrs[7] as i32;
        let attr_sum = attrs[..7].iter().sum();
        let atk_sum = (attack - defense + attrs[2] as i32 + magic - resistance) * 2 + agility + wisdom;
        let mut status = crate::player::PlayerStatus {
            hp: max_hp,
            max_hp,
            attack,
            defense,
            speed,
            agility,
            magic,
            magic_point: wisdom >> 1,
            resistance,
            wisdom,
            attr_sum,
            atk_sum,
            all_sum: attr_sum * 3 + attrs[7],
            ..crate::player::PlayerStatus::default()
        };
        status.at_boost = 1.0;
        let child_clone_name_factor = {
            let factor_name = crate::player::eval_name::eval_str_common_with_rq(&self.name, true, eval_rq);
            let factor_team = crate::player::eval_name::eval_str_common_with_rq(&self.clan_name, true, eval_rq);
            factor_name.max(factor_team - 6.0)
        };
        let clone_build = CloneBuildData::from_score_profile(attrs, child_clone_name_factor);
        skill_import.reset_score_profile(&mut skills, &levels, &boosted, &boosts, &self.skill_order);
        id_key_name.clear();
        id_key_name.push_str(&self.name);
        id_key_name.push('@');
        id_key_name.push_str(&self.clan_name);
        display_name.clear();
        display_name.push_str(&self.name);
        let template = PlayerTemplate::from_score_profile(
            self.id,
            self.name,
            id_key_name,
            self.clan_name,
            display_name,
            team,
            &status,
            skills,
            clone_build,
        );
        PreparedPlayerInit {
            template,
            hp: max_hp,
            alive: true,
            boss_state: PreparedBossState::None,
            shadow_blueprint: None,
            summon_blueprint: None,
            zombie_blueprint: None,
            lazy_blueprint_rq_bits: Some(eval_rq.to_bits()),
        }
    }
}

/// 与 seed 无关的 Runtime v2 对局初始化模板。
///
/// 名字解析、组队加成、玩家 build 和召唤物蓝图只执行一次；每场对局只需根据
/// seed 重建随机排序、初始移动点和 world 视图，供批量评分/胜率路径复用。
#[derive(Debug, Clone)]
pub struct PreparedBattleRoster {
    /// `None` 表示 score 轮次直接保留 prototype 中的固定 target。
    players: Vec<Option<PreparedPlayerInit>>,
    player_alive: Vec<bool>,
    input_groups: Vec<Vec<PlrId>>,
    base_names_sorted: Vec<String>,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
    recycle_score_buffers: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedBattleInit {
    players: Vec<Option<PreparedPlayerInit>>,
    input_groups: Vec<Vec<EntityIdx>>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    rng: RC4,
    teams: Vec<usize>,
    speed_points: Vec<i32>,
    sort_ints: Vec<i32>,
    battle_groups: Vec<Vec<PlrId>>,
    rc4_key: String,
    score_buffers: Option<ScoreRosterBuffers>,
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
    sort_ints: Vec<i32>,
    battle_groups: Vec<Vec<PlrId>>,
    rc4_key: String,
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
        Self::from_groups_with_eval_rq_and_skill_import_selected(raw_groups, eval_rq, registry, skill_import, &[])
    }

    /// 复用首轮已经准备好的固定 target，只重建位于尾部的动态 profile。
    ///
    /// score 的玩家编号布局始终是“固定 target 在前、动态 profile 在后”。固定
    /// target 的名字、技能与召唤物蓝图不会随轮次变化，因此无需再次解析和 build。
    /// 如果遇到非标准布局，或 profile 与同组固定 target 同 clan、需要跨边界升级，
    /// 则回退到完整构造，保证公共 score 入口对任意合法输入仍保持原语义。
    pub(crate) fn from_score_groups_with_cached_targets(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        profile_player_ids: &[PlrId],
        profile_team: &str,
        profile_team_rng: &RC4,
        cached: &Self,
        skill_buffers: &mut [SkillLoadout],
        identity_buffers: &mut [ScoreIdentityBuffer],
        scratch: &mut ScoreRoundScratch,
        roster_buffers: &mut ScoreRosterBuffers,
    ) -> Result<Self, RuntimeV2BattleInitError> {
        let player_count = raw_groups.iter().flatten().filter(|raw| !Player::check_is_seed(raw)).count();
        let fixed_count = profile_player_ids.first().copied().unwrap_or(player_count);
        let suffix_layout = player_count == cached.players.len()
            && fixed_count <= player_count
            && profile_player_ids.iter().copied().eq(fixed_count..player_count);
        if !suffix_layout {
            return Self::from_groups_with_eval_rq_and_skill_import_selected(
                raw_groups,
                eval_rq,
                registry,
                skill_import,
                profile_player_ids,
            );
        }
        if !matches!(profile_team, "!" | "\u{0002}") {
            return Self::from_groups_with_eval_rq_and_skill_import_selected(
                raw_groups,
                eval_rq,
                registry,
                skill_import,
                profile_player_ids,
            );
        }

        scratch.dynamic_inputs.clear();
        scratch.dynamic_groups.resize_with(raw_groups.len(), Vec::new);
        scratch.dynamic_groups.truncate(raw_groups.len());
        for group in &mut scratch.dynamic_groups {
            group.clear();
        }
        let mut next_player_id = 0usize;

        for (team_index, raw_group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in raw_group.iter().enumerate() {
                if Player::check_is_seed(raw) {
                    continue;
                }
                let id = next_player_id;
                next_player_id += 1;
                if id < fixed_count {
                    continue;
                }
                scratch.dynamic_groups[team_index].push(id - fixed_count);
                scratch.dynamic_inputs.push((team_index, player_index, id));
            }
        }

        // 标准 score profile 都使用同一个 modifier 作为 clan；若与同组固定 target
        // 相同，就必须让双方共同参与 upgrade，因此回退完整 legacy 构造。
        for group in &cached.input_groups {
            if group.iter().any(|id| *id >= fixed_count)
                && group.iter().filter(|id| **id < fixed_count).any(|id| {
                    cached.players[*id]
                        .as_ref()
                        .expect("runtime v2 score target cache must contain fixed players")
                        .template
                        .clan_name
                        == profile_team
                })
            {
                return Self::from_groups_with_eval_rq_and_skill_import_selected(
                    raw_groups,
                    eval_rq,
                    registry,
                    skill_import,
                    profile_player_ids,
                );
            }
        }
        scratch.name_keys.clear();
        scratch
            .name_keys
            .resize(scratch.dynamic_inputs.len(), [0u8; crate::player::NAME_MAX_LEN + 1]);
        scratch.name_lengths.clear();
        for (index, &(team_index, player_index, _)) in scratch.dynamic_inputs.iter().enumerate() {
            let raw = &raw_groups[team_index][player_index];
            let Some((name, _)) = raw.split_once('@').filter(|(_, team)| *team == profile_team && !team.contains('+')) else {
                return Self::from_groups_with_eval_rq_and_skill_import_selected(
                    raw_groups,
                    eval_rq,
                    registry,
                    skill_import,
                    profile_player_ids,
                );
            };
            scratch.name_lengths.push(name.len());
            scratch.name_keys[index][1..1 + name.len()].copy_from_slice(name.as_bytes());
        }
        let key_refs = scratch
            .name_keys
            .iter()
            .zip(&scratch.name_lengths)
            .map(|(key, &name_len)| &key[..1 + name_len])
            .collect::<smallvec::SmallVec<[_; 4]>>();
        scratch.profile_rngs.resize_with(scratch.dynamic_inputs.len(), RC4::default);
        for rng in &mut scratch.profile_rngs {
            rng.clone_from(profile_team_rng);
        }
        RC4::update_interleaved(&mut scratch.profile_rngs, &key_refs, 2);
        scratch.dynamic_profiles.clear();
        for (index, &(team_index, player_index, id)) in scratch.dynamic_inputs.iter().enumerate() {
            let (name, team) = raw_groups[team_index][player_index]
                .split_once('@')
                .expect("已经验证的 score profile 必须包含队名分隔符");
            let (mut name_buffer, mut team_buffer) = if let Some(identity) = identity_buffers.get_mut(index) {
                (std::mem::take(&mut identity.name), std::mem::take(&mut identity.clan_name))
            } else {
                (String::new(), String::new())
            };
            name_buffer.clear();
            name_buffer.push_str(name);
            team_buffer.clear();
            team_buffer.push_str(team);
            scratch.dynamic_profiles.push(ScoreProfileBuild::from_rng(
                id,
                name_buffer,
                team_buffer,
                std::mem::take(&mut scratch.profile_rngs[index]),
            ));
        }
        for group in &mut scratch.dynamic_groups {
            group.sort_by(|left, right| {
                scratch.dynamic_profiles[*left]
                    .name
                    .cmp(&scratch.dynamic_profiles[*right].name)
                    .then_with(|| scratch.dynamic_profiles[*left].id.cmp(&scratch.dynamic_profiles[*right].id))
            });
            for left_index in 0..group.len() {
                for right_index in (left_index + 1)..group.len() {
                    let (left, right) = PreparedBattleInit::two_score_profiles_mut(
                        &mut scratch.dynamic_profiles,
                        group[left_index],
                        group[right_index],
                    );
                    left.upgrade_from(right);
                    right.upgrade_from(left);
                }
            }
        }
        scratch.team_by_player.clear();
        scratch.team_by_player.resize(player_count, 0);
        for (team, group) in cached.input_groups.iter().enumerate() {
            for &player in group {
                scratch.team_by_player[player] = team;
            }
        }
        let mut players = std::mem::take(&mut roster_buffers.players);
        players.clear();
        players.resize_with(player_count, || None);
        let mut player_alive = std::mem::take(&mut roster_buffers.player_alive);
        player_alive.resize(player_count, false);
        player_alive[..fixed_count].copy_from_slice(&cached.player_alive[..fixed_count]);
        let mut id_key_names = std::mem::take(&mut roster_buffers.id_key_names);
        id_key_names.resize_with(player_count, String::new);
        for (target, source) in id_key_names[..fixed_count].iter_mut().zip(&cached.id_key_names[..fixed_count]) {
            target.clone_from(source);
        }
        for (profile_index, profile) in scratch.dynamic_profiles.drain(..).enumerate() {
            let id = profile.id;
            id_key_names[id].clear();
            id_key_names[id].push_str(&profile.name);
            id_key_names[id].push('@');
            id_key_names[id].push_str(&profile.clan_name);
            let skills = skill_buffers.get_mut(profile_index).map(std::mem::take).unwrap_or_default();
            let (id_key_name, display_name) = if let Some(identity) = identity_buffers.get_mut(profile_index) {
                (
                    std::mem::take(&mut identity.id_key_name),
                    std::mem::take(&mut identity.display_name),
                )
            } else {
                (String::new(), String::new())
            };
            let prepared = profile.into_prepared(
                scratch.team_by_player[id],
                eval_rq,
                skill_import,
                skills,
                id_key_name,
                display_name,
            );
            player_alive[id] = prepared.alive;
            players[id] = Some(prepared);
        }
        let mut sorted_by_id_name = std::mem::take(&mut roster_buffers.sorted_by_id_name);
        sorted_by_id_name.clear();
        sorted_by_id_name.extend(0..player_count);
        sorted_by_id_name.sort_by(|left, right| id_key_names[*left].cmp(&id_key_names[*right]));
        let mut input_groups = std::mem::take(&mut roster_buffers.input_groups);
        Self::clone_player_groups_reusing(&mut input_groups, &cached.input_groups);
        let mut base_names_sorted = std::mem::take(&mut roster_buffers.base_names_sorted);
        PreparedBattleInit::refill_base_names_sorted(raw_groups, &mut base_names_sorted);
        Ok(Self {
            players,
            player_alive,
            input_groups,
            base_names_sorted,
            id_key_names,
            sorted_by_id_name,
            recycle_score_buffers: true,
        })
    }

    fn from_groups_with_eval_rq_and_skill_import_selected(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        lazy_blueprint_players: &[PlrId],
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
                PreparedBattleInit::prepare_player(
                    player,
                    id,
                    team_by_player[id],
                    &storage,
                    registry,
                    skill_import,
                    lazy_blueprint_players.contains(&id),
                )
            })
            .collect::<Vec<_>>();
        let player_alive = players.iter().map(|player| player.alive).collect();
        let players = players.into_iter().map(Some).collect();
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
            player_alive,
            input_groups,
            base_names_sorted: PreparedBattleInit::base_names_sorted(raw_groups),
            id_key_names,
            sorted_by_id_name,
            recycle_score_buffers: false,
        })
    }

    pub fn with_seed(&self, seed: &[String]) -> PreparedBattleInit { self.seed_state(seed).into_init(self.players.clone()) }

    pub fn into_with_seed(self, seed: &[String]) -> PreparedBattleInit {
        let seed_state = self.seed_state(seed);
        self.into_init_with_seed_state(seed_state)
    }

    /// 用已有 seed 缓冲区生成本轮初始化数据，保留所有小向量的容量。
    pub(crate) fn into_with_reused_seed(self, seed: &[String], mut state: PreparedBattleSeed) -> PreparedBattleInit {
        self.refill_seed_state(seed, &mut state);
        self.into_init_with_seed_state(state)
    }

    fn into_init_with_seed_state(self, state: PreparedBattleSeed) -> PreparedBattleInit {
        let Self {
            players,
            player_alive,
            input_groups,
            base_names_sorted,
            id_key_names,
            sorted_by_id_name,
            recycle_score_buffers,
        } = self;
        let score_buffers = recycle_score_buffers.then_some(ScoreRosterBuffers {
            players: Vec::new(),
            player_alive,
            input_groups,
            base_names_sorted,
            id_key_names,
            sorted_by_id_name,
        });
        state.into_init_with_score_buffers(players, score_buffers)
    }

    pub fn seed_state(&self, seed: &[String]) -> PreparedBattleSeed {
        let mut state = PreparedBattleSeed {
            input_groups: Vec::with_capacity(self.input_groups.len()),
            round_order: Vec::with_capacity(self.players.len()),
            team_roster: Vec::with_capacity(self.input_groups.len()),
            team_alive: Vec::with_capacity(self.input_groups.len()),
            flat_alive: Vec::with_capacity(self.players.len()),
            teams: vec![0; self.players.len()],
            speed_points: vec![0; self.players.len()],
            rng: RC4::default(),
            sort_ints: vec![0; self.players.len()],
            battle_groups: self.input_groups.clone(),
            rc4_key: String::new(),
        };
        self.refill_seed_state(seed, &mut state);
        state
    }

    /// 原地刷新 seed 状态，供同一 worker 的连续对局复用所有小向量容量。
    pub fn refill_seed_state(&self, seed: &[String], state: &mut PreparedBattleSeed) {
        PreparedBattleInit::refill_rc4_key_with_seed(&self.base_names_sorted, seed, &mut state.rc4_key);
        let mut rng = RC4::new(state.rc4_key.as_bytes(), 1);
        rng.js_xor_str(&state.rc4_key);

        state.sort_ints.clear();
        state.sort_ints.resize(self.players.len(), 0);
        for &id in &self.sorted_by_id_name {
            state.sort_ints[id] = rng.rFFFFFF() as i32;
        }

        state.battle_groups.clone_from(&self.input_groups);
        for group in &mut state.battle_groups {
            group.sort_by(|left, right| PreparedBattleInit::cmp_player_keys(&state.sort_ints, &self.id_key_names, *left, *right));
        }
        Self::refill_entity_groups(&mut state.input_groups, &state.battle_groups);
        state.battle_groups.sort_by(|left, right| match (left.first(), right.first()) {
            (Some(left), Some(right)) => PreparedBattleInit::cmp_player_keys(&state.sort_ints, &self.id_key_names, *left, *right),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        for group in &state.battle_groups {
            for player in group {
                rng.encrypt_bytes_no_change(&self.id_key_names[*player]);
            }
            rng.encrypt_bytes(&mut [0]);
        }

        state.teams.clear();
        state.teams.resize(self.players.len(), 0);
        for (team, group) in state.battle_groups.iter().enumerate() {
            for &player in group {
                state.teams[player] = team;
            }
        }

        state.round_order.clear();
        state
            .round_order
            .extend(state.battle_groups.iter().flatten().copied().map(PreparedBattleInit::entity_idx));
        state.round_order.sort_by(|left, right| {
            PreparedBattleInit::cmp_player_keys(&state.sort_ints, &self.id_key_names, left.0 as usize, right.0 as usize)
        });
        state.speed_points.clear();
        state.speed_points.resize(self.players.len(), 0);
        for &player in &state.round_order {
            state.speed_points[player.0 as usize] = rng.r255() as i32;
        }

        Self::refill_entity_groups(&mut state.team_roster, &state.battle_groups);
        Self::resize_nested_groups(&mut state.team_alive, state.battle_groups.len());
        for (alive, group) in state.team_alive.iter_mut().zip(&state.battle_groups) {
            alive.clear();
            alive.extend(
                group
                    .iter()
                    .copied()
                    .filter(|id| self.player_alive[*id])
                    .map(PreparedBattleInit::entity_idx),
            );
        }
        state.flat_alive.clear();
        state.flat_alive.extend(state.team_alive.iter().flatten().copied());
        state.rng = rng;
    }

    fn refill_entity_groups(target: &mut Vec<Vec<EntityIdx>>, source: &[Vec<PlrId>]) {
        Self::resize_nested_groups(target, source.len());
        for (target, source) in target.iter_mut().zip(source) {
            target.clear();
            target.extend(source.iter().copied().map(PreparedBattleInit::entity_idx));
        }
    }

    fn clone_player_groups_reusing(target: &mut Vec<Vec<PlrId>>, source: &[Vec<PlrId>]) {
        target.truncate(source.len());
        target.resize_with(source.len(), Vec::new);
        for (target, source) in target.iter_mut().zip(source) {
            target.clear();
            target.extend_from_slice(source);
        }
    }

    fn resize_nested_groups(groups: &mut Vec<Vec<EntityIdx>>, len: usize) {
        groups.truncate(len);
        groups.resize_with(len, Vec::new);
    }

    pub fn input_groups(&self) -> Vec<Vec<EntityIdx>> {
        self.input_groups.iter().map(|group| PreparedBattleInit::entity_order(group)).collect()
    }
}

impl PreparedBattleSeed {
    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    fn into_init(self, players: Vec<Option<PreparedPlayerInit>>) -> PreparedBattleInit {
        self.into_init_with_score_buffers(players, None)
    }

    fn into_init_with_score_buffers(
        self,
        mut players: Vec<Option<PreparedPlayerInit>>,
        score_buffers: Option<ScoreRosterBuffers>,
    ) -> PreparedBattleInit {
        for (index, player) in players.iter_mut().enumerate() {
            let Some(player) = player else {
                continue;
            };
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
            teams: self.teams,
            speed_points: self.speed_points,
            sort_ints: self.sort_ints,
            battle_groups: self.battle_groups,
            rc4_key: self.rc4_key,
            score_buffers,
        }
    }

    fn apply_entity_seed_values(
        teams: &[usize],
        speed_points: &[i32],
        runtime: &mut CombatRuntime,
    ) -> Result<(), RuntimeV2BattleInitError> {
        if teams.len() != runtime.entities.len() {
            return Err(RuntimeV2BattleInitError::EntityCountMismatch {
                prepared: teams.len(),
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
        for (index, (&team, &speed_points)) in teams.iter().zip(speed_points).enumerate() {
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

        Ok(())
    }

    fn apply_entities(&self, runtime: &mut CombatRuntime) -> Result<(), RuntimeV2BattleInitError> {
        Self::apply_entity_seed_values(&self.teams, &self.speed_points, runtime)
    }

    pub fn apply(self, runtime: &mut CombatRuntime) -> Result<(), RuntimeV2BattleInitError> {
        self.apply_entities(runtime)?;

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

    /// 将 seed 状态应用到已复位的 runtime，同时保留本对象和 world 内部向量的容量。
    pub fn apply_reusing(&self, runtime: &mut CombatRuntime) -> Result<(), RuntimeV2BattleInitError> {
        self.apply_entities(runtime)?;
        runtime.world.sync_initial_views_reusing(
            &runtime.entities,
            &self.round_order,
            &self.team_roster,
            &self.team_alive,
            &self.flat_alive,
        );
        runtime.rng.clone_from(&self.rng);
        runtime.scheduler.reset_action_mode_from_entities(&runtime.entities);
        Ok(())
    }
}

impl CombatRuntime {
    /// 为 score 的动态 profile 按需生成一类召唤物蓝图，并写回实体槽供本场复用。
    pub(crate) fn ensure_plain_minion_blueprint(
        &mut self,
        actor: EntityIdx,
        kind: crate::player::skill::act::minion::MinionKind,
    ) -> bool {
        use crate::player::skill::act::minion::MinionKind;

        let blueprint_export = match kind {
            MinionKind::Shadow => DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT,
            MinionKind::Summon => DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT,
            MinionKind::Zombie => DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT,
            MinionKind::Clone => return false,
        };
        let blueprint_slot = self
            .registry
            .entity_slot_id_by_export_name(blueprint_export)
            .unwrap_or_else(|| panic!("default runtime v2 profile must register {blueprint_export}"));
        match self.entities.get(actor).and_then(|entity| entity.slots.get(blueprint_slot)) {
            Some(SlotValue::PlayerTemplate(_)) => return true,
            Some(_) => panic!("runtime_v2 core minion blueprint slot has invalid value"),
            None => {}
        }

        let lazy_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core lazy blueprint rq slot");
        let (owner, team, child_clone_name_factor) = {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("runtime_v2 lazy blueprint owner disappeared: {}", actor.0));
            match entity.slots.get(lazy_slot) {
                Some(SlotValue::U64(_)) => {}
                Some(_) => panic!("runtime_v2 core lazy blueprint rq slot has invalid value"),
                None => return false,
            }
            let clone_build = entity
                .template
                .clone_build
                .as_ref()
                .unwrap_or_else(|| panic!("runtime_v2 lazy blueprint owner {} is missing clone build data", actor.0));
            let owner = MinionBlueprintOwner::plain(
                actor.0 as usize,
                entity.template.name.clone(),
                entity.template.clan_name.clone(),
                clone_build.attrs(),
                entity.template.at_boost_bits,
            );
            (owner, entity.runtime.team, clone_build.child_name_factor())
        };

        let skill_import = PlainLegacySkillImportMap::new_score_minions(&self.registry);
        let template = PreparedBattleInit::build_plain_score_minion_blueprint(
            &owner,
            team,
            &self.registry,
            &skill_import,
            child_clone_name_factor,
            kind,
        );
        self.entities
            .get_mut(actor)
            .unwrap()
            .slots
            .set(blueprint_slot, SlotValue::PlayerTemplate(Box::new(template)))
            .expect("runtime v2 core minion blueprint slot must exist");
        true
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
        self.apply_and_recover_seed(runtime).map(drop)
    }

    /// 应用本轮初始化并收回 seed 缓冲区，供同一 worker 的下一轮复用。
    pub(crate) fn apply_and_recover_seed(
        mut self,
        runtime: &mut CombatRuntime,
    ) -> Result<(PreparedBattleSeed, Option<ScoreRosterBuffers>), RuntimeV2BattleInitError> {
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
        let lazy_blueprint_rq_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT);
        let score_profile_reset = self.score_buffers.is_some();
        for (index, prepared) in self.players.iter_mut().enumerate() {
            let Some(prepared) = prepared.take() else {
                continue;
            };
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
            entity.template.name = prepared_template.name;
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
            entity.template.skills.prepare_hook_cache(&runtime.registry);
            entity.template.clone_build = prepared_template.clone_build;
            if score_profile_reset {
                entity
                    .runtime
                    .reset_score_profile_from_template(&entity.template, entity_idx, prepared.hp, prepared.alive);
            } else {
                entity.runtime = PlayerRuntime::from_template(&entity.template, &runtime.registry, entity_idx, entity_idx);
                entity.runtime.hp = prepared.hp;
                entity.runtime.alive = prepared.alive;
            }

            // score 的 prototype 带着首轮蓝图；每轮替换 profile 时必须先清掉旧值，
            // 否则延迟构造标记会错误地命中首轮模板。
            for slot in [shadow_blueprint_slot, summon_blueprint_slot, zombie_blueprint_slot]
                .into_iter()
                .flatten()
            {
                entity.slots.remove(slot);
            }
            if let Some(slot) = lazy_blueprint_rq_slot {
                entity.slots.remove(slot);
                if let Some(rq_bits) = prepared.lazy_blueprint_rq_bits {
                    entity
                        .slots
                        .set(slot, SlotValue::U64(rq_bits))
                        .expect("runtime v2 core lazy blueprint rq slot must exist");
                }
            }

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

        // 固定 target 虽然不需要重写冷模板，但 profile 名变化仍可能改变输入组排序，
        // 因此所有实体的本轮 team 与初始移动点都必须按 seed 结果刷新。
        PreparedBattleSeed::apply_entity_seed_values(&self.teams, &self.speed_points, runtime)?;

        runtime.world.sync_initial_views_reusing(
            &runtime.entities,
            &self.round_order,
            &self.team_roster,
            &self.team_alive,
            &self.flat_alive,
        );
        runtime.rng.clone_from(&self.rng);
        runtime.scheduler.reset_action_mode_from_entities(&runtime.entities);
        let seed = PreparedBattleSeed {
            input_groups: self.input_groups,
            round_order: self.round_order,
            team_roster: self.team_roster,
            team_alive: self.team_alive,
            flat_alive: self.flat_alive,
            teams: self.teams,
            speed_points: self.speed_points,
            rng: self.rng,
            sort_ints: self.sort_ints,
            battle_groups: self.battle_groups,
            rc4_key: self.rc4_key,
        };
        let score_buffers = self.score_buffers.map(|mut buffers| {
            buffers.players = self.players;
            buffers
        });
        Ok((seed, score_buffers))
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
        lazy_blueprints: bool,
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
        let blueprint_owner = (!lazy_blueprints).then(|| MinionBlueprintOwner::from_player(id, player));
        #[cfg(test)]
        let template_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let shadow_blueprint = blueprint_owner.as_ref().and_then(|owner| {
            registry
                .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
                .filter(|skill| skills.skills().contains(skill))
                .map(|_| Self::build_shadow_blueprint(owner, team, storage, registry, skill_import, child_clone_name_factor))
        });
        #[cfg(test)]
        let shadow_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let summon_blueprint = blueprint_owner.as_ref().and_then(|owner| {
            registry
                .skill_id_by_export_name(BuiltinActiveSkill::Summon.export_name())
                .filter(|skill| skills.skills().contains(skill))
                .map(|_| Self::build_summon_blueprint(owner, team, storage, registry, skill_import, child_clone_name_factor))
        });
        #[cfg(test)]
        let summon_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let zombie_blueprint = blueprint_owner.as_ref().and_then(|owner| {
            registry
                .skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
                .filter(|skill| skills.skills().contains(skill))
                .map(|_| Self::build_zombie_blueprint(owner, team, storage, registry, skill_import, child_clone_name_factor))
        });
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
            lazy_blueprint_rq_bits: lazy_blueprints.then_some(storage.eval_rq().to_bits()),
        }
    }

    /// 生成普通召唤物名字对应的 RC4 状态和 128 项名字底数。
    ///
    /// 召唤物不会继承 `!` / `\x02` profile 的特殊变换，因此这里直接复刻
    /// `Player::new_minion_and_init` 的普通名字路径，并避免临时 Vec。
    fn plain_minion_name_base(team: &str, name: &str) -> ([u8; 128], RC4) {
        assert!(name.len() <= crate::player::NAME_MAX_LEN, "召唤物名字过长");
        let mut name_key = [0u8; crate::player::NAME_MAX_LEN + 1];
        name_key[1..1 + name.len()].copy_from_slice(name.as_bytes());
        let mut rand = Player::score_profile_team_rng(team);
        rand.update(&name_key[..1 + name.len()], 2);

        let mut name_base = [0u8; 128];
        let mut output = 0usize;
        for &value in &rand.main_val {
            let mapped = ((u32::from(value) * 181) + 160) & 255;
            if (89..217).contains(&mapped) {
                name_base[output] = (mapped & 63) as u8;
                output += 1;
            }
        }
        assert_eq!(output, 128, "召唤物名字底数必须包含 128 项");
        (name_base, rand)
    }

    /// 从普通名字底数推导无武器、无 overlay 的原始八围。
    fn plain_minion_attrs(name_base: &[u8; 128]) -> [u32; 8] {
        let mut sorted_head: [u8; 10] = name_base[..10].try_into().expect("召唤物名字头长度固定");
        sorted_head.sort_unstable();
        let mut attrs = [0u32; 8];
        for (attr, offset) in attrs[..7].iter_mut().zip((10..31).step_by(3)) {
            *attr = u32::from(crate::player::median(
                name_base[offset],
                name_base[offset + 1],
                name_base[offset + 2],
            ));
        }
        attrs[7] =
            154 + u32::from(sorted_head[3]) + u32::from(sorted_head[4]) + u32::from(sorted_head[5]) + u32::from(sorted_head[6]);
        attrs
    }

    fn plain_minion_status(attrs: [u32; 8]) -> crate::player::PlayerStatus {
        let attack = attrs[0] as i32;
        let defense = attrs[1] as i32;
        let speed_attr = attrs[2] as i32;
        let agility = attrs[3] as i32;
        let magic = attrs[4] as i32;
        let resistance = attrs[5] as i32;
        let wisdom = attrs[6] as i32;
        let max_hp = attrs[7] as i32;
        let attr_sum = attrs[..7].iter().sum();
        let atk_sum = (attack - defense + speed_attr + magic - resistance) * 2 + agility + wisdom;
        crate::player::PlayerStatus {
            hp: max_hp,
            max_hp,
            attack,
            defense,
            speed: speed_attr + 160,
            agility,
            magic,
            magic_point: wisdom >> 1,
            resistance,
            wisdom,
            attr_sum,
            atk_sum,
            all_sum: attr_sum * 3 + attrs[7],
            ..crate::player::PlayerStatus::default()
        }
    }

    /// 直接构造普通 score profile 的召唤物模板，跳过完整 legacy Player 与技能对象。
    fn build_plain_score_minion_blueprint(
        owner: &MinionBlueprintOwner,
        team: usize,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        child_clone_name_factor: f64,
        kind: crate::player::skill::act::minion::MinionKind,
    ) -> PlayerTemplate {
        use crate::player::skill::act::minion::MinionKind;

        debug_assert!(owner.overlay(kind).is_none(), "数字直构只适用于无 overlay 召唤物");
        let suffix = match kind {
            MinionKind::Shadow => "shadow",
            MinionKind::Summon => "summon",
            MinionKind::Zombie => "zombie",
            MinionKind::Clone => unreachable!("分身不使用普通召唤物蓝图"),
        };
        let name = format!("{}?{suffix}", owner.base_name);
        let (name_base, mut rand) = Self::plain_minion_name_base(&owner.clan_name, &name);
        let mut attrs = Self::plain_minion_attrs(&name_base);

        let (display_name, player_kind, skills, speed_points) = match kind {
            MinionKind::Shadow => {
                attrs[7] /= 2;
                let raw = name_base[64..68].iter().copied().min().unwrap_or(0);
                let possess_level = ((i32::from(raw) - 10) / 2 + 36).max(0) as u32;
                let skills = skill_import.import_score_shadow_minion(possess_level);
                let player_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
                    .expect("runtime v2 数字幻影需要注册 core 幻影类型");
                ("幻影", player_kind, skills, if owner.at_boost() >= 3.0 { 2048 } else { -2048 })
            }
            MinionKind::Summon => {
                attrs[7] = (attrs[7] / 3).max(1);
                attrs[0] = 0;
                attrs[1] = owner.attrs[1];
                attrs[4] = 0;
                attrs[5] = owner.attrs[5];
                let levels = std::array::from_fn(|slot| {
                    let offset = 64 + slot * 4;
                    u32::from(name_base[offset..offset + 4].iter().copied().min().unwrap_or(0).saturating_sub(10))
                });
                let mut action_order = [0usize, 1, 2];
                rand.sort_list(&mut action_order);
                let skills = skill_import.import_score_summon_minion(levels, action_order);
                let player_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
                    .expect("runtime v2 数字使魔需要注册 core 使魔类型");
                ("使魔", player_kind, skills, 0)
            }
            MinionKind::Zombie => {
                attrs[0] = 0;
                attrs[6] = 0;
                attrs[7] = (attrs[7] >> 1).max(1);
                let player_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_ZOMBIE_KIND_EXPORT)
                    .expect("runtime v2 数字丧尸需要注册 core 丧尸类型");
                ("丧尸", player_kind, SkillLoadout::default(), 0)
            }
            MinionKind::Clone => unreachable!(),
        };

        let status = Self::plain_minion_status(attrs);
        let id_key_name = if owner.clan_name.is_empty() || owner.clan_name == name {
            name.clone()
        } else {
            format!("{name}@{}", owner.clan_name)
        };
        let mut template = PlayerTemplate::new(0, name.clone(), team, status.max_hp, status.attack)
            .with_identity_names(id_key_name, owner.clan_name.clone())
            .with_display_name(display_name)
            .with_magic(status.magic)
            .with_magic_point(status.magic_point)
            .with_wisdom(status.wisdom)
            .with_speed(status.speed)
            .with_def_res(status.defense, status.resistance)
            .with_agility(status.agility)
            .with_at_boost(status.at_boost)
            .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
            .with_speed_points(speed_points)
            .with_skill_loadout(skills);
        template.kind = player_kind;
        template.clone_build =
            Some(CloneBuildData::from_legacy(attrs, [0; 8], 0.0, &status).with_child_name_factor(child_clone_name_factor));
        if matches!(kind, MinionKind::Summon | MinionKind::Zombie) {
            template.reserved_player_ids_before_spawn = 1;
        }
        if kind == MinionKind::Summon {
            template.reuse_skills_on_recast = true;
            template.reuse_stats_on_recast = true;
            template.inherit_owner_def_res = true;
        }
        template
    }

    fn build_shadow_blueprint(
        owner: &MinionBlueprintOwner,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        child_clone_name_factor: f64,
    ) -> PlayerTemplate {
        let shadow = crate::player::skill::act::shadow::build_shadow_minion_from_owner(owner, storage);
        let shadow_skills = skill_import.import_storage(shadow.skill_storage());
        let shadow_kind = registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
            .expect("runtime v2 registry importing shadow must register core shadow kind");
        let mut template = Self::template_from_player(&shadow, 0, team, shadow_skills);
        template.kind = shadow_kind;
        template.clone_build = Some(Self::clone_build_from_player(&shadow, child_clone_name_factor));
        template
    }

    fn build_summon_blueprint(
        owner: &MinionBlueprintOwner,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        child_clone_name_factor: f64,
    ) -> PlayerTemplate {
        let summon_overlay = owner.overlay(crate::player::skill::act::minion::MinionKind::Summon);
        let summon = crate::player::skill::act::summon::build_summon_minion_from_owner(owner, storage, true);
        let summon_skills = skill_import.import_storage(summon.skill_storage());
        let summon_kind = registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("runtime v2 registry importing summon must register core summon kind");
        let mut template = Self::template_from_player(&summon, 0, team, summon_skills);
        template.kind = summon_kind;
        template.reserved_player_ids_before_spawn = 1;
        template.clone_build = Some(Self::clone_build_from_player(&summon, child_clone_name_factor));
        template.reuse_skills_on_recast = summon_overlay.is_none_or(|overlay| overlay.reuse_skills_on_recast);
        let has_overlay_attrs = summon_overlay.is_some_and(|overlay| overlay.attrs.is_some());
        template.reuse_stats_on_recast = !has_overlay_attrs;
        template.inherit_owner_def_res =
            !has_overlay_attrs || summon_overlay.is_some_and(|overlay| overlay.inherit_owner_def_res);
        template
    }

    fn build_zombie_blueprint(
        owner: &MinionBlueprintOwner,
        team: usize,
        storage: &std::sync::Arc<Storage>,
        registry: &ExtensionRegistry,
        skill_import: &PlainLegacySkillImportMap,
        child_clone_name_factor: f64,
    ) -> PlayerTemplate {
        let zombie = crate::player::skill::zombie::build_zombie_minion_blueprint_from_owner(owner, storage);
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

    fn refill_base_names_sorted(raw_groups: &[Vec<String>], names: &mut Vec<String>) {
        let player_count = raw_groups.iter().flatten().filter(|raw| !Player::check_is_seed(raw)).count();
        names.resize_with(player_count, String::new);
        let mut index = 0usize;
        for raw in raw_groups.iter().flatten().filter(|raw| !Player::check_is_seed(raw)) {
            Player::raw_namerena_to_idname_into(raw, &mut names[index]);
            index += 1;
        }
        names.truncate(index);
        names.sort();
        names.dedup();
    }

    fn refill_rc4_key_with_seed(base_names_sorted: &[String], seed: &[String], output: &mut String) {
        output.clear();
        if seed.is_empty() {
            for (index, name) in base_names_sorted.iter().enumerate() {
                if index > 0 {
                    output.push('\r');
                }
                output.push_str(name);
            }
            return;
        }

        let mut names = base_names_sorted.iter().chain(seed).collect::<smallvec::SmallVec<[&String; 8]>>();
        names.sort_unstable();
        names.dedup();
        for (index, name) in names.into_iter().enumerate() {
            if index > 0 {
                output.push('\r');
            }
            output.push_str(name);
        }
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

    fn two_score_profiles_mut(
        players: &mut [ScoreProfileBuild],
        left: usize,
        right: usize,
    ) -> (&mut ScoreProfileBuild, &mut ScoreProfileBuild) {
        assert_ne!(left, right, "runtime v2 score upgrade requested the same profile twice");
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

#[cfg(test)]
mod score_profile_tests {
    use super::*;

    #[test]
    fn compact_score_profiles_match_full_legacy_builds() {
        let config = default_custom_runtime_v2_import_config().expect("runtime v2 profile should build");
        let skill_import = PlainLegacySkillImportMap::new(&config.registry);
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let storage = Storage::new_arc_with_eval_rq(eval_rq);
        for modifier in ["!", "\u{0002}"] {
            let first_base = crate::engine::PROFILE_START as usize;
            let first_groups = vec![
                vec!["mario".to_owned(), format!("{first_base}@{modifier}")],
                vec![
                    format!("{}@{modifier}", first_base + 1),
                    format!("{}@{modifier}", first_base + 2),
                ],
            ];
            let cached = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import(
                &first_groups,
                eval_rq,
                &config.registry,
                &skill_import,
            )
            .expect("score target cache should build");
            let profile_ids = [1usize, 2, 3];
            let team_rng = Player::score_profile_team_rng(modifier);

            for round in 0..64 {
                let base = first_base + round * profile_ids.len();
                let groups = vec![
                    vec!["mario".to_owned(), format!("{base}@{modifier}")],
                    vec![format!("{}@{modifier}", base + 1), format!("{}@{modifier}", base + 2)],
                ];
                let expected = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import_selected(
                    &groups,
                    eval_rq,
                    &config.registry,
                    &skill_import,
                    &profile_ids,
                )
                .expect("full score profile should build");
                let actual = PreparedBattleRoster::from_score_groups_with_cached_targets(
                    &groups,
                    eval_rq,
                    &config.registry,
                    &skill_import,
                    &profile_ids,
                    modifier,
                    &team_rng,
                    &cached,
                    &mut [],
                    &mut [],
                    &mut ScoreRoundScratch::default(),
                    &mut ScoreRosterBuffers::default(),
                )
                .expect("compact score profile should build");

                for id in profile_ids {
                    assert_eq!(
                        actual.players[id], expected.players[id],
                        "modifier={modifier:?}, round={round}, id={id}"
                    );

                    let template = &actual.players[id].as_ref().expect("数字 profile 必须存在").template;
                    let owner = MinionBlueprintOwner::plain(
                        id,
                        template.name.clone(),
                        template.clan_name.clone(),
                        template.clone_build.as_ref().expect("数字 profile 必须含分身数据").attrs(),
                        template.at_boost_bits,
                    );
                    let factor_name = crate::player::eval_name::eval_str_common_with_rq(&owner.base_name, true, eval_rq);
                    let factor_team = crate::player::eval_name::eval_str_common_with_rq(&owner.clan_name, true, eval_rq);
                    let child_factor = factor_name.max(factor_team - 6.0);
                    for kind in [
                        crate::player::skill::act::minion::MinionKind::Shadow,
                        crate::player::skill::act::minion::MinionKind::Summon,
                        crate::player::skill::act::minion::MinionKind::Zombie,
                    ] {
                        let legacy = match kind {
                            crate::player::skill::act::minion::MinionKind::Shadow => PreparedBattleInit::build_shadow_blueprint(
                                &owner,
                                template.team,
                                &storage,
                                &config.registry,
                                &skill_import,
                                child_factor,
                            ),
                            crate::player::skill::act::minion::MinionKind::Summon => PreparedBattleInit::build_summon_blueprint(
                                &owner,
                                template.team,
                                &storage,
                                &config.registry,
                                &skill_import,
                                child_factor,
                            ),
                            crate::player::skill::act::minion::MinionKind::Zombie => PreparedBattleInit::build_zombie_blueprint(
                                &owner,
                                template.team,
                                &storage,
                                &config.registry,
                                &skill_import,
                                child_factor,
                            ),
                            crate::player::skill::act::minion::MinionKind::Clone => unreachable!(),
                        };
                        let compact = PreparedBattleInit::build_plain_score_minion_blueprint(
                            &owner,
                            template.team,
                            &config.registry,
                            &skill_import,
                            child_factor,
                            kind,
                        );
                        assert_eq!(
                            compact, legacy,
                            "召唤物模板不一致：modifier={modifier:?}, round={round}, id={id}, kind={kind:?}"
                        );
                    }
                }
                assert_eq!(actual.player_alive, expected.player_alive);
                assert_eq!(actual.input_groups, expected.input_groups);
                assert_eq!(actual.base_names_sorted, expected.base_names_sorted);
                assert_eq!(actual.id_key_names, expected.id_key_names);
                assert_eq!(actual.sorted_by_id_name, expected.sorted_by_id_name);
            }
        }
    }
}
