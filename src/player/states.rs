#[derive(Debug, Clone, Copy)]
pub struct State {
    state_type: StateType,
}

#[derive(Debug, Clone, Copy)]
pub enum StateType {
    /// 冻结
    Frozen,
}
