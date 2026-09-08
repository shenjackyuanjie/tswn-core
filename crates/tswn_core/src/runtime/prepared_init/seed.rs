use super::*;

impl PreparedBattleSeed {
    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    /// 把本轮已排好的输入分组交给 runner，并接管 runner 上一轮的向量作为下次复用缓冲。
    pub(crate) fn swap_input_groups(&mut self, target: &mut Vec<Vec<EntityIdx>>) {
        std::mem::swap(target, &mut self.input_groups);
    }

    pub(super) fn into_init(self, players: Vec<Option<PreparedPlayerInit>>) -> PreparedBattleInit {
        self.into_init_with_score_buffers(players, None)
    }

    pub(super) fn into_init_with_score_buffers(
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

    pub(super) fn apply_entity_seed_values(
        teams: &[usize],
        speed_points: &[i32],
        runtime: &mut CombatRuntime,
    ) -> Result<(), RuntimeBattleInitError> {
        if teams.len() != runtime.entities.len() {
            return Err(RuntimeBattleInitError::EntityCountMismatch {
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
                .unwrap_or_else(|| panic!("runtime entity disappeared during seed reset: {}", entity_idx.0));
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
                entity.slots.update_player_template_team(slot, team);
            }
        }

        Ok(())
    }

    fn apply_entities(&self, runtime: &mut CombatRuntime) -> Result<(), RuntimeBattleInitError> {
        Self::apply_entity_seed_values(&self.teams, &self.speed_points, runtime)
    }

    pub fn apply(self, runtime: &mut CombatRuntime) -> Result<(), RuntimeBattleInitError> {
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
    pub fn apply_reusing(&self, runtime: &mut CombatRuntime) -> Result<(), RuntimeBattleInitError> {
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
    pub(crate) fn ensure_plain_minion_blueprint(&mut self, actor: EntityIdx, kind: crate::namerena::MinionKind) -> bool {
        let export = match kind {
            crate::namerena::MinionKind::Shadow => DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT,
            crate::namerena::MinionKind::Summon => DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT,
            crate::namerena::MinionKind::Zombie => DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT,
        };
        let slot = self.registry.entity_slot_id_by_export_name(export).expect("默认蓝图槽必须存在");
        if matches!(
            self.entities.get(actor).and_then(|e| e.slots.get(slot)),
            Some(SlotValue::PlayerTemplate(_))
        ) {
            return true;
        }
        let Some(template) = self.preview_plain_minion_blueprint(actor, kind) else {
            return false;
        };
        self.entities
            .get_mut(actor)
            .unwrap()
            .slots
            .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
            .expect("默认蓝图槽必须存在");
        true
    }

    /// 纯计算蓝图：可读取现有缓存，但不写槽位、不使用战斗随机数。
    pub(crate) fn preview_plain_minion_blueprint(
        &self,
        actor: EntityIdx,
        kind: crate::namerena::MinionKind,
    ) -> Option<PlayerTemplate> {
        use crate::namerena::MinionKind;

        let blueprint_export = match kind {
            MinionKind::Shadow => DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT,
            MinionKind::Summon => DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT,
            MinionKind::Zombie => DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT,
        };
        let blueprint_slot = self
            .registry
            .entity_slot_id_by_export_name(blueprint_export)
            .unwrap_or_else(|| panic!("default runtime profile must register {blueprint_export}"));
        match self.entities.get(actor).and_then(|entity| entity.slots.get(blueprint_slot)) {
            Some(SlotValue::PlayerTemplate(template)) => return Some((**template).clone()),
            Some(_) => panic!("runtime core minion blueprint slot has invalid value"),
            None => {}
        }

        let lazy_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)
            .expect("default runtime profile must register core lazy blueprint rq slot");
        let (base_name, clan_name, owner_attrs, at_boost, team, child_clone_name_factor) = {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("runtime lazy blueprint owner disappeared: {}", actor.0));
            match entity.slots.get(lazy_slot) {
                Some(SlotValue::U64(_)) => {}
                Some(_) => panic!("runtime core lazy blueprint rq slot has invalid value"),
                // 分身、幻影等战斗召唤物不会经过 score roster 的初始 profile，
                // 因而没有该标记；但它们也可能带有召唤类技能，需要按自身模板
                // 延迟构造下一层蓝图。
                None if entity.runtime.is_combat_minion() =>
                {
                    #[cfg(not(feature = "no_debug"))]
                    if std::env::var_os("TSWN_PROBE_MINION_BLUEPRINT").is_some() {
                        eprintln!(
                            "[probe:minion-blueprint] actor={} kind={kind:?} name={} source=combat-minion",
                            actor.0, entity.template.name
                        );
                    }
                }
                None => return None,
            }
            let clone_build = entity
                .template
                .clone_build
                .as_ref()
                .unwrap_or_else(|| panic!("runtime lazy blueprint owner {} is missing clone build data", actor.0));
            // JS 的 clone/minion 会把 `?N` 写到展示名，但内部名仍保留
            // `?shadow` / `?summon`。`id_key_name` 保存的是这份内部身份，
            // 这里继续拿它生成下一层蓝图，不能误用战报名。
            let base_name = entity
                .template
                .id_key_name
                .strip_suffix(&format!("@{}", entity.template.clan_name))
                .unwrap_or(&entity.template.name)
                .to_owned();
            (
                base_name,
                entity.template.clan_name.clone(),
                clone_build.attrs(),
                f64::from_bits(entity.template.at_boost_bits),
                entity.runtime.team,
                clone_build.child_name_factor(),
            )
        };

        let skill_import = BuiltinSkillImportMap::new_score_minions(&self.registry);
        let template = PreparedBattleInit::build_plain_score_minion_blueprint(
            &base_name,
            &clan_name,
            owner_attrs,
            at_boost,
            team,
            &self.registry,
            &skill_import,
            child_clone_name_factor,
            kind,
        );
        Some(template)
    }
}
