use super::*;

impl CustomBed2Import {
    /// 解析 bed2 行格式：名字、可选队伍标记与 `+` 段体力；无法解析返回 None。
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

    /// 按玩家外观格式解析 bed2 行；体力取外观值，缺省时用标记值。
    pub fn parse_player_facade_raw(raw: &str) -> Option<Self> {
        let marker_import = Self::parse(raw)?;
        let id_name = crate::namerena::raw_namerena_to_id_name(raw.trim());
        let (name, team, facade_hp) = Self::parse_facade_id_name(&id_name);
        Some(Self {
            name,
            team,
            hp: facade_hp.unwrap_or(marker_import.hp),
        })
    }

    /// 把导入结果转成 PlayerTemplate：默认防抗、身份名与召唤技能。
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

    /// 把 bed2 roster 转换成准备模板。
    pub fn roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomBed2RosterImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    /// 把 bed2 roster 转换成准备模板，并写入召唤物模板槽。
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

    /// 把 bed2 roster 转换成准备模板，并写入幻影模板槽。
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

    /// 把 bed2 roster 转换成准备模板，并写入亡灵模板槽。
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

    /// 把 bed2 roster 转换成准备模板，并写入全部使魔模板槽。
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

    /// 在准备模板上写召唤 overlay：解析火与爆炸技能 ID，把 roster 中第一个召唤 overlay 落成模板槽。
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

    /// 在准备模板上写影袭 overlay：解析附身技能 ID，把 roster 中第一个影袭 overlay 落成模板槽。
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

    /// 在准备模板上写亡灵 overlay：把 roster 中第一个亡灵 overlay 落成模板槽。
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

    /// 一次性写全部使魔 overlay：召唤、影袭与亡灵共用同一准备模板；各类技能导出名缺失时报对应变体错误。
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

    /// 把 bed2 roster 逐行转成 PlayerTemplate；解析失败带队伍与行内位置。
    pub fn roster_into_player_templates(
        raw_groups: &[Vec<String>],
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomBed2RosterImportError> {
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                if crate::namerena::is_seed_line(raw.trim()) {
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

    /// 把混合 roster（bed2 行与 namerena 行）转换成准备模板。
    pub fn mixed_roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomMixedRosterImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    /// 把混合 roster 转换成准备模板，并写入召唤物模板槽。
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

    /// 把混合 roster 转换成准备模板，并写入幻影模板槽。
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

    /// 把混合 roster 转换成准备模板，并写入亡灵模板槽。
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

    /// 把混合 roster 转换成准备模板，并写入全部使魔模板槽。
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

    /// 把混合 roster 逐行转成 PlayerTemplate：bed2 行走 bed2 解析，其余走 namerena 构建。
    pub fn mixed_roster_into_player_templates(
        raw_groups: &[Vec<String>],
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomMixedRosterImportError> {
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                let raw_trimmed = raw.trim();
                if crate::namerena::is_seed_line(raw_trimmed) {
                    continue;
                }

                let template = if let Some(import) = Self::parse_player_facade_raw(raw_trimmed) {
                    import.into_player_template(next_id, bed2_kind, team_index, bed2_summon_skill)
                } else {
                    let input = crate::namerena::NamerenaInput::parse(raw).map_err(|error| CustomMixedRosterImportError {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                        message: error.to_string(),
                    })?;
                    let mut prepared =
                        crate::namerena::PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ)
                            .map_err(|error| match error {})?;
                    let player = prepared.players.pop().ok_or_else(|| CustomMixedRosterImportError {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                        message: "native namerena importer returned no player".to_owned(),
                    })?;
                    let status = player.status;
                    // 冴月麟参考属性允许负攻击；其余导入仍保持原有的非负约束。
                    let signed_rin =
                        player.class == crate::namerena::PlayerClass::Boss && player.name == "rin" && status.attack == -26;
                    if status.max_hp <= 0 || (status.attack < 0 && !signed_rin) || status.defense < 0 || status.resistance < 0 {
                        return Err(CustomMixedRosterImportError {
                            team_index,
                            player_index,
                            raw: raw.clone(),
                            message: format!(
                                "native player import produced unsupported status max_hp={} attack={} defense={} resistance={}",
                                status.max_hp, status.attack, status.defense, status.resistance
                            ),
                        });
                    }
                    PlayerTemplate::new(next_id, player.name, team_index, status.max_hp, status.attack)
                        .with_identity_names(player.id_key_name, player.clan_name)
                        .with_display_name(player.display_name)
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

    /// 解析门面 id_name：拆出名字、队伍与体力标记，`+` 段的体力优先于 `@` 段。
    fn parse_facade_id_name(id_name: &str) -> (String, Option<String>, Option<i32>) {
        let (base, plus_rest) = id_name.split_once('+').unwrap_or((id_name, ""));
        let (name, team, team_marker_hp) = Self::split_facade_name_team(base);
        let plus_marker_hp = Self::parse_bed2_plus_segments(plus_rest);
        (name, team, plus_marker_hp.or(team_marker_hp))
    }

    /// 按 `@` 拆分名字与队伍段，再解析队伍段里的 bed2 体力标记。
    fn split_facade_name_team(raw: &str) -> (String, Option<String>, Option<i32>) {
        if let Some((name, team)) = raw.split_once('@') {
            let (team, hp) = Self::split_bed2_team_marker(team.trim());
            (name.trim().to_owned(), team, hp)
        } else {
            (raw.trim().to_owned(), None, None)
        }
    }

    /// 解析队伍段中的 bed2 标记：纯 `bed2` 或 `名字@bed2` 表示使用默认体力，空队伍段返回 `None`。
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

    /// 取 `+` 分段中最后一个 `bed2[hp]` 体力标记。
    fn parse_bed2_plus_segments(raw: &str) -> Option<i32> { raw.split('+').filter_map(Self::parse_bed2_plus_marker).last() }

    /// 解析单个 `bed2[hp]` 段；hp 必须为正整数。
    fn parse_bed2_plus_marker(segment: &str) -> Option<i32> {
        let rest = segment.trim().strip_prefix("bed2[")?;
        let hp = rest.strip_suffix(']')?.trim().parse::<i32>().ok()?;
        (hp > 0).then_some(hp)
    }

    /// 扫描 roster，找到第一个带召唤 overlay 的玩家并生成召唤模板。
    fn first_summon_template_from_roster(
        raw_groups: &[Vec<String>],
        summon_kind: PlayerKindId,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::namerena::is_seed_line(raw.trim()) {
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

    /// 扫描 roster，找到第一个带影袭 overlay 的玩家并生成影袭模板。
    fn first_shadow_template_from_roster(
        raw_groups: &[Vec<String>],
        shadow_kind: PlayerKindId,
        possess_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::namerena::is_seed_line(raw.trim()) {
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

    /// 扫描 roster，找到第一个带亡灵 overlay 的玩家并生成亡灵模板；技能导出名缺失时返回导入错误。
    fn first_zombie_template_from_roster(
        raw_groups: &[Vec<String>],
        zombie_kind: PlayerKindId,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<Option<PlayerTemplate>, CustomBed2ZombieTemplateImportError> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::namerena::is_seed_line(raw.trim()) {
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

    /// 由召唤 overlay 生成 `名字?0` 模板：缺省属性偏防御与抗性，带火/爆炸技能，并可继承 owner 的防御抗性。
    fn summon_template_from_overlay(
        import: &Self,
        team: usize,
        summon_kind: PlayerKindId,
        overlay: &crate::namerena::MinionOverlay,
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

    /// 由影袭 overlay 生成 `名字?shadow` 模板：只带附身技能，行动点为负。
    fn shadow_template_from_overlay(
        import: &Self,
        team: usize,
        shadow_kind: PlayerKindId,
        overlay: &crate::namerena::MinionOverlay,
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

    /// 由亡灵 overlay 生成 `名字?zombie` 模板：技能按导出名前缀解析，行动点为 0。
    fn zombie_template_from_overlay(
        import: &Self,
        team: usize,
        zombie_kind: PlayerKindId,
        overlay: &crate::namerena::MinionOverlay,
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

    /// 由 overlay 技能表生成召唤技能配置：按名字映射 lane 组成行动序；没有可识别技能时退回默认三技能。
    fn summon_skill_loadout_from_overlay(
        overlay: &crate::namerena::MinionOverlay,
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

    /// 把召唤 overlay 技能名映射到固定 lane：火1、火2与爆炸分别为 0/1/2。
    fn summon_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::namerena::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::namerena::summon_slot_skill_ref_from_name(name))?;
        match skill_ref {
            crate::namerena::ClassifiedSkillRef::SummonFire1 => Some(0),
            crate::namerena::ClassifiedSkillRef::SummonFire2 => Some(1),
            crate::namerena::ClassifiedSkillRef::SummonExplode => Some(2),
            _ => None,
        }
    }

    /// 由 overlay 技能表生成影袭技能配置：只认附身 lane，没有可识别技能时退回默认。
    fn shadow_skill_loadout_from_overlay(overlay: &crate::namerena::MinionOverlay, possess_skill: SkillId) -> SkillLoadout {
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

    /// 把影袭 overlay 技能名映射到附身 lane（唯一 lane 为 0）。
    fn shadow_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::namerena::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::namerena::phantom_skill_ref_from_name(name))?;
        match skill_ref {
            crate::namerena::ClassifiedSkillRef::PhantomPossess => Some(0),
            _ => None,
        }
    }

    /// 由 overlay 技能表生成亡灵技能配置：按导出名（含前缀）解析技能 ID，缺失时报导入错误。
    fn zombie_skill_loadout_from_overlay(
        overlay: &crate::namerena::MinionOverlay,
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

    /// 解析亡灵 overlay 技能名对应的导出名后缀：先走分类技能引用，再走归一化名字匹配。
    fn zombie_overlay_skill_export_suffix(name: &str) -> Option<String> {
        match crate::namerena::player_classified_skill_ref_from_name(name) {
            Some(crate::namerena::ClassifiedSkillRef::Normal(skill_id)) => {
                return Some(Self::normal_skill_export_suffix(skill_id));
            }
            Some(crate::namerena::ClassifiedSkillRef::SummonFire1) => return Some("summon_fire1".to_owned()),
            Some(crate::namerena::ClassifiedSkillRef::SummonFire2) => return Some("summon_fire2".to_owned()),
            Some(crate::namerena::ClassifiedSkillRef::SummonExplode) => return Some("explode".to_owned()),
            Some(crate::namerena::ClassifiedSkillRef::PhantomPossess) => return Some("possess".to_owned()),
            None => {}
        }
        match Self::normalize_minion_overlay_skill_name(name).as_str() {
            "possess" | "possession" => Some("possess".to_owned()),
            "explode" | "selfdestruct" | "self_destruct" | "summonexplode" => Some("explode".to_owned()),
            _ => crate::namerena::skill_name_to_id(name).map(Self::normal_skill_export_suffix),
        }
    }

    /// 由技能编号生成导出名后缀：去掉 `skl` 前缀并转为小写。
    fn normal_skill_export_suffix(skill_id: usize) -> String {
        let export_name = crate::namerena::skill_name_for_export(skill_id);
        export_name.strip_prefix("skl").unwrap_or(export_name.as_str()).to_ascii_lowercase()
    }

    /// 归一化使魔 overlay 技能名：去掉 `skl` 或 `skill` 前缀并转小写。
    fn normalize_minion_overlay_skill_name(name: &str) -> String {
        let lower = name.trim().to_ascii_lowercase();
        lower
            .strip_prefix("skl")
            .or_else(|| lower.strip_prefix("skill"))
            .unwrap_or(lower.as_str())
            .to_string()
    }

    /// 从原始行解析玩家 overlay：按 JSON 外的 `+` 分段，取最后一段可解析的 overlay。
    fn player_overlay_from_raw(raw: &str) -> Option<crate::namerena::PlayerOverlay> {
        Self::split_by_plus_outside_json(raw)
            .into_iter()
            .filter_map(|segment| crate::namerena::PlayerOverlay::parse_inline(segment.trim()))
            .last()
    }

    /// 按 JSON 外的 `+` 拆分原始行：字符串、花括号与方括号内的加号不作为分隔符。
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
