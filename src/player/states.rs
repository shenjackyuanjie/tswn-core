use crate::player::PlayerStatus;

#[derive(Debug, Clone, Copy)]
pub struct State {
    state_type: StateType,
}

impl State {
    pub fn new(state_type: StateType) -> Self { Self { state_type } }

    pub fn update_state(&self, status: &mut PlayerStatus) {
        match self.state_type {
            StateType::Charm { original_group } => {}
            StateType::Curse => {
                status.atk_sum *= 4;
            }
            StateType::Haste { faster } => status.speed *= faster,
            StateType::Ice => {
                status.set_frozen(true);
            }
            StateType::Slow => {
                status.speed /= 2;
            }
            StateType::Lazy => {
                status.speed /= 2;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum StateType {
    /// 魅惑
    Charm { original_group: u32 },
    /// 诅咒
    Curse,
    /// 疾走
    Haste { faster: i32 },
    /// 冻结
    Ice,
    /// 迟缓
    Slow,

    /// TODO: BOSS
    Lazy,
}
