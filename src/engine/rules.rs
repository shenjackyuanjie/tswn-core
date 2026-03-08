#[derive(Default)]
pub struct RuleRegistry {
    pub skill_rules: usize,
    pub weapon_rules: usize,
    pub boss_rules: usize,
}

impl RuleRegistry {
    pub fn register_skill_rule(&mut self) {
        self.skill_rules += 1;
    }

    pub fn register_weapon_rule(&mut self) {
        self.weapon_rules += 1;
    }

    pub fn register_boss_rule(&mut self) {
        self.boss_rules += 1;
    }
}
