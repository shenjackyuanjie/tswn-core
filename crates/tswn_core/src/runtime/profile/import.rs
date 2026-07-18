use super::*;

impl CustomBed2Import {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        let (name, team, plus_rest, team_marker_hp) = if let Some((name, team_and_rest)) = raw.split_once('@') {
            let (team_part, plus_rest) = team_and_rest.split_once('+').unwrap_or((team_and_rest, ""));
            let (team, hp) = Self::split_bed2_team_marker(team_part.trim());
            (name.trim(), team, plus_rest, hp)
        } else if let Some((name, plus_rest)) = raw.split_once('+') {
            (name.trim(), None, plus_rest, None)
        } else {
            return None;
        };

        let hp = Self::parse_bed2_plus_segments(plus_rest).or(team_marker_hp)?;
        Some(Self {
            name: name.to_owned(),
            team,
            hp,
        })
    }

    pub fn parse_player_facade_raw(raw: &str) -> Option<Self> {
        let marker_import = Self::parse(raw)?;
        let id_name = crate::player::Player::raw_namerena_to_idname(raw.trim());
        let (name, team, facade_hp) = Self::parse_facade_id_name(&id_name);
        Some(Self {
            name,
            team,
            hp: facade_hp.unwrap_or(marker_import.hp),
        })
    }

    pub fn into_player_template(self, id: PlrId, kind: PlayerKindId, team: usize, summon_skill: SkillId) -> PlayerTemplate {
        let id_key_name = self
            .team
            .as_ref()
            .filter(|clan| !clan.is_empty() && *clan != &self.name)
            .map_or_else(|| self.name.clone(), |clan| format!("{}@{clan}", self.name));
        let clan_name = self.team.unwrap_or_else(|| self.name.clone());
        PlayerTemplate::with_kind(id, self.name, kind, team, self.hp, 0)
            .with_identity_names(id_key_name, clan_name)
            .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
            .with_skills([summon_skill])
    }

    pub fn roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomBed2RosterImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn roster_into_prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_summon_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_shadow_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_zombie_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_minion_overlays(raw_groups, registry, players, config)
    }

    fn prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let fire_skill = registry.skill_id_by_export_name(config.fire_skill_export_name).ok_or_else(|| {
            CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.fire_skill_export_name.to_owned(),
            }
        })?;
        let explode_skill = registry.skill_id_by_export_name(config.explode_skill_export_name).ok_or_else(|| {
            CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.explode_skill_export_name.to_owned(),
            }
        })?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(summon_template) =
            Self::first_summon_template_from_roster(raw_groups, config.summon_kind, fire_skill, explode_skill)
        {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(summon_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let possess_skill = registry.skill_id_by_export_name(config.possess_skill_export_name).ok_or_else(|| {
            CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: config.possess_skill_export_name.to_owned(),
            }
        })?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(shadow_template) = Self::first_shadow_template_from_roster(raw_groups, config.shadow_kind, possess_skill) {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(shadow_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let zombie_template =
            Self::first_zombie_template_from_roster(raw_groups, config.zombie_kind, &registry, config.skill_export_name_prefix)?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(zombie_template) = zombie_template {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(zombie_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let fire_skill = registry.skill_id_by_export_name(config.summon.fire_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Summon(CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.summon.fire_skill_export_name.to_owned(),
            })
        })?;
        let explode_skill = registry.skill_id_by_export_name(config.summon.explode_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Summon(CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.summon.explode_skill_export_name.to_owned(),
            })
        })?;
        let possess_skill = registry.skill_id_by_export_name(config.shadow.possess_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Shadow(CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: config.shadow.possess_skill_export_name.to_owned(),
            })
        })?;
        let zombie_template = Self::first_zombie_template_from_roster(
            raw_groups,
            config.zombie.zombie_kind,
            &registry,
            config.zombie.skill_export_name_prefix,
        )
        .map_err(CustomBed2MinionOverlayImportError::Zombie)?;

        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(summon_template) =
            Self::first_summon_template_from_roster(raw_groups, config.summon.summon_kind, fire_skill, explode_skill)
        {
            template.slots.set(
                config.summon.template_slot,
                SlotValue::PlayerTemplate(Box::new(summon_template)),
            )?;
        }
        if let Some(shadow_template) =
            Self::first_shadow_template_from_roster(raw_groups, config.shadow.shadow_kind, possess_skill)
        {
            template.slots.set(
                config.shadow.template_slot,
                SlotValue::PlayerTemplate(Box::new(shadow_template)),
            )?;
        }
        if let Some(zombie_template) = zombie_template {
            template.slots.set(
                config.zombie.template_slot,
                SlotValue::PlayerTemplate(Box::new(zombie_template)),
            )?;
        }
        Ok(template)
    }

    pub fn roster_into_player_templates(
        raw_groups: &[Vec<String>],
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomBed2RosterImportError> {
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    return Err(CustomBed2RosterImportError {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                    });
                };
                players.push(import.into_player_template(next_id, kind, team_index, summon_skill));
                next_id += 1;
            }
        }
        Ok(players)
    }

    pub fn mixed_roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomMixedRosterImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn mixed_roster_into_prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_summon_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_shadow_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_zombie_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_minion_overlays(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_player_templates(
        raw_groups: &[Vec<String>],
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomMixedRosterImportError> {
        let storage = crate::engine::storage::Storage::new_arc();
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                let raw_trimmed = raw.trim();
                if crate::player::Player::check_is_seed(raw_trimmed) {
                    continue;
                }

                let template = if let Some(import) = Self::parse_player_facade_raw(raw_trimmed) {
                    import.into_player_template(next_id, bed2_kind, team_index, bed2_summon_skill)
                } else {
                    let mut player =
                        crate::player::Player::new_from_namerena_raw(raw.clone(), storage.clone()).map_err(|error| {
                            CustomMixedRosterImportError {
                                team_index,
                                player_index,
                                raw: raw.clone(),
                                message: format!("{error:?}"),
                            }
                        })?;
                    player.build();
                    let status = player.get_status();
                    if status.max_hp <= 0 || status.attack < 0 || status.defense < 0 || status.resistance < 0 {
                        return Err(CustomMixedRosterImportError {
                            team_index,
                            player_index,
                            raw: raw.clone(),
                            message: format!(
                                "legacy player facade produced unsupported status max_hp={} attack={} defense={} resistance={}",
                                status.max_hp, status.attack, status.defense, status.resistance
                            ),
                        });
                    }
                    PlayerTemplate::new(next_id, player.id_name(), team_index, status.max_hp, status.attack)
                        .with_identity_names(player.id_key_name(), player.clan_name())
                        .with_display_name(player.display_name())
                        .with_magic(status.magic)
                        .with_magic_point(status.magic_point)
                        .with_wisdom(status.wisdom)
                        .with_agility(status.agility)
                        .with_at_boost(status.at_boost)
                        .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
                        .with_def_res(status.defense, status.resistance)
                };
                players.push(template);
                next_id += 1;
            }
        }
        Ok(players)
    }

    fn parse_facade_id_name(id_name: &str) -> (String, Option<String>, Option<i32>) {
        let (base, plus_rest) = id_name.split_once('+').unwrap_or((id_name, ""));
        let (name, team, team_marker_hp) = Self::split_facade_name_team(base);
        let plus_marker_hp = Self::parse_bed2_plus_segments(plus_rest);
        (name, team, plus_marker_hp.or(team_marker_hp))
    }

    fn split_facade_name_team(raw: &str) -> (String, Option<String>, Option<i32>) {
        if let Some((name, team)) = raw.split_once('@') {
            let (team, hp) = Self::split_bed2_team_marker(team.trim());
            (name.trim().to_owned(), team, hp)
        } else {
            (raw.trim().to_owned(), None, None)
        }
    }

    fn split_bed2_team_marker(team: &str) -> (Option<String>, Option<i32>) {
        if team == "bed2" {
            return (None, Some(DEFAULT_BED2_HP));
        }
        match team.rsplit_once('@') {
            Some((team, "bed2")) if !team.is_empty() => (Some(team.to_owned()), Some(DEFAULT_BED2_HP)),
            _ if team.is_empty() => (None, None),
            _ => (Some(team.to_owned()), None),
        }
    }

    fn parse_bed2_plus_segments(raw: &str) -> Option<i32> { raw.split('+').filter_map(Self::parse_bed2_plus_marker).last() }

    fn parse_bed2_plus_marker(segment: &str) -> Option<i32> {
        let rest = segment.trim().strip_prefix("bed2[")?;
        let hp = rest.strip_suffix(']')?.trim().parse::<i32>().ok()?;
        (hp > 0).then_some(hp)
    }

    fn first_summon_template_from_roster(
        raw_groups: &[Vec<String>],
        summon_kind: PlayerKindId,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(summon_overlay) = overlay.summon.as_ref() else {
                    continue;
                };
                return Some(Self::summon_template_from_overlay(
                    &import,
                    team_index,
                    summon_kind,
                    summon_overlay,
                    fire_skill,
                    explode_skill,
                ));
            }
        }
        None
    }

    fn first_shadow_template_from_roster(
        raw_groups: &[Vec<String>],
        shadow_kind: PlayerKindId,
        possess_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(shadow_overlay) = overlay.shadow.as_ref() else {
                    continue;
                };
                return Some(Self::shadow_template_from_overlay(
                    &import,
                    team_index,
                    shadow_kind,
                    shadow_overlay,
                    possess_skill,
                ));
            }
        }
        None
    }

    fn first_zombie_template_from_roster(
        raw_groups: &[Vec<String>],
        zombie_kind: PlayerKindId,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<Option<PlayerTemplate>, CustomBed2ZombieTemplateImportError> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(zombie_overlay) = overlay.zombie.as_ref() else {
                    continue;
                };
                return Ok(Some(Self::zombie_template_from_overlay(
                    &import,
                    team_index,
                    zombie_kind,
                    zombie_overlay,
                    registry,
                    skill_export_name_prefix,
                )?));
            }
        }
        Ok(None)
    }

    fn summon_template_from_overlay(
        import: &Self,
        team: usize,
        summon_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> PlayerTemplate {
        let attrs = overlay.attrs.unwrap_or([0, DEFAULT_BED2_DEFENSE, 0, 0, 0, DEFAULT_BED2_RESISTANCE, 0, 1]);
        let skills = Self::summon_skill_loadout_from_overlay(overlay, fire_skill, explode_skill);
        PlayerTemplate::with_kind(
            0,
            format!("{}?0", import.name),
            summon_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(attrs[2].max(0) + 160)
        .with_policy_overrides(PlayerPolicyOverrides::default().with_inherit_owner_def_res(overlay.inherit_owner_def_res))
        .with_skill_loadout(skills)
    }

    fn shadow_template_from_overlay(
        import: &Self,
        team: usize,
        shadow_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        possess_skill: SkillId,
    ) -> PlayerTemplate {
        let attrs = overlay.attrs.unwrap_or([0, 0, 0, 0, 0, 0, 0, 1]);
        let skills = Self::shadow_skill_loadout_from_overlay(overlay, possess_skill);
        PlayerTemplate::with_kind(
            0,
            format!("{}?shadow", import.name),
            shadow_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(-2048)
        .with_skill_loadout(skills)
    }

    fn zombie_template_from_overlay(
        import: &Self,
        team: usize,
        zombie_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<PlayerTemplate, CustomBed2ZombieTemplateImportError> {
        let attrs = overlay.attrs.unwrap_or([0, 0, 0, 0, 0, 0, 0, 1]);
        let skills = Self::zombie_skill_loadout_from_overlay(overlay, registry, skill_export_name_prefix)?;
        Ok(PlayerTemplate::with_kind(
            0,
            format!("{}?zombie", import.name),
            zombie_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(0)
        .with_skill_loadout(skills))
    }

    fn summon_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> SkillLoadout {
        let mut active_order = Vec::new();
        if let Some(skill_levels) = overlay.skills.as_ref() {
            for (name, _) in skill_levels {
                let Some(lane) = Self::summon_overlay_skill_lane(name) else {
                    continue;
                };
                if !active_order.contains(&lane) {
                    active_order.push(lane);
                }
            }
        }
        if active_order.is_empty() {
            active_order.extend([0, 1, 2]);
        }
        summon_default_skill_loadout(fire_skill, explode_skill, [0, 1, 2]).with_active_order(active_order)
    }

    fn summon_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::player::skill::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::player::skill::summon_slot_skill_ref_from_name(name))?;
        match skill_ref {
            crate::player::skill::ClassifiedSkillRef::SummonFire1 => Some(0),
            crate::player::skill::ClassifiedSkillRef::SummonFire2 => Some(1),
            crate::player::skill::ClassifiedSkillRef::SummonExplode => Some(2),
            _ => None,
        }
    }

    fn shadow_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        possess_skill: SkillId,
    ) -> SkillLoadout {
        let mut active_order = Vec::new();
        if let Some(skill_levels) = overlay.skills.as_ref() {
            for (name, _) in skill_levels {
                let Some(lane) = Self::shadow_overlay_skill_lane(name) else {
                    continue;
                };
                if !active_order.contains(&lane) {
                    active_order.push(lane);
                }
            }
        }
        if active_order.is_empty() {
            active_order.push(0);
        }
        SkillLoadout::from_skills([possess_skill]).with_active_order(active_order)
    }

    fn shadow_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::player::skill::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::player::skill::phantom_skill_ref_from_name(name))?;
        match skill_ref {
            crate::player::skill::ClassifiedSkillRef::PhantomPossess => Some(0),
            _ => None,
        }
    }

    fn zombie_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<SkillLoadout, CustomBed2ZombieTemplateImportError> {
        let Some(skill_levels) = overlay.skills.as_ref() else {
            return Ok(SkillLoadout::default());
        };
        let mut skills = Vec::new();
        for (raw_name, _) in skill_levels {
            let Some(suffix) = Self::zombie_overlay_skill_export_suffix(raw_name) else {
                continue;
            };
            let export_name = if skill_export_name_prefix.is_empty() {
                suffix
            } else {
                format!("{skill_export_name_prefix}.{suffix}")
            };
            let skill_id = registry.skill_id_by_export_name(&export_name).ok_or_else(|| {
                CustomBed2ZombieTemplateImportError::MissingSkillExportName {
                    export_name: export_name.clone(),
                }
            })?;
            if !skills.contains(&skill_id) {
                skills.push(skill_id);
            }
        }
        Ok(SkillLoadout::from_skills(skills))
    }

    fn zombie_overlay_skill_export_suffix(name: &str) -> Option<String> {
        match crate::player::skill::player_classified_skill_ref_from_name(name) {
            Some(crate::player::skill::ClassifiedSkillRef::Normal(skill_id)) => {
                return Some(Self::normal_skill_export_suffix(skill_id));
            }
            Some(crate::player::skill::ClassifiedSkillRef::SummonFire1) => return Some("summon_fire1".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::SummonFire2) => return Some("summon_fire2".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::SummonExplode) => return Some("explode".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::PhantomPossess) => return Some("possess".to_owned()),
            None => {}
        }
        match Self::normalize_minion_overlay_skill_name(name).as_str() {
            "possess" | "possession" => Some("possess".to_owned()),
            "explode" | "selfdestruct" | "self_destruct" | "summonexplode" => Some("explode".to_owned()),
            _ => crate::player::skill::skill_name_to_id(name).map(Self::normal_skill_export_suffix),
        }
    }

    fn normal_skill_export_suffix(skill_id: usize) -> String {
        let export_name = crate::player::skill::skill_name_for_export(skill_id);
        export_name.strip_prefix("skl").unwrap_or(export_name.as_str()).to_ascii_lowercase()
    }

    fn normalize_minion_overlay_skill_name(name: &str) -> String {
        let lower = name.trim().to_ascii_lowercase();
        lower
            .strip_prefix("skl")
            .or_else(|| lower.strip_prefix("skill"))
            .unwrap_or(lower.as_str())
            .to_string()
    }

    fn player_overlay_from_raw(raw: &str) -> Option<crate::player::overlay::PlayerOverlay> {
        Self::split_by_plus_outside_json(raw)
            .into_iter()
            .filter_map(|segment| crate::player::overlay::PlayerOverlay::parse_inline(segment.trim()))
            .last()
    }

    fn split_by_plus_outside_json(raw: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut escaped = false;
        let mut brace_depth = 0usize;
        let mut bracket_depth = 0usize;
        for ch in raw.chars() {
            if in_string {
                current.push(ch);
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
            } else if ch == '+' && brace_depth == 0 && bracket_depth == 0 {
                segments.push(std::mem::take(&mut current));
            } else {
                current.push(ch);
                match ch {
                    '"' => in_string = true,
                    '{' => brace_depth += 1,
                    '}' => brace_depth = brace_depth.saturating_sub(1),
                    '[' => bracket_depth += 1,
                    ']' => bracket_depth = bracket_depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        segments.push(current);
        segments
    }
}
