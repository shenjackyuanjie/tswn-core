use crate::player::PlrId;
use std::sync::atomic::{AtomicU64, Ordering};

static RUN_UPDATES_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    /// 赢！
    Win,
    /// 没动作。
    None,
    /// 下一行（用于换行分隔）。
    NextLine,
}

#[derive(Debug, Clone)]
pub struct RunUpdate {
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub message: String,
    pub caster: PlrId,
    pub target: PlrId,
    pub targets: Vec<PlrId>,
    pub update_type: UpdateType,
}

impl RunUpdate {
    pub fn new_dummy() -> RunUpdate {
        RunUpdate {
            score: 0,
            delay0: 0,
            delay1: 0,
            message: "\n".to_string(),
            caster: 0,
            target: 0,
            targets: vec![],
            update_type: UpdateType::None,
        }
    }

    pub fn msg(&self) -> String {
        let mut msg = self.message.clone();
        // param: Object ?
        // [0] -> caster
        // [1] -> target
        // [2] -> targets
        msg = msg.replace("[0]", &self.caster.to_string());
        msg = msg.replace("[1]", &self.target.to_string());
        msg = msg.replace(
            "[2]",
            &self.targets.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(","),
        );
        msg
    }

    pub fn new_newline() -> RunUpdate {
        RunUpdate {
            score: 0,
            delay0: 0,
            delay1: 0,
            message: "\n".to_string(),
            caster: 0,
            target: 0,
            targets: vec![],
            update_type: UpdateType::NextLine,
        }
    }

    pub fn new(msg: impl ToString, caster: PlrId, target: PlrId, score: u32) -> Self {
        RunUpdate {
            score,
            delay0: 0,
            delay1: 0,
            message: msg.to_string(),
            caster,
            target,
            targets: Vec::new(),
            update_type: UpdateType::None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunUpdates {
    pub id: u64,
    pub updates: Vec<RunUpdate>,
    pub on_update_end: Vec<PlrId>,
}

impl RunUpdates {
    pub fn new() -> RunUpdates {
        RunUpdates {
            id: RUN_UPDATES_ID.fetch_add(1, Ordering::Relaxed),
            updates: vec![],
            on_update_end: vec![],
        }
    }

    pub fn add(&mut self, update: RunUpdate) { self.updates.push(update); }

    pub fn add_all(&mut self, updates: &mut [RunUpdate]) { self.updates.extend_from_slice(updates); }
}
