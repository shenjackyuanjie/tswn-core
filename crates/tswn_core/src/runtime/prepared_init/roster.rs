use super::*;

impl PreparedBattleRoster {
    pub fn from_groups(raw_groups: &[Vec<String>], registry: &ExtensionRegistry) -> Result<Self, RuntimeBattleInitError> {
        Self::from_groups_with_eval_rq(raw_groups, crate::namerena::eval_name::DEFAULT_EVAL_RQ, registry)
    }

    pub fn from_groups_with_eval_rq(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
    ) -> Result<Self, RuntimeBattleInitError> {
        let skill_import = BuiltinSkillImportMap::new(registry);
        Self::from_groups_with_eval_rq_and_skill_import(raw_groups, eval_rq, registry, &skill_import)
    }

    pub fn from_groups_with_eval_rq_and_skill_import(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
        skill_import: &BuiltinSkillImportMap,
    ) -> Result<Self, RuntimeBattleInitError> {
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
        skill_import: &BuiltinSkillImportMap,
        profile_player_ids: &[PlrId],
        profile_team: &str,
        profile_team_rng: &RC4,
        cached: &Self,
        skill_buffers: &mut [SkillLoadout],
        identity_buffers: &mut [ScoreIdentityBuffer],
        scratch: &mut ScoreRoundScratch,
        roster_buffers: &mut ScoreRosterBuffers,
    ) -> Result<Self, RuntimeBattleInitError> {
        let player_count = raw_groups.iter().flatten().filter(|raw| !is_seed_line(raw)).count();
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
                if is_seed_line(raw) {
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
        // 相同，就必须让双方共同参与 upgrade，因此回退完整 roster 构造。
        for group in &cached.input_groups {
            if group.iter().any(|id| *id >= fixed_count)
                && group.iter().filter(|id| **id < fixed_count).any(|id| {
                    cached.players[*id]
                        .as_ref()
                        .expect("runtime score target cache must contain fixed players")
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
        scratch.name_keys.resize(scratch.dynamic_inputs.len(), [0u8; NAME_MAX_LEN + 1]);
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
        let shared_numeric_name_len = scratch.name_lengths.first().copied().filter(|&name_len| {
            scratch.name_lengths.iter().all(|length| *length == name_len)
                && scratch.dynamic_inputs.iter().all(|&(team_index, player_index, _)| {
                    raw_groups[team_index][player_index]
                        .split_once('@')
                        .is_some_and(|(name, _)| name.as_bytes().iter().all(u8::is_ascii_digit))
                })
        });
        let cached_child_clone_name_factor = shared_numeric_name_len.map(|name_len| {
            if !scratch.child_clone_name_factor_ready
                || scratch.factor_eval_rq_bits != eval_rq.to_bits()
                || scratch.factor_name_len != name_len
                || scratch.factor_team != profile_team
            {
                let (first_team, first_player, _) = scratch.dynamic_inputs[0];
                let first_name = raw_groups[first_team][first_player]
                    .split_once('@')
                    .expect("已经验证的 score profile 必须包含队名分隔符")
                    .0;
                let factor_name = crate::namerena::eval_name::eval_str_common_with_rq(first_name, true, eval_rq);
                let factor_team = crate::namerena::eval_name::eval_str_common_with_rq(profile_team, true, eval_rq);
                scratch.factor_eval_rq_bits = eval_rq.to_bits();
                scratch.factor_name_len = name_len;
                scratch.factor_team.clear();
                scratch.factor_team.push_str(profile_team);
                scratch.child_clone_name_factor = factor_name.max(factor_team - 6.0);
                scratch.child_clone_name_factor_ready = true;
            }
            scratch.child_clone_name_factor
        });
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
                cached_child_clone_name_factor,
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
            profile_seed_rc4_prefix: None,
            id_key_names,
            sorted_by_id_name,
            recycle_score_buffers: true,
        })
    }

    pub(super) fn from_groups_with_eval_rq_and_skill_import_selected(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        registry: &ExtensionRegistry,
        skill_import: &BuiltinSkillImportMap,
        lazy_blueprint_players: &[PlrId],
    ) -> Result<Self, RuntimeBattleInitError> {
        let mut groups = Vec::with_capacity(raw_groups.len());
        for (team_index, raw_group) in raw_groups.iter().enumerate() {
            let mut group = Vec::with_capacity(raw_group.len());
            for (player_index, raw) in raw_group.iter().enumerate() {
                if crate::namerena::is_seed_line(raw) {
                    continue;
                }
                let spec = crate::namerena::PlayerSpec::parse(raw).map_err(|error| RuntimeBattleInitError::Player {
                    team_index,
                    player_index,
                    raw: raw.clone(),
                    message: error.to_string(),
                })?;
                group.push(spec);
            }
            if !group.is_empty() {
                groups.push(group);
            }
        }
        let input = crate::namerena::NamerenaInput {
            groups,
            seed: Vec::new(),
        };
        let prepared = match crate::namerena::PreparedRoster::build(&input, eval_rq) {
            Ok(prepared) => prepared,
            Err(error) => match error {},
        };
        let player_count = prepared.players.len();
        let input_groups = prepared.groups;
        let id_key_names = prepared.players.iter().map(|player| player.id_key_name.clone()).collect::<Vec<_>>();
        let mut sorted_by_id_name = (0..player_count).collect::<Vec<_>>();
        sorted_by_id_name.sort_by(|left, right| id_key_names[*left].cmp(&id_key_names[*right]));
        let mut team_by_player = vec![0; player_count];
        for (team, group) in input_groups.iter().enumerate() {
            for player in group {
                team_by_player[*player] = team;
            }
        }
        let players = prepared
            .players
            .iter()
            .map(|player| {
                PreparedBattleInit::prepare_namerena_player(
                    player,
                    team_by_player[player.id],
                    registry,
                    skill_import,
                    eval_rq,
                    lazy_blueprint_players.contains(&player.id),
                )
            })
            .collect::<Vec<_>>();
        let player_alive = players.iter().map(|player| player.alive).collect();
        let players = players.into_iter().map(Some).collect();
        let base_names_sorted = PreparedBattleInit::base_names_sorted(raw_groups);
        let profile_seed_rc4_prefix = PreparedBattleInit::profile_seed_rc4_prefix(&base_names_sorted);
        Ok(Self {
            players,
            player_alive,
            input_groups,
            base_names_sorted,
            profile_seed_rc4_prefix,
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
            profile_seed_rc4_prefix: _,
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
        let mut rng = self
            .profile_seed_rc4_prefix
            .as_ref()
            .and_then(|prefix| RC4::new_with_key_schedule_prefix(state.rc4_key.as_bytes(), prefix))
            .unwrap_or_else(|| RC4::new(state.rc4_key.as_bytes(), 1));
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

#[cfg(test)]
mod native_roster_tests {
    use super::*;

    fn assert_rosters_equal(actual: &PreparedBattleRoster, expected: &PreparedBattleRoster, context: &str) {
        assert_eq!(actual.players, expected.players, "{context}: players");
        assert_eq!(actual.player_alive, expected.player_alive, "{context}: alive flags");
        assert_eq!(actual.input_groups, expected.input_groups, "{context}: groups");
        assert_eq!(actual.base_names_sorted, expected.base_names_sorted, "{context}: seed names");
        assert_eq!(actual.id_key_names, expected.id_key_names, "{context}: id names");
        assert_eq!(actual.sorted_by_id_name, expected.sorted_by_id_name, "{context}: id order");
        assert_eq!(
            actual.recycle_score_buffers, expected.recycle_score_buffers,
            "{context}: recycle flag"
        );
        assert_eq!(
            actual.profile_seed_rc4_prefix.as_ref().map(Rc4KeySchedulePrefix::len),
            expected.profile_seed_rc4_prefix.as_ref().map(Rc4KeySchedulePrefix::len),
            "{context}: seed prefix"
        );
    }

    #[test]
    fn native_roster_templates_are_deterministic() {
        let config = default_custom_runtime_import_config().expect("runtime profile should build");
        let skill_import = BuiltinSkillImportMap::new(&config.registry);
        let cases = [
            vec![
                vec!["alice@red+剁手刀".to_owned(), "bob@red".to_owned(), "seed:7@!".to_owned()],
                vec![
                    "covid@!".to_owned(),
                    "lazy@!".to_owned(),
                    "saitama@!".to_owned(),
                    "testsubject@!".to_owned(),
                ],
                vec![
                    "云剑狄卡敢@!".to_owned(),
                    "target@!".to_owned(),
                    "target@\u{0002}".to_owned(),
                    "target@\u{0003}".to_owned(),
                ],
            ],
            vec![
                vec![
                    r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklshadow":10,"sklsummon":10,"sklzombie":10},"shadow":{"attrs":[46,47,48,49,50,51,52,200],"skills":{"sklpossess":9}},"summon":{"attrs":[50,51,52,53,54,55,56,180],"skills":{"normal:sklrapid":9,"sklfire1":5,"summon:sklexplode":3},"reuse_skills_on_recast":true,"inherit_owner_def_res":true},"zombie":{"attrs":[40,41,42,43,44,45,46,90],"skills":{"sklrapid":7}}}"#.to_owned(),
                    r#"diy@same+diy[72,39,69,76,67,66,0,84]{"sklfire":5,"sklheal":"40+30"}"#.to_owned(),
                ],
                vec!["plain".to_owned()],
            ],
        ];

        for (case_index, groups) in cases.iter().enumerate() {
            let player_count = groups.iter().flatten().filter(|raw| !crate::namerena::is_seed_line(raw)).count();
            for lazy in [Vec::new(), (player_count > 0).then_some(vec![0]).unwrap_or_default()] {
                let actual = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import_selected(
                    groups,
                    crate::namerena::eval_name::DEFAULT_EVAL_RQ,
                    &config.registry,
                    &skill_import,
                    &lazy,
                )
                .unwrap();
                let expected = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import_selected(
                    groups,
                    crate::namerena::eval_name::DEFAULT_EVAL_RQ,
                    &config.registry,
                    &skill_import,
                    &lazy,
                )
                .unwrap();
                assert_rosters_equal(&actual, &expected, &format!("case {case_index}, lazy={lazy:?}"));
                assert_eq!(actual.players.len(), player_count);
                for (id, player) in actual.players.iter().enumerate() {
                    let player = player.as_ref().expect("native roster player must be prepared");
                    assert_eq!(player.template.id, id);
                    assert!(player.template.max_hp > 0);
                    assert_eq!(player.lazy_blueprint_rq_bits.is_some(), lazy.contains(&id));
                }
            }
        }
    }
}
