use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::PlrId;
use crate::rc4::RC4;

pub type ActorHook = fn(PlrId, &Arc<Storage>, &mut RC4, &mut RunUpdates);

#[derive(Default)]
pub struct HookPipeline {
    pre_action: Vec<ActorHook>,
    post_action: Vec<ActorHook>,
    pre_damage: Vec<ActorHook>,
    post_damage: Vec<ActorHook>,
}

impl HookPipeline {
    pub fn register_pre_action(&mut self, hook: ActorHook) {
        self.pre_action.push(hook);
    }

    pub fn register_post_action(&mut self, hook: ActorHook) {
        self.post_action.push(hook);
    }

    pub fn register_pre_damage(&mut self, hook: ActorHook) {
        self.pre_damage.push(hook);
    }

    pub fn register_post_damage(&mut self, hook: ActorHook) {
        self.post_damage.push(hook);
    }

    pub fn run_pre_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_action {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_action {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_pre_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_damage {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_damage {
            hook(actor, storage, randomer, updates);
        }
    }
}
