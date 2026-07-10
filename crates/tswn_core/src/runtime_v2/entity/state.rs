use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEntry {
    pub legacy_order_key: u32,
    pub extension_state_id: Option<StateId>,
    pub hook_mask: ProcMask,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
    pub payload: StatePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CovidInfectionEntry {
    pub boss: EntityIdx,
    pub mutation: i32,
    pub days: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum StatePayload {
    #[default]
    None,
    FireMagHalfSteps(i32),
    Ice {
        frozen_step: i32,
    },
    ShieldValue(i32),
    Curse {
        prob: i32,
        multiply: i32,
    },
    Poison {
        caster: Option<u32>,
        target: Option<u32>,
        atp_bits: u64,
        count: i32,
    },
    Haste {
        faster: i32,
        step: i32,
    },
    Berserk {
        step: i32,
    },
    Charm {
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
    },
    Slow {
        step: i32,
    },
    Iron {
        protect: i32,
        step: i32,
    },
    CovidBoss {
        mutation: i32,
    },
    CovidInfection {
        entries: SmallVec<[CovidInfectionEntry; 2]>,
        mutation_set: SmallVec<[i32; 4]>,
        recovered: bool,
    },
    SaitamaBoss {
        turns: i32,
        damages: i32,
        hitters: SmallVec<[EntityIdx; 8]>,
        minions: SmallVec<[EntityIdx; 8]>,
    },
    LazyBoss {
        at_boost_bits: u64,
    },
    LazyInfection {
        boss: EntityIdx,
    },
}

impl StateEntry {
    pub fn legacy(legacy_order_key: u32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::None,
        }
    }

    pub fn fire_mag(legacy_order_key: u32, half_steps: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::FireMagHalfSteps(half_steps),
        }
    }

    pub fn ice(legacy_order_key: u32, frozen_step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Ice { frozen_step },
        }
    }

    pub fn shield(legacy_order_key: u32, state_id: StateId, shield: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::ShieldValue(shield),
        }
    }

    pub fn curse(legacy_order_key: u32, state_id: StateId, prob: i32, multiply: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Curse { prob, multiply },
        }
    }

    pub fn poison(
        legacy_order_key: u32,
        state_id: StateId,
        caster: Option<u32>,
        target: Option<u32>,
        atp: f64,
        count: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Poison {
                caster,
                target,
                atp_bits: atp.to_bits(),
                count,
            },
        }
    }

    pub fn haste(legacy_order_key: u32, state_id: StateId, faster: i32, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Haste { faster, step },
        }
    }

    pub fn berserk(legacy_order_key: u32, step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Berserk { step },
        }
    }

    pub fn charm(
        legacy_order_key: u32,
        state_id: StateId,
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            },
        }
    }

    pub fn slow(legacy_order_key: u32, state_id: StateId, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Slow { step },
        }
    }

    pub fn iron(legacy_order_key: u32, state_id: StateId, protect: i32, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Iron { protect, step },
        }
    }

    pub fn covid_boss(legacy_order_key: u32, mutation: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidBoss { mutation },
        }
    }

    pub fn covid_infection(
        legacy_order_key: u32,
        state_id: StateId,
        boss: EntityIdx,
        mutation: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidInfection {
                entries: SmallVec::from_slice(&[CovidInfectionEntry { boss, mutation, days: 0 }]),
                mutation_set: SmallVec::from_slice(&[mutation]),
                recovered: false,
            },
        }
    }

    pub fn lazy_boss(legacy_order_key: u32, at_boost: f64) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyBoss {
                at_boost_bits: at_boost.to_bits(),
            },
        }
    }

    pub fn saitama_boss(legacy_order_key: u32, state_id: StateId, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::SaitamaBoss {
                turns: 0,
                damages: 0,
                hitters: SmallVec::new(),
                minions: SmallVec::new(),
            },
        }
    }

    pub fn lazy_infection(legacy_order_key: u32, state_id: StateId, boss: EntityIdx, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyInfection { boss },
        }
    }

    pub fn fire_mag_value(&self) -> Option<f64> {
        match &self.payload {
            StatePayload::FireMagHalfSteps(half_steps) => Some(f64::from(*half_steps) * 0.5),
            StatePayload::None
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn shield_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::ShieldValue(shield) => Some(*shield),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn haste_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Haste { faster, step } => Some((*faster, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn poison_value(&self) -> Option<(Option<u32>, Option<u32>, f64, i32)> {
        match &self.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                count,
            } => Some((*caster, *target, f64::from_bits(*atp_bits), *count)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn charm_value(&self) -> Option<(usize, Option<usize>, Option<usize>, Option<u32>, i32)> {
        match &self.payload {
            StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            } => Some((*group_id, *effective_team_idx, *source_team_idx, *target, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn slow_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Slow { step } => Some(*step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn ice_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Ice { frozen_step } => Some(*frozen_step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn iron_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Iron { protect, step } => Some((*protect, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. } => None,
            StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn priority_for_hook(&self, hook: ProcMask) -> SkillPriority {
        match &self.payload {
            StatePayload::Poison { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(150),
            StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. }
                if hook.intersects(ProcMask::POST_ACTION) =>
            {
                SkillPriority(210)
            }
            StatePayload::Iron { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(210),
            StatePayload::CovidInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            StatePayload::LazyInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            _ => self.priority,
        }
    }

    pub fn positive_clear_message(&self, owner_alive: bool) -> Option<(i32, &'static str)> {
        match &self.payload {
            StatePayload::Haste { .. } if owner_alive => Some((300, "[1]从[疾走]中解除")),
            StatePayload::Iron { .. } => Some((400, "[1]的[铁壁]被打消了")),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn is_positive_state(&self) -> bool {
        match &self.payload {
            StatePayload::ShieldValue(shield) => *shield > 0,
            StatePayload::Haste { .. } => true,
            StatePayload::Iron { step, .. } => *step > 0,
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => false,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateStore {
    entries: SmallVec<[StateEntry; 8]>,
    hook_mask: ProcMask,
    generation: u32,
    index: HashMap<u32, usize>,
}

impl StateStore {
    pub fn entries(&self) -> &[StateEntry] { &self.entries }

    pub fn hook_mask(&self) -> ProcMask { self.hook_mask }

    pub fn generation(&self) -> u32 { self.generation }

    pub fn is_frozen(&self) -> bool { self.entries.iter().any(|entry| matches!(&entry.payload, StatePayload::Ice { .. })) }

    pub fn effective_speed(&self, base_speed: i32) -> i32 {
        let mut speed = base_speed;
        for entry in &self.entries {
            match &entry.payload {
                StatePayload::Haste { faster, .. } => speed *= *faster,
                StatePayload::Slow { .. } => speed /= 2,
                StatePayload::LazyInfection { .. } => speed /= 2,
                _ => {}
            }
        }
        speed
    }

    pub fn entry(&self, legacy_order_key: u32) -> Option<&StateEntry> {
        self.index.get(&legacy_order_key).and_then(|idx| self.entries.get(*idx))
    }

    pub fn entry_mut(&mut self, legacy_order_key: u32) -> Option<&mut StateEntry> {
        let idx = self.index.get(&legacy_order_key).copied()?;
        self.entries.get_mut(idx)
    }

    pub fn fire_mag(&self, legacy_order_key: u32) -> f64 {
        self.entry(legacy_order_key).and_then(StateEntry::fire_mag_value).unwrap_or(0.0)
    }

    pub fn ice_frozen_step(&self, legacy_order_key: u32) -> Option<i32> {
        self.entry(legacy_order_key).and_then(StateEntry::ice_value)
    }

    pub fn add_ice_frozen_step(&mut self, legacy_order_key: u32, frozen_step: i32) {
        if let Some(entry) = self.entry_mut(legacy_order_key) {
            let current = match &entry.payload {
                StatePayload::Ice { frozen_step } => *frozen_step,
                _ => 0,
            };
            entry.payload = StatePayload::Ice {
                frozen_step: current + frozen_step,
            };
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::ice(legacy_order_key, frozen_step));
    }

    pub fn apply_ice_pre_step(&mut self, step: i32, move_points: i32) -> (i32, bool) {
        let Some((legacy_order_key, frozen_step)) = self.entries.iter_mut().find_map(|entry| {
            if let StatePayload::Ice { frozen_step } = &mut entry.payload {
                Some((entry.legacy_order_key, frozen_step))
            } else {
                None
            }
        }) else {
            return (step, false);
        };

        if *frozen_step > 0 {
            if step != 0 {
                *frozen_step -= step;
                self.generation = self.generation.wrapping_add(1);
            }
            return (0, false);
        }
        if step + move_points >= MOVE_POINT_THRESHOLD {
            assert!(
                self.clear_legacy_key(legacy_order_key),
                "runtime_v2 ice state disappeared during pre-step"
            );
            return (0, true);
        }
        (step, false)
    }

    pub fn add_fire_mag_half_step(&mut self, legacy_order_key: u32) {
        if let Some(idx) = self.index.get(&legacy_order_key).copied()
            && let Some(entry) = self.entries.get_mut(idx)
        {
            match &mut entry.payload {
                StatePayload::FireMagHalfSteps(half_steps) => {
                    *half_steps += 1;
                }
                StatePayload::None
                | StatePayload::Ice { .. }
                | StatePayload::ShieldValue(_)
                | StatePayload::Curse { .. }
                | StatePayload::Iron { .. }
                | StatePayload::CovidBoss { .. }
                | StatePayload::CovidInfection { .. }
                | StatePayload::SaitamaBoss { .. }
                | StatePayload::LazyBoss { .. }
                | StatePayload::LazyInfection { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
                StatePayload::Poison { .. }
                | StatePayload::Haste { .. }
                | StatePayload::Berserk { .. }
                | StatePayload::Charm { .. }
                | StatePayload::Slow { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
            }
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::fire_mag(legacy_order_key, 1));
    }

    pub fn set_shield_value(&mut self, legacy_order_key: u32, shield: i32) -> bool {
        self.set_payload(legacy_order_key, StatePayload::ShieldValue(shield.max(0)))
    }

    pub fn set_payload(&mut self, legacy_order_key: u32, payload: StatePayload) -> bool {
        let Some(entry) = self.entry_mut(legacy_order_key) else {
            return false;
        };
        entry.payload = payload;
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn add_legacy_key(&mut self, legacy_order_key: u32) -> bool { self.add_entry(StateEntry::legacy(legacy_order_key)) }

    pub fn add_entry(&mut self, entry: StateEntry) -> bool {
        if self.index.contains_key(&entry.legacy_order_key) {
            return false;
        }

        self.index.insert(entry.legacy_order_key, self.entries.len());
        self.hook_mask |= entry.hook_mask;
        self.entries.push(entry);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn clear_legacy_key(&mut self, legacy_order_key: u32) -> bool {
        let Some(idx) = self.index.get(&legacy_order_key).copied() else {
            return false;
        };

        self.entries.remove(idx);
        self.rebuild_index();
        self.rebuild_hook_mask();
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn clear_positive_states_with_ordered_messages(&mut self, owner_alive: bool) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        let mut to_remove = Vec::new();

        for entry in &self.entries {
            if entry.is_positive_state() {
                if let Some(message) = entry.positive_clear_message(owner_alive) {
                    messages.push((message.0, entry.registration_order, entry.legacy_order_key, message.1));
                }
                to_remove.push(entry.legacy_order_key);
            }
        }

        messages.sort_unstable_by(|(priority_a, order_a, key_a, _), (priority_b, order_b, key_b, _)| {
            priority_a
                .cmp(priority_b)
                .then_with(|| order_a.cmp(order_b))
                .then_with(|| key_a.cmp(key_b))
        });
        for legacy_order_key in to_remove {
            self.clear_legacy_key(legacy_order_key);
        }
        messages.into_iter().map(|(priority, _, _, message)| (priority, message)).collect()
    }

    pub fn entries_in_hook_order(&self) -> Vec<&StateEntry> {
        let mut entries: Vec<&StateEntry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| (entry.priority, entry.registration_order));
        entries
    }

    pub fn entries_in_hook_order_for(&self, hook: ProcMask) -> Vec<&StateEntry> {
        let mut entries: Vec<&StateEntry> = self.entries.iter().filter(|entry| entry.hook_mask.intersects(hook)).collect();
        entries.sort_by_key(|entry| (entry.priority_for_hook(hook), entry.registration_order));
        entries
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.index.insert(entry.legacy_order_key, idx);
        }
    }

    fn rebuild_hook_mask(&mut self) {
        self.hook_mask = self.entries.iter().fold(ProcMask::default(), |mask, entry| mask | entry.hook_mask);
    }
}
