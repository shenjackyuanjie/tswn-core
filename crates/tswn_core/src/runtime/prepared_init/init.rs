use super::*;

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
    ) -> Result<Self, RuntimeBattleInitError> {
        Self::from_groups_with_eval_rq(raw_groups, seed, crate::player::eval_name::DEFAULT_EVAL_RQ, registry)
    }

    pub fn from_groups_with_eval_rq(
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
        registry: &ExtensionRegistry,
    ) -> Result<Self, RuntimeBattleInitError> {
        Ok(PreparedBattleRoster::from_groups_with_eval_rq(raw_groups, eval_rq, registry)?.with_seed(seed))
    }

    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    pub fn apply(self, runtime: &mut CombatRuntime) -> Result<(), RuntimeBattleInitError> {
        self.apply_and_recover_seed(runtime).map(drop)
    }

    /// 应用本轮初始化并收回 seed 缓冲区，供同一 worker 的下一轮复用。
    pub(crate) fn apply_and_recover_seed(
        mut self,
        runtime: &mut CombatRuntime,
    ) -> Result<(PreparedBattleSeed, Option<ScoreRosterBuffers>), RuntimeBattleInitError> {
        if self.players.len() != runtime.entities.len() {
            return Err(RuntimeBattleInitError::EntityCountMismatch {
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
                .unwrap_or_else(|| panic!("runtime entity disappeared during battle init: {}", entity_idx.0));
            if score_profile_reset {
                entity.states.clear_score_profile_for_reuse();
                entity.slots.clear();
            }
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
            if !score_profile_reset {
                for slot in [shadow_blueprint_slot, summon_blueprint_slot, zombie_blueprint_slot]
                    .into_iter()
                    .flatten()
                {
                    entity.slots.remove(slot);
                }
            }
            if let Some(slot) = lazy_blueprint_rq_slot {
                if !score_profile_reset {
                    entity.slots.remove(slot);
                }
                if let Some(rq_bits) = prepared.lazy_blueprint_rq_bits {
                    entity
                        .slots
                        .set(slot, SlotValue::U64(rq_bits))
                        .expect("runtime core lazy blueprint rq slot must exist");
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
                        .expect("default runtime profile must register saitama boss state");
                    entity.states.add_entry(StateEntry::saitama_boss(
                        PLAIN_SAITAMA_BOSS_STATE_KEY,
                        state_id,
                        SkillPriority(i32::MAX),
                    ));
                }
            }

            if let Some(template) = prepared.shadow_blueprint {
                let slot = shadow_blueprint_slot.expect("runtime shadow skill requires the core shadow blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(template))
                    .expect("runtime core shadow blueprint slot must exist");
            }
            if let Some(template) = prepared.summon_blueprint {
                let slot = summon_blueprint_slot.expect("runtime summon skill requires the core summon blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(template))
                    .expect("runtime core summon blueprint slot must exist");
            }
            if let Some(template) = prepared.zombie_blueprint {
                let slot = zombie_blueprint_slot.expect("runtime zombie skill requires the core zombie blueprint entity slot");
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(template))
                    .expect("runtime core zombie blueprint slot must exist");
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

    pub(super) fn apply_team_upgrades(players: &mut [Player], groups: &mut [Vec<PlrId>]) {
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

    pub(super) fn build_players(players: &mut [Player]) {
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

    pub(super) fn set_prepared_team(prepared: &mut PreparedPlayerInit, team: usize) {
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

    pub(super) fn prepare_player(
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
                .expect("default runtime profile must register core boss kind"),
            PlayerType::Boost => registry
                .player_kind_id_by_export_name(DEFAULT_CORE_BOOST_KIND_EXPORT)
                .expect("default runtime profile must register core boost kind"),
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
                .map(|_| {
                    Box::new(Self::build_shadow_blueprint(
                        owner,
                        team,
                        storage,
                        registry,
                        skill_import,
                        child_clone_name_factor,
                    ))
                })
        });
        #[cfg(test)]
        let shadow_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let summon_blueprint = blueprint_owner.as_ref().and_then(|owner| {
            registry
                .skill_id_by_export_name(BuiltinActiveSkill::Summon.export_name())
                .filter(|skill| skills.skills().contains(skill))
                .map(|_| {
                    Box::new(Self::build_summon_blueprint(
                        owner,
                        team,
                        storage,
                        registry,
                        skill_import,
                        child_clone_name_factor,
                    ))
                })
        });
        #[cfg(test)]
        let summon_elapsed = phase_started.elapsed();
        #[cfg(test)]
        let phase_started = std::time::Instant::now();
        let zombie_blueprint = blueprint_owner.as_ref().and_then(|owner| {
            registry
                .skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
                .filter(|skill| skills.skills().contains(skill))
                .map(|_| {
                    Box::new(Self::build_zombie_blueprint(
                        owner,
                        team,
                        storage,
                        registry,
                        skill_import,
                        child_clone_name_factor,
                    ))
                })
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
                "[runtime_prepared_player] id={id} name={:?} skills={} skill={}ns template={}ns shadow={}ns summon={}ns zombie={}ns blueprints={}/{}/{}",
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
    pub(super) fn build_plain_score_minion_blueprint(
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
                    .expect("runtime 数字幻影需要注册 core 幻影类型");
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
                    .expect("runtime 数字使魔需要注册 core 使魔类型");
                ("使魔", player_kind, skills, 0)
            }
            MinionKind::Zombie => {
                attrs[0] = 0;
                attrs[6] = 0;
                attrs[7] = (attrs[7] >> 1).max(1);
                let player_kind = registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_ZOMBIE_KIND_EXPORT)
                    .expect("runtime 数字丧尸需要注册 core 丧尸类型");
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
            .expect("runtime registry importing shadow must register core shadow kind");
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
            .expect("runtime registry importing summon must register core summon kind");
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
            .expect("runtime registry importing zombie must register core zombie kind");
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

    pub(super) fn base_names_sorted(raw_groups: &[Vec<String>]) -> Vec<String> {
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

    pub(super) fn refill_base_names_sorted(raw_groups: &[Vec<String>], names: &mut Vec<String>) {
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

    pub(super) fn refill_rc4_key_with_seed(base_names_sorted: &[String], seed: &[String], output: &mut String) {
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

        if let [seed_name] = seed {
            let mut first = true;
            let mut seed_written = false;
            for name in base_names_sorted {
                if !seed_written && seed_name < name {
                    Self::push_rc4_key_name(output, &mut first, seed_name);
                    seed_written = true;
                } else if seed_name == name {
                    seed_written = true;
                }
                Self::push_rc4_key_name(output, &mut first, name);
            }
            if !seed_written {
                Self::push_rc4_key_name(output, &mut first, seed_name);
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

    #[inline]
    fn push_rc4_key_name(output: &mut String, first: &mut bool, name: &str) {
        if !*first {
            output.push('\r');
        }
        output.push_str(name);
        *first = false;
    }

    /// 为批量胜率的连续数字 seed 预计算第一轮 KSA 的稳定前缀。
    ///
    /// 实际密钥每轮仍会逐字节校验前缀；名字排序位置发生变化时自动回退完整 KSA。
    pub(super) fn profile_seed_rc4_prefix(base_names_sorted: &[String]) -> Option<Rc4KeySchedulePrefix> {
        let seed = format!("seed:{}@!", crate::engine::PROFILE_START as usize + 1);
        let mut key = String::new();
        Self::refill_rc4_key_with_seed(base_names_sorted, std::slice::from_ref(&seed), &mut key);
        let seed_offset = key.find(&seed)?;
        let prefix_len = (seed_offset + "seed:".len()).min(crate::rc4::VAL_LEN);
        (prefix_len != 0).then(|| Rc4KeySchedulePrefix::new(&key.as_bytes()[..prefix_len]))
    }

    pub(super) fn cmp_player_keys(sort_ints: &[i32], id_key_names: &[String], left: PlrId, right: PlrId) -> std::cmp::Ordering {
        sort_ints[left]
            .cmp(&sort_ints[right])
            .then_with(|| id_key_names[left].cmp(&id_key_names[right]))
            .then_with(|| left.cmp(&right))
    }

    fn two_players_mut(players: &mut [Player], left: PlrId, right: PlrId) -> (&mut Player, &mut Player) {
        assert_ne!(left, right, "runtime battle init requested the same player twice");
        if left < right {
            let (before_right, from_right) = players.split_at_mut(right);
            (&mut before_right[left], &mut from_right[0])
        } else {
            let (before_left, from_left) = players.split_at_mut(left);
            (&mut from_left[0], &mut before_left[right])
        }
    }

    pub(super) fn two_score_profiles_mut(
        players: &mut [ScoreProfileBuild],
        left: usize,
        right: usize,
    ) -> (&mut ScoreProfileBuild, &mut ScoreProfileBuild) {
        assert_ne!(left, right, "runtime score upgrade requested the same profile twice");
        if left < right {
            let (before_right, from_right) = players.split_at_mut(right);
            (&mut before_right[left], &mut from_right[0])
        } else {
            let (before_left, from_left) = players.split_at_mut(left);
            (&mut from_left[0], &mut before_left[right])
        }
    }

    pub(super) fn entity_order(players: &[PlrId]) -> Vec<EntityIdx> { players.iter().copied().map(Self::entity_idx).collect() }

    pub(super) fn entity_idx(player: PlrId) -> EntityIdx {
        EntityIdx(player.try_into().expect("runtime prepared player id overflowed entity index"))
    }
}

#[cfg(test)]
mod score_profile_tests {
    use super::*;

    #[test]
    fn single_seed_rc4_key_merge_matches_sorted_reference() {
        let base_names = vec!["alpha".to_owned(), "middle".to_owned(), "zulu".to_owned()];
        for seed_name in ["0", "alpha", "seed:33554432@!", "zzzz"] {
            let seed = vec![seed_name.to_owned()];
            let mut actual = String::new();
            PreparedBattleInit::refill_rc4_key_with_seed(&base_names, &seed, &mut actual);

            let mut expected_names = base_names.iter().chain(&seed).map(String::as_str).collect::<Vec<_>>();
            expected_names.sort_unstable();
            expected_names.dedup();
            assert_eq!(actual, expected_names.join("\r"), "seed={seed_name:?}");
        }
    }

    #[test]
    fn accelerated_score_name_base_matches_scalar_filter() {
        for key in [b"!".as_slice(), b"33554431".as_slice(), b"score-profile-key".as_slice()] {
            let rng = RC4::new(key, 3);
            assert_eq!(score_name_base(&rng.main_val), score_name_base_scalar(&rng.main_val));
        }
    }

    #[test]
    fn compact_score_profiles_match_full_legacy_builds() {
        let config = default_custom_runtime_import_config().expect("runtime profile should build");
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
