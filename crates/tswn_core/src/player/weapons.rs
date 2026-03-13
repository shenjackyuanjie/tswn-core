//! # 武器系统 (weapons)
//!
//! 本模块实现武器系统，包括武器计算和属性修改。
//!
//! ## 功能说明
//!
//! - **武器计算** — 根据武器名称计算八围加成和技能 boost
//! - **属性修改** — 死亡笔记等特殊武器效果
//! - **促销武器** — 特殊促销武器的处理
//!
//! ## 武器计算流程
//!
//! 1. **初始化 RC4** — 使用武器名称作为种子
//! 2. **计算八围加成** — 根据 RC4 序列计算各属性加成
//! 3. **选择技能** — 根据 RC4 选择要 boost 的技能 ID
//! 4. **计算 boost 量** — 在 pre_upgrade 中计算技能 boost 量
//!
//! ## 武器类型
//!
//! | 类型        | 说明                          |
//! |-------------|-------------------------------|
//! | `Normal`    | 普通武器                      |
//! | `DeathNote` | 死亡笔记（直接击杀）          |
//! | `Promo`     | 促销武器                      |
//! | `RModifier`  | 属性修改器                    |
//!
//! ## 常量说明
//!
//! 对应 JS 中的各种常量（如 `$.av()`、`$.ap()` 等）：
//! - `AV` — 属性数量 (8 围)
//! - `AP` — 循环上界 / HP index
//! - `BG` — 技能总数
//! - `AI` — seed slice 上界
//! - `A4` — attr randomizer 特殊值
//! - `IH` — p\[r_idx] 赋值
//! - `PN` — kp 比较阈值
//! - `B1` — kp 减数
//! - `D1` — bn 循环上界
//! - `MY` — skill_factor 基数
//! - `Q8` — cB delta 常量
//!
//! ## 武器状态
//!
//! [`WeaponState`] 存储武器计算状态：
//! - `seed` — RC4 状态 (JS: this.d)
//! - `attr_bonus` — 八围加成 (JS: this.r)
//! - `skill_index` — 要 boost 的技能 ID (JS: this.e)
//! - `skill_factor` — 技能 boost 量 (JS: this.f)
//! - `weapon_type` — 武器类型
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::weapons::calc_weapon;
//!
//! let weapon_state = calc_weapon("死亡笔记");
//! println!("八围加成: {:?}", weapon_state.attr_bonus);
//! println!("技能 ID: {}", weapon_state.skill_index);
//! println!("boost 量: {}", weapon_state.skill_factor);
//! ```

use crate::player::Player;
use crate::rc4::RC4;

// ── 常量 (对应 JS $.xxx()) ──────────────────────────────
const AV: usize = 8; // $.av() - 属性数量(8围)
const AP: usize = 7; // $.ap() - 循环上界 / HP index
const BG: usize = 40; // $.bg() - 技能总数
const AI: usize = 48; // $.aI() - seed slice 上界
const A4: usize = 6; // $.a4() - attr randomizer 特殊值
const IH: i32 = 18; // $.iH() - p[r_idx] 赋值
const PN: i32 = 53; // $.pN() - kp 比较阈值
const B1: i32 = 50; // $.b1() - kp 减数
const D1: usize = 31; // $.d1() - bn 循环上界
const MY: i32 = 480; // $.mY() - skill_factor 基数
const Q8: i32 = 999; // $.q8() - cB delta 常量

/// 武器计算状态 (JS: T.Weapon instance fields d/r/e/f)
#[derive(Debug, Clone)]
pub struct WeaponState {
    /// RC4 state & 63, 256 bytes (JS: this.d)
    pub seed: Vec<u8>,
    /// 八围加成 (JS: this.r)
    pub attr_bonus: [i32; AV],
    /// 要 boost 的技能 ID (JS: this.e, 0-39)
    pub skill_index: usize,
    /// 技能 boost 量, 在 pre_upgrade 中计算 (JS: this.f)
    pub skill_factor: i32,
    /// 武器类型
    pub weapon_type: WeaponType,
}

impl WeaponState {
    // ── JS T.Weapon.prototype.b3(a) ─────────────────────
    /// 基础武器初始化: 从武器 RC4 计算 seed / attr_bonus / skill_index
    fn init_base(rc4: &mut RC4) -> (Vec<u8>, [i32; AV], usize) {
        // d = rc4.state.map(|x| x & 63)
        let seed: Vec<u8> = rc4.main_val.iter().map(|&x| x & 63).collect();

        // e = rc4.nextInt(40) → skill id
        let skill_index = rc4.next_i32(BG as i32) as usize;

        // r_idx = rc4.nextInt(8)
        let r_idx = rc4.next_i32(AV as i32) as usize;

        // p[8] 权重数组
        let mut p = [0i32; AV];
        if r_idx == A4 {
            for k in 0..AV {
                p[k] = seed[BG + k] as i32;
            }
        } else {
            for k in 0..AV {
                let v = seed[BG + k] as i32;
                p[k] = if v > PN { v - B1 } else { 0 };
            }
            p[r_idx] = IH;
        }

        // 统计正值数量 n 和正值总和 * 3
        let mut n = 0i32;
        let mut m = 0i32;
        for &v in &p {
            if v > 0 {
                n += 1;
                m += v;
            }
        }
        m *= 3;

        // sort seed[0..8], 计算可分配总值 i_total
        let mut j = [0u8; AV];
        j.copy_from_slice(&seed[0..AV]);
        j.sort_unstable();
        let i_total = j[1] as i32 + j[4] as i32 + n;

        // 按权重分配到 8 围
        let mut attr_bonus = [0i32; AV];
        if m > 0 {
            let mut h = i_total;
            for k in 0..AP {
                let g = (i_total * p[k]) / m;
                h -= g * 3;
                attr_bonus[k] = g;
            }
            if p[AP] > 0 {
                attr_bonus[AP] = h;
            }
        }

        (seed, attr_bonus, skill_index)
    }

    /// 为 Generic 武器创建状态 (JS: new T.Weapon + b3)
    pub fn new_generic(weapon_name: &str) -> Self {
        let weapon_bytes: Vec<u8> = std::iter::once(0u8).chain(weapon_name.as_bytes().iter().copied()).collect();
        let mut rc4 = RC4::new(&weapon_bytes, 2);
        let (seed, attr_bonus, skill_index) = Self::init_base(&mut rc4);
        Self {
            seed,
            attr_bonus,
            skill_index,
            skill_factor: 0,
            weapon_type: WeaponType::Generic,
        }
    }

    /// 为 S11 武器创建状态 (JS: WeaponS11.b3 → cN + override r)
    pub fn new_s11(weapon_name: &str) -> Self {
        let weapon_bytes: Vec<u8> = std::iter::once(0u8).chain(weapon_name.as_bytes().iter().copied()).collect();
        let mut rc4 = RC4::new(&weapon_bytes, 2);
        let (seed, _attr_bonus, _skill_index) = Self::init_base(&mut rc4);
        // S11 覆盖 r 和 e (e 在 b6 中使用不同逻辑)
        let attr_bonus = [11, 0, 11, 0, 0, 0, 0, 0];
        Self {
            seed,
            attr_bonus,
            skill_index: 0, // S11 的 b6 不用 this.e
            skill_factor: 0,
            weapon_type: WeaponType::S11,
        }
    }

    /// 为 DeathNote 武器创建状态
    pub fn new_deathnote(weapon_name: &str) -> Self {
        let weapon_bytes: Vec<u8> = std::iter::once(0u8).chain(weapon_name.as_bytes().iter().copied()).collect();
        let mut rc4 = RC4::new(&weapon_bytes, 2);
        let (seed, attr_bonus, skill_index) = Self::init_base(&mut rc4);
        Self {
            seed,
            attr_bonus,
            skill_index,
            skill_factor: 0,
            weapon_type: WeaponType::DeathNote,
        }
    }

    /// 为 BossEx 武器创建状态 (JS BossWeapon: cN + override cB/bn)
    pub fn new_boss_ex(weapon_name: &str) -> Self {
        let weapon_bytes: Vec<u8> = std::iter::once(0u8).chain(weapon_name.as_bytes().iter().copied()).collect();
        let mut rc4 = RC4::new(&weapon_bytes, 2);
        let (seed, attr_bonus, skill_index) = Self::init_base(&mut rc4);
        Self {
            seed,
            attr_bonus,
            skill_index,
            skill_factor: 0,
            weapon_type: WeaponType::BossEx,
        }
    }

    // ── JS T.Weapon.prototype.cB (base version) ────────
    /// 返回 |m| + |j| + |r|，并可能修改 name_base
    fn cb_base(raw_name_base: &[u8; 128], name_base: &mut [u8], weapon_seed: &[u8], d: usize) -> i32 {
        let m = weapon_seed[d] as i32 - raw_name_base[d] as i32;
        let j = weapon_seed[d + 1] as i32 - raw_name_base[d + 1] as i32;
        let r = weapon_seed[d + 2] as i32 - raw_name_base[d + 2] as i32;

        if m > 0 && j > 0 && r > 0 {
            let q = d + ((m + j + r + Q8) / 3) as usize;
            // 如果 q 超出 128，JS 中 b[q] = undefined → 跳过
            if q < name_base.len() && q < weapon_seed.len() {
                let p = weapon_seed[q] as i32;
                let o = name_base[q] as i32;
                let n = (p - o) / 2 + 1;
                if n > 0 {
                    name_base[q] = (o + n) as u8;
                }
            }
        }

        m.abs() + j.abs() + r.abs()
    }

    // ── JS T.Weapon.prototype.bn (preUpgrade) ──────────
    /// 基础 preUpgrade: 遍历 name_base 三元组, 累积 delta, 计算 skill_factor
    pub fn pre_upgrade_base(&mut self, raw_name_base: &[u8; 128], name_base: &mut [u8]) {
        let mut o = 0i32;
        let mut s = 10;
        while s < D1 {
            o += Self::cb_base(raw_name_base, name_base, &self.seed, s);
            s += 3;
        }
        let f = (MY - o) / A4 as i32;
        self.skill_factor = f.max(0);
    }

    // ── JS T.Weapon.prototype.cs (postUpgrade) ─────────
    /// 基础 postUpgrade: attr_bonus 加到八围 + b6
    pub fn post_upgrade_base(&self, player: &mut Player) {
        for s in 0..AV {
            player.attr[s] = (player.attr[s] as i32 + self.attr_bonus[s]) as u32;
        }
        self.b6(player);
    }

    // ── JS T.Weapon.prototype.b6 ───────────────────────
    /// 基础 b6: boost skill[e] by f
    fn b6(&self, player: &mut Player) {
        if self.skill_factor == 0 && self.weapon_type == WeaponType::Generic {
            // 即使 factor=0, 如果 level==0 仍要标记 boosted
            let skill = player.skills.skill_by_id_mut(self.skill_index);
            if skill.level() == 0 {
                skill.boosted = true;
            }
            return;
        }
        match self.weapon_type {
            WeaponType::Generic | WeaponType::DeathNote | WeaponType::BossEx => {
                let skill = player.skills.skill_by_id_mut(self.skill_index);
                let old = skill.level();
                if old == 0 {
                    skill.boosted = true;
                }
                skill.set_level(old + self.skill_factor as u32);
            }
            // S11 的 b6 添加 SklS11 技能（未实现，暂不处理）
            // RinickModifier 的 b6 逻辑更复杂（未实现）
            _ => {}
        }
    }
}

// ══════════════════════════════════════════════════════════
// Weapon (高层接口：在 build 中调用)
// ══════════════════════════════════════════════════════════

pub struct Weapon;

impl Weapon {
    /// 创建武器状态，在 Player 构造时调用 (JS: new T.Weapon + b3)
    pub fn create_state(weapon_name: &str) -> Option<WeaponState> {
        let wt = WeaponType::from_name(weapon_name);
        match wt {
            WeaponType::None => None,
            WeaponType::Generic => Some(WeaponState::new_generic(weapon_name)),
            WeaponType::S11 => Some(WeaponState::new_s11(weapon_name)),
            WeaponType::DeathNote => Some(WeaponState::new_deathnote(weapon_name)),
            WeaponType::BossEx => Some(WeaponState::new_boss_ex(weapon_name)),
            WeaponType::RinickModifier => {
                // RinickModifier 的 cs 需要在 post_upgrade 中特殊处理
                // 先用 base init 创建，后面 post_upgrade 会覆盖 attr_bonus
                let weapon_bytes: Vec<u8> = std::iter::once(0u8).chain(weapon_name.as_bytes().iter().copied()).collect();
                let mut rc4 = RC4::new(&weapon_bytes, 2);
                let (seed, attr_bonus, skill_index) = WeaponState::init_base(&mut rc4);
                Some(WeaponState {
                    seed,
                    attr_bonus,
                    skill_index,
                    skill_factor: 0,
                    weapon_type: WeaponType::RinickModifier,
                })
            }
        }
    }

    /// preUpgrade: 在 initRawAttr 之前调用
    pub fn pre_upgrade(state: &mut WeaponState, player: &mut Player) {
        match state.weapon_type {
            WeaponType::Generic | WeaponType::S11 | WeaponType::DeathNote => {
                // 继承 T.Weapon.prototype.bn
                state.pre_upgrade_base(&player.raw_name_base, &mut player.name_base[..]);
            }
            WeaponType::BossEx => {
                // BossWeapon 有不同的 cB 和 bn 逻辑
                // TODO: BossWeapon preUpgrade
                boss_weapon_pre_upgrade(state, player);
            }
            WeaponType::RinickModifier => {
                // RinickModifier 继承 base bn
                state.pre_upgrade_base(&player.raw_name_base, &mut player.name_base[..]);
            }
            WeaponType::None => {}
        }
    }

    /// postUpgrade: 在 initRawAttr + initSkills 之后调用
    pub fn post_upgrade(state: &WeaponState, player: &mut Player) {
        match state.weapon_type {
            WeaponType::Generic | WeaponType::DeathNote | WeaponType::BossEx => {
                state.post_upgrade_base(player);
            }
            WeaponType::S11 => {
                // S11 继承 base cs (attr_bonus=[11,0,11,...] + b6 = 添加 SklS11)
                state.post_upgrade_base(player);
                // TODO: S11 的 b6 应该添加 SklS11 技能而不是 boost 普通技能
            }
            WeaponType::RinickModifier => {
                // RinickModifier.cs 先计算 attr_bonus = 63 - current_attr,
                // HP 部分特殊处理, 然后调用 base cs
                rinick_modifier_post_upgrade(state, player);
            }
            WeaponType::None => {}
        }
    }
}

/// BossWeapon preUpgrade (JS: BossWeapon.prototype.bn → cB + dW)
fn boss_weapon_pre_upgrade(state: &mut WeaponState, player: &mut Player) {
    // BossWeapon.bn 调用 this.cB(r.E, r.t, this.d, $.ap()) 一次
    // 然后调用 this.dW() (= base bn)
    // BossWeapon.cB 不同于 T.Weapon.cB
    let d = AP; // $.ap() = 7
    boss_weapon_cb(&player.raw_name_base, &mut player.name_base[..], &state.seed, d);
    // dW = base bn
    state.pre_upgrade_base(&player.raw_name_base, &mut player.name_base[..]);
}

/// BossWeapon.cB (不同于 T.Weapon.cB)
fn boss_weapon_cb(_raw_name_base: &[u8; 128], name_base: &mut [u8], weapon_seed: &[u8], d: usize) {
    // JS BossWeapon.prototype.cB
    for p in 0..3usize {
        let o = d + p;
        if o >= name_base.len() || o >= weapon_seed.len() {
            break;
        }
        let n = weapon_seed[o] as i32;
        let m = name_base[o] as i32;
        let l = n - m;
        if l > 0 {
            name_base[o] = (m + l) as u8;
        } else if m < 63 {
            // $.at() = 63... 可能不是 63, 先用 name_base 值域上界
            // 实际 $.at() 需要确认
            name_base[o] = (m + 63) as u8;
        }
    }
}

/// RinickModifier postUpgrade
fn rinick_modifier_post_upgrade(state: &WeaponState, player: &mut Player) {
    // RinickModifier.cs: r = q.map(x => 63 - x) 对前7围
    // r[7]: if q[7] < 324 then 324 - q[7] else 0
    // 然后 dV = base cs
    let mut modified_state = state.clone();
    for i in 0..AP {
        modified_state.attr_bonus[i] = 63i32 - player.attr[i] as i32;
        // 确保不为负 (如果attr > 63)
        if modified_state.attr_bonus[i] < 0 {
            modified_state.attr_bonus[i] = 0;
        }
    }
    // HP (index 7): 确保至少 324
    let hp_val = player.attr[AP] as i32;
    modified_state.attr_bonus[AP] = if hp_val < 324 { 324 - hp_val } else { 0 };

    modified_state.post_upgrade_base(player);

    // RinickModifier 的 b6 逻辑更复杂 (设置多个技能级别)
    // TODO: 实现 RinickModifier 特殊的 b6 逻辑
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WeaponType {
    None,
    Generic,
    DeathNote,
    S11,
    RinickModifier,
    BossEx,
}

impl WeaponType {
    pub fn from_name(name: &str) -> Self {
        if name.is_empty() {
            return Self::None;
        }
        if name.contains("剁手刀") {
            return Self::S11;
        }
        if name.contains("死亡笔记") {
            return Self::DeathNote;
        }
        if name.contains("属性修改器") {
            return Self::RinickModifier;
        }
        if name.ends_with("ex") || name.ends_with("EX") {
            return Self::BossEx;
        }
        Self::Generic
    }
}
