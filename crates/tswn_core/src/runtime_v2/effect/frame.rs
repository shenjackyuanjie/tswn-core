use super::*;

#[derive(Debug, Clone)]
pub struct RuntimeFrame {
    pub updates: RunUpdates,
}

impl RuntimeFrame {
    pub fn damage_update(caster: usize, target: usize, amount: i32) -> RunUpdate {
        RunUpdate::new("[0]攻击[1]", caster, target, amount.max(0) as u32)
    }

    pub fn legacy_damage_update(caster: usize, target: usize, amount: i32) -> RunUpdate {
        if amount <= 0 {
            let mut update = RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", target, target, 10);
            update.param = Some(0);
            return update;
        }

        let message = if amount >= 160 {
            "[1]受到[2]点伤害[s_dmg160]"
        } else if amount >= 120 {
            "[1]受到[2]点伤害[s_dmg120]"
        } else {
            "[1]受到[2]点伤害"
        };
        let mut update = RunUpdate::new(message, caster, target, amount as u32);
        update.delay0 = if amount > 250 { 1500 } else { 1000 + amount * 2 };
        update
    }

    pub fn heal_update(caster: usize, target: usize, amount: i32) -> RunUpdate {
        RunUpdate::new("[1]回复体力[2]点", caster, target, amount.max(0) as u32)
    }

    pub fn spawn_update(caster: usize, spawned: usize) -> RunUpdate {
        RunUpdate::new("出现一个新的[1]", caster, spawned, 0)
    }

    pub fn add_state_update(target: usize) -> RunUpdate { RunUpdate::new("[1]状态改变", target, target, 0) }

    pub fn clear_state_update(target: usize) -> RunUpdate { RunUpdate::new("[1]状态解除", target, target, 0) }

    pub fn revive_update(caster: usize, target: usize, hp: i32) -> RunUpdate {
        RunUpdate::new("[1][复活]了", caster, target, hp.max(0) as u32)
    }

    pub fn remove_update(caster: usize, target: usize) -> RunUpdate { RunUpdate::new("[1]消失了", caster, target, 0) }

    pub fn replay_update(caster: usize, target: usize, message: impl Into<String>, score: u32) -> RunUpdate {
        RunUpdate::new(message.into(), caster, target, score)
    }

    pub fn single_damage(caster: usize, target: usize, amount: i32) -> Self {
        let mut updates = RunUpdates::new();
        updates.add(Self::damage_update(caster, target, amount));
        Self { updates }
    }

    pub fn render_core_replay(&self) -> Vec<CoreReplayEvent> {
        self.updates
            .updates
            .iter()
            .map(|update| CoreReplayEvent {
                message: update.message.to_string(),
                caster: update.caster,
                target: update.target,
                targets: update.targets.iter().copied().collect(),
                param: update.param,
                score: update.score,
            })
            .collect()
    }

    pub fn render_core_show(&self) -> Vec<CoreShowEvent> {
        self.updates
            .updates
            .iter()
            .map(|update| CoreShowEvent {
                text: update.msg(),
                score: update.score,
            })
            .collect()
    }
}
