//! 爱丽丝的人偶编队、舞台剧、自爆与八云蓝的式神召唤。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    /// 直接读取仍存活的人偶，死亡或自爆后即可再次召唤，不依赖易失计数器。
    pub fn pcb_dolls(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        self.entities
            .iter()
            .filter_map(|(id, e)| {
                (e.runtime.owner == actor
                    && e.runtime.hp > 0
                    && e.runtime.alive
                    && e.runtime.rosmontic.as_ref().is_some_and(|s| (10..=18).contains(&s.child_kind)))
                .then_some(id)
            })
            .collect()
    }

    pub fn ros_alice(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        if skill == 1 || skill == 2 {
            let dolls = self.pcb_dolls(actor);
            let count = dolls.len();
            self.rosmontic_message(
                actor,
                if skill == 1 {
                    "[0]使用[魔操「回归虚无」]"
                } else {
                    "[0]使用[操符「少女文乐」]"
                },
                updates,
            );
            let atp = self.ros_atp(actor, true) * if skill == 1 { 2.0 } else { 1.0 };
            if skill == 1 {
                // 自爆走正常死亡链，随后按当前人偶数连击；本体与自己的其他人偶不受伤。
                for child in dolls {
                    self.rosmontic_message(child, "[0][自爆]", updates);
                    self.entities.get_mut(child).unwrap().runtime.hp = 0;
                    self.drain_summon_explode_self_death_into(child, updates);
                }
            }
            for target in targets {
                if target == actor || self.entities.get(target).unwrap().runtime.owner == actor {
                    continue;
                }
                let resistance = self.entities.get(target).unwrap().runtime.resistance;
                if skill == 1 {
                    self.entities.get_mut(target).unwrap().runtime.resistance = 0;
                }
                for _ in 0..count {
                    self.ros_hit(actor, target, skill == 1, atp, skill == 1, PlainAttackOnDamage::None, updates);
                }
                if skill == 1 {
                    self.entities.get_mut(target).unwrap().runtime.resistance = resistance;
                }
            }
            return;
        }
        let (role, name, attrs, points, attract) = match skill {
            0 => (18, "实验型.提泰妮娅人偶", [2000, 300, 150, 100, 100, 100, 150, 80], -1024, 2.0),
            3 => (10, "上海人偶", [200, 90, 90, 90, 90, 90, 90, 90], 2048, 1.0),
            4 => (11, "蓬莱人偶", [200, 90, 90, 90, 90, 90, 90, 90], 2048, 1.0),
            5 => (12, "荷兰人偶", [160, 115, 80, 120, 120, 80, 80, 90], 2048, 1.0),
            6 => (13, "法兰西人偶", [160, 80, 80, 80, 80, 80, 80, 80], 2048, 1.0),
            7 => (17, "俄罗斯人偶", [190, 75, 75, 75, 75, 175, 75, 75], 1024, 1.0),
            8 => (15, "伦敦人偶", [140, 10, 90, 130, 120, 70, 70, 100], 2048, 1.0),
            9 => (16, "京都人偶", [300, 5, 180, 75, 75, 180, 180, 180], 2048, 1.0),
            _ => (14, "西藏人偶", [600, 200, 200, 100, 10, 100, 200, 100], -6144, 4.0),
        };
        let text = match skill {
            0 => "[0]使用[试验中「提泰妮娅级」]",
            3 => "[0]使用[诅咒「魔彩光的上海人偶」]",
            4 => "[0]使用[诅咒「上吊的蓬莱人偶」]",
            5 => "[0]使用[红符「红发的荷兰人偶」]",
            6 => "[0]使用[苍符「博爱的法兰西人偶」]",
            7 => "[0]使用[白符「白垩的俄罗斯人偶」]",
            8 => "[0]使用[暗符「雾之伦敦人偶」]",
            9 => "[0]使用[雅符「春之京都人偶」]",
            _ => "[0]使用[回符「轮回的西藏人偶」]",
        };
        self.rosmontic_message(actor, text, updates);
        self.pcb_spawn(actor, role, name, attrs, points, attract, updates);
        if skill == 6 {
            self.pcb_spawn(actor, role, "奥尔良人偶", attrs, points, attract, updates);
        }
    }

    pub fn ros_ran(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            0 => {
                self.ros_state(actor).levels[0] = 0;
                self.rosmontic_message(actor, "[0]使用[式神「前鬼后鬼的守护」]", updates);
                for (role, name) in [(20, "前鬼"), (21, "后鬼")] {
                    self.pcb_spawn(actor, role, name, [240, 80, 80, 80, 80, 80, 80, 80], -1024, 1.0, updates);
                }
            }
            1 => {
                self.ros_state(actor).levels[1] = 0;
                self.rosmontic_message(actor, "[0]使用[式神「橙」]", updates);
                self.pcb_spawn(actor, 22, "橙", [500, 70, 70, 95, 95, 50, 80, 60], 0, 1.0, updates);
            }
            2 => {
                self.ros_state(actor).levels[2] -= 40;
                self.rosmontic_message(actor, "[0]使用[幻神「饭纲权现降临」]", updates);
                for target in targets {
                    let e = self.entities.get(target).unwrap();
                    if e.runtime.is_combat_minion()
                        || self.is_rosmontic_boss(target, "ran")
                        || self.is_rosmontic_boss(target, "chen")
                    {
                        let speed = e.runtime.speed;
                        let e = self.entities.get_mut(target).unwrap();
                        e.runtime.move_state.speed_points += speed;
                        let s = e.runtime.rosmontic.get_or_insert_with(Default::default);
                        s.divine_turns = s.divine_turns.saturating_add(3);
                        e.apply_runtime_stats_from_template(false);
                        self.ros_message(actor, target, "[1]进入[神现]，属性大幅度增加", updates);
                    }
                }
            }
            _ => {
                self.rosmontic_message(actor, "[0]使用[式神「十二神将之宴」]", updates);
                let n = self.ros_state(actor).illusion_count;
                self.ros_state(actor).illusion_count = n.saturating_add(1);
                let factor = 1.0 + 0.12 * f64::from(n);
                let value = (50.0 * factor) as i32;
                self.pcb_spawn(
                    actor,
                    23,
                    "幻神",
                    [(100.0 * factor) as i32, value, value, value, value, value, value, value],
                    0,
                    1.0,
                    updates,
                );
            }
        }
    }
}
