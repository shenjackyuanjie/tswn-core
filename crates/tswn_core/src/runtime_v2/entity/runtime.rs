use super::*;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MoveState {
    pub speed_points: i32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPolicyOverrides {
    pub owner_resolution: Option<OwnerResolutionPolicy>,
    pub damage_share: Option<DamageSharePolicy>,
    pub merge: Option<MergePolicy>,
    pub inherit_owner_def_res: Option<bool>,
}

impl PlayerPolicyOverrides {
    pub fn apply_to(self, mut policies: PlayerKindPolicies) -> PlayerKindPolicies {
        if let Some(owner_resolution) = self.owner_resolution {
            policies.owner_resolution = owner_resolution;
        }
        if let Some(damage_share) = self.damage_share {
            policies.damage_share = damage_share;
        }
        if let Some(merge) = self.merge {
            policies.merge = merge;
        }
        if let Some(inherit_owner_def_res) = self.inherit_owner_def_res {
            policies.inherit_owner_def_res = inherit_owner_def_res;
        }
        policies
    }

    pub fn with_damage_share(mut self, damage_share: DamageSharePolicy) -> Self {
        self.damage_share = Some(damage_share);
        self
    }

    pub fn with_inherit_owner_def_res(mut self, inherit_owner_def_res: bool) -> Self {
        self.inherit_owner_def_res = Some(inherit_owner_def_res);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectLinkRuntime {
    pub owner: EntityIdx,
    pub level: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HideRuntime {
    pub level: u32,
    pub attract_bits: u64,
    pub agility: i32,
    pub defense: i32,
    pub resistance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssassinateRuntime {
    pub fixed_lane: usize,
    pub target: EntityIdx,
    pub break_on_damage: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterRuntime {
    pub pending: bool,
    pub last_target: Option<EntityIdx>,
    pub last_updates_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeCorpseKind {
    #[default]
    None,
    Merge,
    Zombie,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
    pub attack: i32,
    pub magic: i32,
    pub magic_point: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_bits: u64,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
    pub kind: PlayerKindId,
    pub owner: EntityIdx,
    pub root_owner: EntityIdx,
    pub team: usize,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
    pub move_state: MoveState,
    pub charge: ChargeRuntime,
    pub accumulate: AccumulateRuntime,
    pub shield: i32,
    pub protect_to: Option<EntityIdx>,
    pub protect_from: Vec<ProtectLinkRuntime>,
    pub protect_pre_defend_skill_count: Option<usize>,
    pub upgrade_active: bool,
    pub hide: Option<HideRuntime>,
    pub assassinate: Option<AssassinateRuntime>,
    pub counter: CounterRuntime,
    pub corpse: RuntimeCorpseKind,
}

impl PlayerRuntime {
    pub fn from_template(
        template: &PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        root_owner: EntityIdx,
    ) -> Self {
        let (flags, policies) = registry
            .player_kind(template.kind)
            .map_or((PlayerKindFlags::NONE, PlayerKindPolicies::default()), |kind| {
                (kind.flags, template.policy_overrides.apply_to(kind.policies))
            });
        Self {
            hp: template.max_hp,
            alive: true,
            attack: template.attack,
            magic: template.magic,
            magic_point: template.magic_point,
            wisdom: template.wisdom,
            speed: template.speed,
            defense: template.defense,
            resistance: template.resistance,
            agility: template.agility,
            at_boost_bits: template.at_boost_bits,
            at_boost_millionths: template.at_boost_millionths,
            attr_sum: template.attr_sum,
            atk_sum: template.atk_sum,
            attract_bits: template.attract_bits,
            kind: template.kind,
            owner,
            root_owner,
            team: template.team,
            flags,
            policies,
            move_state: template.move_state,
            charge: ChargeRuntime::default(),
            accumulate: AccumulateRuntime::default(),
            shield: 0,
            protect_to: None,
            protect_from: Vec::new(),
            protect_pre_defend_skill_count: None,
            upgrade_active: false,
            hide: None,
            assassinate: None,
            counter: CounterRuntime::default(),
            corpse: RuntimeCorpseKind::None,
        }
    }

    pub fn at_boost(&self) -> f64 { f64::from_bits(self.at_boost_bits) }

    pub fn attract(&self) -> f64 { f64::from_bits(self.attract_bits) }

    pub fn is_minion(&self) -> bool { self.flags.contains(PlayerKindFlags::MINION) }

    pub fn is_combat_minion(&self) -> bool { self.flags.contains(PlayerKindFlags::COMBAT_MINION) }

    pub fn active(&self) -> bool { self.alive && self.hp > 0 }

    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        if !self.active() {
            return false;
        }
        let require_mp = randomer.r3x3() as i32;
        if self.magic_point < require_mp {
            return false;
        }
        self.magic_point -= require_mp;
        true
    }

    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.magic } else { self.attack };
        let a = {
            let mut temp = [
                randomer.r127() as i32,
                randomer.r127() as i32,
                randomer.r127() as i32,
                atk + 64,
                atk,
            ];
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        a * b * self.at_boost()
    }

    pub fn magic_defense(&self) -> i32 { self.resistance + 64 }

    pub fn magic_accuracy(&self) -> i32 { self.magic + self.agility }

    pub fn magic_dodge(&self) -> i32 { self.resistance + self.agility }

    pub fn dodge(accuracy: i32, dodge_value: i32, randomer: &mut RC4) -> bool {
        let chance = {
            let temp = 24 + dodge_value - accuracy;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= chance
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChargeRuntime {
    pub active: bool,
    pub post_action_active: bool,
    pub step: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccumulateRuntime {
    pub active: bool,
    pub acc_bits: u64,
    pub charge_bonus_bits: u64,
}

impl Default for AccumulateRuntime {
    fn default() -> Self {
        Self {
            active: false,
            acc_bits: 1.7000000476837158_f64.to_bits(),
            charge_bonus_bits: 0.0_f64.to_bits(),
        }
    }
}

impl AccumulateRuntime {
    pub fn acc(self) -> f64 { f64::from_bits(self.acc_bits) }

    pub fn charge_bonus(self) -> f64 { f64::from_bits(self.charge_bonus_bits) }

    fn set_acc(&mut self, acc: f64) { self.acc_bits = acc.to_bits(); }

    fn set_charge_bonus(&mut self, charge_bonus: f64) { self.charge_bonus_bits = charge_bonus.to_bits(); }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRecord {
    pub template: PlayerTemplate,
    pub runtime: PlayerRuntime,
    pub states: StateStore,
    pub slots: EntitySlotStorage,
}

impl EntityRecord {
    #[inline]
    pub fn is_active(&self) -> bool { self.runtime.active() && !self.states.is_frozen() }

    #[inline]
    pub fn effective_speed(&self) -> i32 {
        let mut speed = self.states.effective_speed(self.template.speed);
        if self.runtime.upgrade_active {
            speed += 20;
        }
        speed
    }

    pub fn apply_derived_stats(&mut self, stats: CloneDerivedStats) {
        self.template.apply_derived_stats(stats);
        self.refresh_runtime_stats_from_template();
    }

    pub fn refresh_runtime_stats_from_template(&mut self) {
        let hide_level = self.runtime.hide.map(|hide| hide.level);
        self.runtime.attack = self.template.attack;
        self.runtime.magic = self.template.magic;
        self.runtime.wisdom = self.template.wisdom;
        self.runtime.speed = self.template.speed;
        self.runtime.defense = self.template.defense;
        self.runtime.resistance = self.template.resistance;
        self.runtime.agility = self.template.agility;
        self.runtime.attr_sum = self.template.attr_sum;
        self.runtime.atk_sum = self.states.effective_atk_sum(self.template.atk_sum);
        self.runtime.attract_bits = self.template.attract_bits;
        self.runtime.attract_bits = self.states.effective_attract(self.runtime.attract()).to_bits();
        if self.runtime.upgrade_active {
            self.runtime.attack += 30;
            self.runtime.defense += 30;
            self.runtime.agility += 30;
            self.runtime.magic += 30;
            self.runtime.resistance += 30;
            self.runtime.speed += 20;
            self.runtime.wisdom += 20;
        }
        self.refresh_runtime_at_boost();
        if let Some(level) = hide_level {
            self.runtime.hide = Some(HideRuntime {
                level,
                attract_bits: self.runtime.attract_bits,
                agility: self.runtime.agility,
                defense: self.runtime.defense,
                resistance: self.runtime.resistance,
            });
            self.runtime.attract_bits = (self.runtime.attract() / 10.0).to_bits();
            if level > 63 {
                let boost = (level - 63) as i32;
                self.runtime.agility += boost;
                self.runtime.defense += boost;
                self.runtime.resistance += boost;
            }
        }
    }

    pub fn activate_upgrade_runtime(&mut self) -> bool {
        if self.runtime.upgrade_active {
            return false;
        }
        self.runtime.upgrade_active = true;
        self.runtime.move_state.speed_points += 400;
        self.refresh_runtime_stats_from_template();
        true
    }

    pub fn clear_upgrade_runtime(&mut self) -> bool {
        if !self.runtime.upgrade_active {
            return false;
        }
        self.runtime.upgrade_active = false;
        self.refresh_runtime_stats_from_template();
        true
    }

    pub fn activate_charge_runtime(&mut self) {
        self.runtime.charge.step += 2;
        self.runtime.charge.active = true;
        self.runtime.charge.post_action_active = true;
        self.refresh_runtime_at_boost();
    }

    pub fn tick_charge_post_action(&mut self) -> bool {
        if !self.runtime.charge.post_action_active {
            return false;
        }

        self.runtime.charge.step -= 1;
        if self.runtime.charge.step <= 0 {
            self.runtime.charge.active = false;
            self.runtime.charge.post_action_active = false;
            self.refresh_runtime_at_boost();
        }
        true
    }

    pub fn clear_charge_runtime(&mut self) -> bool {
        if !self.runtime.charge.active {
            return false;
        }

        self.runtime.charge.active = false;
        self.runtime.charge.post_action_active = false;
        self.refresh_runtime_at_boost();
        true
    }

    pub fn activate_accumulate_runtime(&mut self) -> bool {
        if self.runtime.accumulate.active {
            return false;
        }

        let charge_active = self.runtime.at_boost_millionths >= 3_000_000;
        self.runtime.accumulate.active = true;
        self.runtime.accumulate.set_charge_bonus(if charge_active { 1.0 } else { 0.0 });
        if charge_active {
            self.runtime.move_state.speed_points += 500;
        }
        self.refresh_runtime_at_boost();
        self.runtime.move_state.speed_points += 400;
        true
    }

    pub fn clear_accumulate_runtime(&mut self) -> bool {
        if !self.runtime.accumulate.active {
            return false;
        }

        self.runtime.accumulate.active = false;
        self.runtime.accumulate.set_acc(1.600000023841858);
        self.runtime.accumulate.set_charge_bonus(0.0);
        self.refresh_runtime_at_boost();
        true
    }

    pub fn clear_positive_runtime_messages(&mut self) -> Vec<(i32, &'static str)> {
        let emit_state_cancel = self.runtime.alive && self.runtime.hp > 0;
        let mut messages = Vec::new();
        if self.clear_accumulate_runtime() {
            messages.push((100, "[1]的[聚气]被打消了"));
        }
        if self.clear_charge_runtime() {
            messages.push((200, "[1]的[蓄力]被中止了"));
        }
        if self.clear_upgrade_runtime() && emit_state_cancel {
            messages.push((500, "[1]的[垂死]属性被打消"));
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    pub fn clear_positive_messages(&mut self) -> Vec<(i32, &'static str)> {
        let emit_state_cancel = self.runtime.alive && self.runtime.hp > 0;
        let mut messages = self.clear_positive_runtime_messages();
        messages.extend(self.states.clear_positive_states_with_ordered_messages(emit_state_cancel));
        self.refresh_runtime_stats_from_template();
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    fn refresh_runtime_at_boost(&mut self) {
        let mut at_boost = f64::from_bits(self.template.at_boost_bits);
        if self.runtime.charge.active {
            at_boost *= 3.0;
        }
        if self.runtime.accumulate.active {
            at_boost *= self.runtime.accumulate.acc() + self.runtime.accumulate.charge_bonus();
        }
        self.runtime.at_boost_bits = at_boost.to_bits();
        self.runtime.at_boost_millionths = at_boost_to_millionths(at_boost);
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityArena {
    entities: Vec<Option<EntityRecord>>,
}

impl EntityArena {
    pub fn from_templates(players: Vec<PlayerTemplate>) -> Self {
        Self::from_templates_with_registry(players, &ExtensionRegistry::default())
    }

    pub fn from_templates_with_registry(players: Vec<PlayerTemplate>, registry: &ExtensionRegistry) -> Self {
        let entities = players
            .into_iter()
            .enumerate()
            .map(|(idx, template)| {
                let owner = EntityIdx(idx as u32);
                let runtime = PlayerRuntime::from_template(&template, registry, owner, owner);
                Some(EntityRecord {
                    template,
                    runtime,
                    states: StateStore::default(),
                    slots: EntitySlotStorage::from_registry(registry),
                })
            })
            .collect();
        Self { entities }
    }

    pub fn len(&self) -> usize { self.entities.len() }

    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    pub fn get(&self, idx: EntityIdx) -> Option<&EntityRecord> { self.entities.get(idx.0 as usize).and_then(Option::as_ref) }

    pub fn get_mut(&mut self, idx: EntityIdx) -> Option<&mut EntityRecord> {
        self.entities.get_mut(idx.0 as usize).and_then(Option::as_mut)
    }

    pub fn next_spawn_idx(&self, template: &PlayerTemplate) -> EntityIdx {
        Self::next_spawn_idx_from_slot_count(self.entities.len(), template)
    }

    pub fn next_spawn_idx_from_slot_count(slot_count: usize, template: &PlayerTemplate) -> EntityIdx {
        let reserved =
            usize::try_from(template.reserved_player_ids_before_spawn).expect("runtime_v2 reserved player id count overflow");
        let idx = slot_count.checked_add(reserved).expect("runtime_v2 entity slot count overflow");
        EntityIdx(idx.try_into().expect("runtime_v2 entity index overflow"))
    }

    pub fn spawn_from_template(&mut self, template: PlayerTemplate, registry: &ExtensionRegistry) -> EntityIdx {
        self.spawn_from_template_with_owner(template, registry, None, None)
    }

    pub fn spawn_from_template_with_owner(
        &mut self,
        mut template: PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: Option<EntityIdx>,
        root_owner: Option<EntityIdx>,
    ) -> EntityIdx {
        let idx = self.next_spawn_idx(&template);
        let reserved =
            usize::try_from(template.reserved_player_ids_before_spawn).expect("runtime_v2 reserved player id count overflow");
        self.entities.extend(std::iter::repeat_n(None, reserved));
        template.id = idx.0 as usize + 1;
        let owner = owner.unwrap_or(idx);
        let root_owner = root_owner.unwrap_or(owner);
        if let Some(owner_idx) = Some(owner).filter(|owner_idx| *owner_idx != idx) {
            let owner_entity = self
                .get(owner_idx)
                .unwrap_or_else(|| panic!("unknown runtime_v2 spawn owner entity: {}", owner_idx.0));
            // Legacy `queue_spawn(owner, child)` assigns the child to the
            // owner's current world group. Blueprint teams are import-time
            // placeholders and must not decide runtime ownership.
            template.team = owner_entity.runtime.team;
            let policies = template.effective_policies(registry);
            if policies.inherit_owner_def_res {
                template.defense = owner_entity.runtime.defense;
                template.resistance = owner_entity.runtime.resistance;
            }
        }
        let runtime = PlayerRuntime::from_template(&template, registry, owner, root_owner);
        self.entities.push(Some(EntityRecord {
            template,
            runtime,
            states: StateStore::default(),
            slots: EntitySlotStorage::from_registry(registry),
        }));
        idx
    }

    pub fn iter(&self) -> impl Iterator<Item = (EntityIdx, &EntityRecord)> {
        self.entities
            .iter()
            .enumerate()
            .filter_map(|(idx, entity)| entity.as_ref().map(|entity| (EntityIdx(idx as u32), entity)))
    }
}
