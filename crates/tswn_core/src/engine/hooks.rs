//! # 钩子管线 (hooks)
//!
//! 本模块提供 [`HookPipeline`]，实现了四个可注册的回调注入点：
//!
//! | 钩子点           | 触发时机                                          |
//! |------------------|---------------------------------------------------|
//! | `pre_action`     | 玩家本 tick 行动**开始前**（已选出当前行动角色）  |
//! | `post_action`    | 玩家本 tick 行动**结束后**（含 run_update_end）   |
//! | `pre_damage`     | 玩家执行攻击/技能**前**（即 Player::step 前）      |
//! | `post_damage`    | 玩家执行攻击/技能**后**（即 Player::step 后）      |
//!
//! 每个钩子点可注册多个回调函数（[`ActorHook`]），按注册顺序依次执行。
//!
//! ## 注意事项
//!
//! - 钩子函数签名固定为 `fn(PlrId, &Arc<Storage>, &mut RC4, &mut RunUpdates)`
//! - 钩子不持有 `WorldState` 引用，如需读取存活信息请通过 `Storage` 查询
//! - 目前 `pre_action`/`post_action` 由 [`EngineCore`](crate::engine::engine_core::EngineCore) 驱动，
//!   `pre_damage`/`post_damage` 由 [`tick::resolve_combat`](crate::engine::tick::resolve_combat) 驱动
//! - 为了兼顾性能与扩展性，内置路径可继续使用函数指针；
//!   需要 DLL/hook 扩展时可注册 trait object 形式的动态钩子。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::PlrId;
use crate::rc4::RC4;

/// 钩子回调函数类型。参数依次为：行动角色 ID、存储引用、随机数生成器、更新帧列表。
pub type ActorHook = fn(PlrId, &Arc<Storage>, &mut RC4, &mut RunUpdates);

pub trait ActorHookDyn: Send + Sync {
    fn run(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates);
}

impl<F> ActorHookDyn for F
where
    F: Fn(PlrId, &Arc<Storage>, &mut RC4, &mut RunUpdates) + Send + Sync,
{
    fn run(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        (self)(actor, storage, randomer, updates);
    }
}

/// 战斗事件钩子管线，管理四个回调注入点的回调列表。
#[derive(Default)]
pub struct HookPipeline {
    pre_action: Vec<ActorHook>,
    pre_action_dyn: Vec<Box<dyn ActorHookDyn>>,
    post_action: Vec<ActorHook>,
    post_action_dyn: Vec<Box<dyn ActorHookDyn>>,
    pre_damage: Vec<ActorHook>,
    pre_damage_dyn: Vec<Box<dyn ActorHookDyn>>,
    post_damage: Vec<ActorHook>,
    post_damage_dyn: Vec<Box<dyn ActorHookDyn>>,
}

impl HookPipeline {
    pub fn register_pre_action(&mut self, hook: ActorHook) { self.pre_action.push(hook); }

    pub fn register_pre_action_dyn<H: ActorHookDyn + 'static>(&mut self, hook: H) { self.pre_action_dyn.push(Box::new(hook)); }

    pub fn register_post_action(&mut self, hook: ActorHook) { self.post_action.push(hook); }

    pub fn register_post_action_dyn<H: ActorHookDyn + 'static>(&mut self, hook: H) { self.post_action_dyn.push(Box::new(hook)); }

    pub fn register_pre_damage(&mut self, hook: ActorHook) { self.pre_damage.push(hook); }

    pub fn register_pre_damage_dyn<H: ActorHookDyn + 'static>(&mut self, hook: H) { self.pre_damage_dyn.push(Box::new(hook)); }

    pub fn register_post_damage(&mut self, hook: ActorHook) { self.post_damage.push(hook); }

    pub fn register_post_damage_dyn<H: ActorHookDyn + 'static>(&mut self, hook: H) { self.post_damage_dyn.push(Box::new(hook)); }

    #[inline]
    pub fn has_pre_damage_hooks(&self) -> bool { !self.pre_damage.is_empty() || !self.pre_damage_dyn.is_empty() }

    pub fn run_pre_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_action {
            hook(actor, storage, randomer, updates);
        }
        for hook in &self.pre_action_dyn {
            hook.run(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_action {
            hook(actor, storage, randomer, updates);
        }
        for hook in &self.post_action_dyn {
            hook.run(actor, storage, randomer, updates);
        }
    }

    pub fn run_pre_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_damage {
            hook(actor, storage, randomer, updates);
        }
        for hook in &self.pre_damage_dyn {
            hook.run(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_damage {
            hook(actor, storage, randomer, updates);
        }
        for hook in &self.post_damage_dyn {
            hook.run(actor, storage, randomer, updates);
        }
    }
}
