use super::*;
#[cfg(test)]
use crate::engine::storage::Storage;
#[cfg(test)]
use crate::player::PlayerType;
use crate::player::skill::act::minion::MinionBlueprintOwner;
use crate::player::utils::trim_js_line_end;
use crate::player::{Player, PlrId};
use crate::rc4::Rc4KeySchedulePrefix;

mod init;
mod roster;
mod seed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeBattleInitError {
    Player {
        team_index: usize,
        player_index: usize,
        raw: String,
        message: String,
    },
    EntityCountMismatch {
        prepared: usize,
        runtime: usize,
    },
}

impl std::fmt::Display for RuntimeBattleInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Player {
                team_index,
                player_index,
                raw,
                message,
            } => write!(
                f,
                "runtime battle init failed at team {team_index}, player {player_index} ({raw:?}): {message}"
            ),
            Self::EntityCountMismatch { prepared, runtime } => {
                write!(
                    f,
                    "runtime battle init entity count mismatch: prepared={prepared}, runtime={runtime}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeBattleInitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedBossState {
    None,
    Covid,
    Lazy,
    Saitama,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedPlayerInit {
    template: PlayerTemplate,
    hp: i32,
    alive: bool,
    boss_state: PreparedBossState,
    // 三类蓝图在 score 热路径通常都延迟构造。使用最终槽位本来就需要的 Box，
    // 避免三个空 Option 仍把每个准备对象撑大一整份 PlayerTemplate。
    shadow_blueprint: Option<Box<PlayerTemplate>>,
    summon_blueprint: Option<Box<PlayerTemplate>>,
    zombie_blueprint: Option<Box<PlayerTemplate>>,
    lazy_blueprint_rq_bits: Option<u64>,
}

/// score 数字 profile 的紧凑构造态；只保留生成 Runtime 模板真正需要的数据。
struct ScoreProfileBuild {
    id: PlrId,
    name: String,
    clan_name: String,
    name_base: [u8; 128],
    raw_name_base: [u8; 128],
    skill_order: [u32; 40],
    test_ex: bool,
}

/// 连续 score 轮次之间回收的动态 profile 身份字符串。
#[derive(Debug, Default)]
pub(crate) struct ScoreIdentityBuffer {
    pub(crate) name: String,
    pub(crate) id_key_name: String,
    pub(crate) clan_name: String,
    pub(crate) display_name: String,
}

/// score 每轮构造动态 profile 时复用的临时向量。
#[derive(Default)]
pub(crate) struct ScoreRoundScratch {
    dynamic_inputs: Vec<(usize, usize, usize)>,
    dynamic_groups: Vec<Vec<usize>>,
    name_keys: Vec<[u8; crate::player::NAME_MAX_LEN + 1]>,
    name_lengths: Vec<usize>,
    profile_rngs: Vec<RC4>,
    dynamic_profiles: Vec<ScoreProfileBuild>,
    team_by_player: Vec<usize>,
    factor_eval_rq_bits: u64,
    factor_name_len: usize,
    factor_team: String,
    child_clone_name_factor: f64,
    child_clone_name_factor_ready: bool,
}

/// score roster 构造与应用完成后交还下一轮的输出向量。
#[derive(Debug, Default, Clone)]
pub(crate) struct ScoreRosterBuffers {
    players: Vec<Option<PreparedPlayerInit>>,
    player_alive: Vec<bool>,
    input_groups: Vec<Vec<PlrId>>,
    base_names_sorted: Vec<String>,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
}

impl ScoreProfileBuild {
    fn from_rng(id: PlrId, name: String, clan_name: String, mut rand: RC4) -> Self {
        let mut name_base = score_name_base(&rand.main_val);
        let mut raw_name_base = name_base;
        let test_ex = clan_name == "!";
        if test_ex {
            for value in &mut name_base[6..50] {
                if *value < 41 {
                    *value = (*value & 15) + 41;
                }
            }
            for value in &mut name_base[50..] {
                if *value < 16 {
                    *value += 32;
                }
            }
            raw_name_base = name_base;
        } else {
            debug_assert_eq!(clan_name, "\u{0002}");
            for value in &mut name_base[..50] {
                if *value < 12 {
                    *value = 63 - *value;
                }
            }
        }

        let mut skill_order = std::array::from_fn(|index| index as u32);
        rand.sort_list(&mut skill_order);
        Self {
            id,
            name,
            clan_name,
            name_base,
            raw_name_base,
            skill_order,
            test_ex,
        }
    }

    fn upgrade_from(&mut self, other: &Self) {
        if self.test_ex {
            return;
        }
        for index in 7..128 {
            if other.raw_name_base[index - 1] == self.raw_name_base[index] && other.raw_name_base[index] > self.name_base[index] {
                self.name_base[index] = other.raw_name_base[index];
            }
        }
    }

    fn into_prepared(
        self,
        team: usize,
        eval_rq: f64,
        cached_child_clone_name_factor: Option<f64>,
        skill_import: &PlainLegacySkillImportMap,
        mut skills: SkillLoadout,
        mut id_key_name: String,
        mut display_name: String,
    ) -> PreparedPlayerInit {
        let mut sorted_head: [u8; 10] = self.name_base[..10].try_into().expect("score profile head length is fixed");
        sorted_head.sort_unstable();
        let mut attrs = [0u32; 8];
        for (attr, offset) in attrs[..7].iter_mut().zip((10..31).step_by(3)) {
            *attr = u32::from(crate::player::median(
                self.name_base[offset],
                self.name_base[offset + 1],
                self.name_base[offset + 2],
            ));
        }
        attrs[7] =
            154 + u32::from(sorted_head[3]) + u32::from(sorted_head[4]) + u32::from(sorted_head[5]) + u32::from(sorted_head[6]);

        let mut levels = [0u32; 35];
        let mut boosted = [false; 35];
        let mut boosts: [Option<crate::player::skill::SkillBoost>; 35] = std::array::from_fn(|_| None);
        let mut slot_skill_keys = [None; 16];
        for (slot, offset) in (64..128).step_by(4).enumerate() {
            let small = *self.name_base[offset..offset + 4].iter().min().unwrap();
            let key = self.skill_order[slot] as usize;
            if small <= 10 || key >= 35 {
                continue;
            }
            levels[key] = u32::from(small - 10);
            boosted[key] = self.raw_name_base[offset..offset + 4].iter().min().copied().unwrap() <= 10;
            slot_skill_keys[slot] = Some(key);
        }
        for &key in self.skill_order.iter().rev() {
            let key = key as usize;
            if key < 25 && levels[key] > 0 && !boosted[key] {
                let base = levels[key];
                levels[key] = base.saturating_mul(2);
                boosted[key] = true;
                boosts[key] = Some(crate::player::skill::SkillBoost::LastBoost(base));
                break;
            }
        }
        for (slot, left, right) in [(14usize, 60usize, 61usize), (15, 62, 63)] {
            let Some(key) = slot_skill_keys[slot] else {
                continue;
            };
            if levels[key] == 0 || boosted[key] {
                continue;
            }
            let base = levels[key];
            let amount = u32::from(self.name_base[left].min(self.name_base[right])).min(base);
            levels[key] = base.saturating_add(amount);
            boosted[key] = true;
            boosts[key] = Some(crate::player::skill::SkillBoost::SlotBoost { base, boost: amount });
        }

        let attack = attrs[0] as i32;
        let defense = attrs[1] as i32;
        let speed = attrs[2] as i32 + 160;
        let agility = attrs[3] as i32;
        let magic = attrs[4] as i32;
        let resistance = attrs[5] as i32;
        let wisdom = attrs[6] as i32;
        let max_hp = attrs[7] as i32;
        let attr_sum = attrs[..7].iter().sum();
        let atk_sum = (attack - defense + attrs[2] as i32 + magic - resistance) * 2 + agility + wisdom;
        let mut status = crate::player::PlayerStatus {
            hp: max_hp,
            max_hp,
            attack,
            defense,
            speed,
            agility,
            magic,
            magic_point: wisdom >> 1,
            resistance,
            wisdom,
            attr_sum,
            atk_sum,
            all_sum: attr_sum * 3 + attrs[7],
            ..crate::player::PlayerStatus::default()
        };
        status.at_boost = 1.0;
        let child_clone_name_factor = cached_child_clone_name_factor.unwrap_or_else(|| {
            let factor_name = crate::player::eval_name::eval_str_common_with_rq(&self.name, true, eval_rq);
            let factor_team = crate::player::eval_name::eval_str_common_with_rq(&self.clan_name, true, eval_rq);
            factor_name.max(factor_team - 6.0)
        });
        let clone_build = CloneBuildData::from_score_profile(attrs, child_clone_name_factor);
        skill_import.reset_score_profile(&mut skills, &levels, &boosted, &boosts, &self.skill_order);
        id_key_name.clear();
        id_key_name.push_str(&self.name);
        id_key_name.push('@');
        id_key_name.push_str(&self.clan_name);
        display_name.clear();
        display_name.push_str(&self.name);
        let template = PlayerTemplate::from_score_profile(
            self.id,
            self.name,
            id_key_name,
            self.clan_name,
            display_name,
            team,
            &status,
            skills,
            clone_build,
        );
        PreparedPlayerInit {
            template,
            hp: max_hp,
            alive: true,
            boss_state: PreparedBossState::None,
            shadow_blueprint: None,
            summon_blueprint: None,
            zombie_blueprint: None,
            lazy_blueprint_rq_bits: Some(eval_rq.to_bits()),
        }
    }
}

#[inline]
fn score_name_base(values: &[u8; 256]) -> [u8; 128] {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: 上面的运行时检测保证当前处理器支持本函数使用的全部 AVX2 指令。
        return unsafe { score_name_base_avx2(values) };
    }
    score_name_base_scalar(values)
}

fn score_name_base_scalar(values: &[u8; 256]) -> [u8; 128] {
    let mut name_base = [0u8; 128];
    let mut output = 0usize;
    for &value in values {
        let mapped = ((u32::from(value) * 181) + 160) & 255;
        if (89..217).contains(&mapped) {
            name_base[output] = (mapped & 63) as u8;
            output += 1;
        }
    }
    assert_eq!(output, 128, "score profile name base must contain 128 entries");
    name_base
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn score_name_base_avx2(values: &[u8; 256]) -> [u8; 128] {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let multiplier = _mm256_set1_epi16(181);
    let offset = _mm256_set1_epi16(160);
    let low_byte = _mm256_set1_epi16(255);
    let lower_bound = _mm256_set1_epi16(88);
    let upper_bound = _mm256_set1_epi16(217);
    let mut name_base = [0u8; 128];
    let mut output = 0usize;
    let mut mapped_bytes = [0u8; 16];

    for chunk_start in (0..256).step_by(16) {
        // SAFETY: chunk_start 依次为 0..240，始终可以读取完整 16 字节。
        let input = unsafe { _mm_loadu_si128(values.as_ptr().add(chunk_start).cast()) };
        let widened = _mm256_cvtepu8_epi16(input);
        let mapped = _mm256_and_si256(_mm256_add_epi16(_mm256_mullo_epi16(widened, multiplier), offset), low_byte);
        let selected = _mm256_and_si256(_mm256_cmpgt_epi16(mapped, lower_bound), _mm256_cmpgt_epi16(upper_bound, mapped));
        let mapped_packed = _mm_packus_epi16(_mm256_castsi256_si128(mapped), _mm256_extracti128_si256(mapped, 1));
        let selected_packed = _mm_packs_epi16(_mm256_castsi256_si128(selected), _mm256_extracti128_si256(selected, 1));
        // SAFETY: mapped_bytes 恰好可写入 16 字节。
        unsafe { _mm_storeu_si128(mapped_bytes.as_mut_ptr().cast(), mapped_packed) };
        let mut mask = _mm_movemask_epi8(selected_packed) as u32;
        while mask != 0 {
            let index = mask.trailing_zeros() as usize;
            name_base[output] = mapped_bytes[index] & 63;
            output += 1;
            mask &= mask - 1;
        }
    }
    assert_eq!(output, 128, "score profile AVX2 name base must contain 128 entries");
    name_base
}

/// 与 seed 无关的 Runtime 对局初始化模板。
///
/// 名字解析、组队加成、玩家 build 和召唤物蓝图只执行一次；每场对局只需根据
/// seed 重建随机排序、初始移动点和 world 视图，供批量评分/胜率路径复用。
#[derive(Debug, Clone)]
pub struct PreparedBattleRoster {
    /// `None` 表示 score 轮次直接保留 prototype 中的固定 target。
    players: Vec<Option<PreparedPlayerInit>>,
    player_alive: Vec<bool>,
    input_groups: Vec<Vec<PlrId>>,
    base_names_sorted: Vec<String>,
    profile_seed_rc4_prefix: Option<Rc4KeySchedulePrefix>,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
    recycle_score_buffers: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedBattleInit {
    players: Vec<Option<PreparedPlayerInit>>,
    input_groups: Vec<Vec<EntityIdx>>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    rng: RC4,
    teams: Vec<usize>,
    speed_points: Vec<i32>,
    sort_ints: Vec<i32>,
    battle_groups: Vec<Vec<PlrId>>,
    rc4_key: String,
    score_buffers: Option<ScoreRosterBuffers>,
}

/// 只包含 seed 会改变的对局初始状态。
///
/// 固定 roster 的批量胜率路径可直接把这份轻量状态应用到已经复位的 prototype，
/// 无需为每局深拷贝玩家模板、技能表和召唤物蓝图。
#[derive(Debug, Clone)]
pub struct PreparedBattleSeed {
    input_groups: Vec<Vec<EntityIdx>>,
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    teams: Vec<usize>,
    speed_points: Vec<i32>,
    rng: RC4,
    sort_ints: Vec<i32>,
    battle_groups: Vec<Vec<PlrId>>,
    rc4_key: String,
}
