//! `base` 任务：输出单个原始名字的 128 字节 roll、属性与 16 个技能槽。
//!
//! 这是社区 base 工具的无头版本：只接受单个原始名字，输出 HP 区间、10 个 HP
//! 原始字节、七围的「值 / 值 (原始三元组)」以及按行动顺序排列的 16 个技能槽。
//! 数值全部来自 `tswn_core::namerena` 的 `raw_name_base`，不另行解析名字。
//! GUI 的 base 页与 `openbox-cli base` 都调用 [`run_base`]，两者的文本逐字节一致。

use std::fmt::Write as _;

use tswn_core::namerena::{
    NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerSpec, PreparedPlayer, PreparedRoster, TEAM_MAX_LEN,
    eval_name::DEFAULT_EVAL_RQ,
};
use tswn_core::rc4::RC4;

const PROP_NAMES: [&str; 7] = ["攻", "防", "速", "敏", "魔", "抗", "智"];

/// 社区 base 工具使用的短技能名；id 32 沿用「亡灵」的叫法。
const SKILL_NAMES: [&str; 40] = [
    "火球",
    "冰冻",
    "雷击",
    "地裂",
    "吸血",
    "投毒",
    "连击",
    "会心",
    "瘟疫",
    "命轮",
    "狂暴",
    "魅惑",
    "加速",
    "减速",
    "诅咒",
    "治愈",
    "苏生",
    "净化",
    "铁壁",
    "蓄力",
    "聚气",
    "潜行",
    "血祭",
    "分身",
    "幻术",
    "防御",
    "守护",
    "反弹",
    "护符",
    "护盾",
    "反击",
    "吞噬",
    "亡灵",
    "垂死",
    "隐匿",
    "空技能",
    "空技能",
    "空技能",
    "空技能",
    "空技能",
];

/// 执行 base，返回完整的多行文本。
///
/// 只接受一个原始名字：空输入、多行输入、`+diy` / `+ol` / 武器后缀以及非普通
/// 名字都会返回错误文本，由调用方决定如何呈现（CLI 打到 stderr，GUI 打到日志）。
pub fn run_base(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("base 需要一个名字。".to_string());
    }
    if raw.lines().filter(|line| !line.trim().is_empty()).count() != 1 {
        return Err("base 一次只支持一个名字（避免刷屏）。".to_string());
    }

    let spec = PlayerSpec::parse(raw).map_err(|err| format!("名字解析失败: {err}"))?;
    if spec.overlay.is_some() || spec.weapon.is_some() {
        return Err("base 只支持原始名字，不支持 +diy / +ol / 武器后缀。".to_string());
    }
    if spec.class != PlayerClass::Normal {
        return Err("base 只支持普通名字。".to_string());
    }

    let input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]]).map_err(|err| format!("构建输入失败: {err}"))?;
    let roster = match PreparedRoster::build(&input, DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    let player = roster.players.into_iter().next().ok_or_else(|| "没有构建出玩家。".to_string())?;
    Ok(render(&player).trim_end().to_string())
}

fn render(player: &PreparedPlayer) -> String {
    let raw = &player.raw_name_base;
    let mut out = String::new();

    let raw_head: [u8; 10] = raw[..10].try_into().expect("name head has fixed size");
    let mut head = raw_head;
    head.sort_unstable();
    let hp_min = 154 + head[3..7].iter().map(|value| u32::from(*value)).sum::<u32>();
    let hp_max = 154 + head[4..8].iter().map(|value| u32::from(*value)).sum::<u32>();
    let _ = writeln!(out, "HP: {hp_min} / {hp_max}");
    let _ = writeln!(out, "({})", join_u8(&raw_head));

    for (index, offset) in (10..31).step_by(3).enumerate() {
        let raw_triplet = [raw[offset], raw[offset + 1], raw[offset + 2]];
        let mut sorted = raw_triplet;
        sorted.sort_unstable();
        let _ = writeln!(
            out,
            "{}: {} / {} ({})",
            PROP_NAMES[index],
            u32::from(sorted[1]) + 36,
            u32::from(sorted[2]) + 36,
            join_u8(&raw_triplet)
        );
    }
    let _ = writeln!(out);

    let action_order = action_order(player);
    let mut slot_ids = [0usize; 16];
    let mut slot_levels = [0u32; 16];
    for (slot, skill_id) in action_order.iter().take(16).enumerate() {
        let offset = 64 + slot * 4;
        let min = raw[offset..offset + 4].iter().copied().min().unwrap_or(0);
        let skill_id = *skill_id as usize;
        slot_ids[slot] = skill_id;
        if min > 10 && skill_id < 35 {
            slot_levels[slot] = u32::from(min - 10);
        }
    }

    // 与 tswn-core / sqrtools 一致的「末尾主动」：从后往前找第一个主动技能翻倍。
    let mut double_slot = None;
    for slot in (0..16).rev() {
        if slot_levels[slot] > 0 && slot_ids[slot] < 25 {
            slot_levels[slot] *= 2;
            double_slot = Some(slot);
            break;
        }
    }
    // 槽位 14/15 的末尾座位加成。
    for (slot, left, right) in [(14usize, 60usize, 61usize), (15, 62, 63)] {
        if slot_levels[slot] == 0 || double_slot == Some(slot) {
            continue;
        }
        let amount = u32::from(raw[left].min(raw[right])).min(slot_levels[slot]);
        slot_levels[slot] += amount;
    }

    for slot in 0..16 {
        let skill_id = slot_ids[slot];
        let _ = write!(out, "#{slot:02} {}", SKILL_NAMES[skill_id]);
        if skill_id >= 35 {
            let _ = writeln!(out);
            continue;
        }

        let offset = 64 + slot * 4;
        let raw_skill = [raw[offset], raw[offset + 1], raw[offset + 2], raw[offset + 3]];
        let mut sorted = raw_skill;
        sorted.sort_unstable();
        let first = i32::from(sorted[0]);
        let second = i32::from(sorted[1]);
        let shown = if second > 10 { second - 10 } else { 0 };

        let (result, extra) = if slot < 14 {
            if double_slot == Some(slot) {
                (shown * 2, "\n    ↑末尾主动".to_string())
            } else {
                (shown, String::new())
            }
        } else if slot_levels[slot] > 0 {
            if double_slot == Some(slot) {
                (shown * 2, "\n    ↑末尾主动".to_string())
            } else {
                let seat_left = i32::from(raw[32 + slot * 2]);
                let seat_right = i32::from(raw[33 + slot * 2]);
                let a = (second - 10) + (second - 10).min(seat_left).min(seat_right);
                let b = (first - 10) + (first - 10).min(seat_left.max(seat_right));
                (a.max(b), format!("\n    ↑末尾座位加成: {:02} {:02}", seat_left, seat_right))
            }
        } else {
            (shown, String::new())
        };

        let _ = writeln!(
            out,
            ": {:02} / {:02} ({}){}",
            slot_levels[slot],
            result,
            join_u8(&raw_skill),
            extra
        );
    }

    out
}

fn action_order(player: &PreparedPlayer) -> [u32; 40] {
    let mut name_key = [0u8; NAME_MAX_LEN + 1];
    name_key[1..1 + player.name.len()].copy_from_slice(player.name.as_bytes());
    let clan = &player.clan_name;
    let mut team_key = [0u8; TEAM_MAX_LEN + 1];
    team_key[1..1 + clan.len()].copy_from_slice(clan.as_bytes());

    let mut rand = RC4::new(&team_key[..1 + clan.len()], 1);
    rand.update(&name_key[..1 + player.name.len()], 2);
    let mut order: [u32; 40] = std::array::from_fn(|index| index as u32);
    rand.sort_list(&mut order);
    order
}

fn join_u8(values: &[u8]) -> String { values.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(",") }

#[cfg(test)]
mod tests {
    use super::run_base;

    #[test]
    fn base_matches_reference_example() {
        let expected = "\
HP: 327 / 349
(53,50,22,31,30,58,48,44,55,11)
攻: 85 / 86 (49,16,50)
防: 88 / 97 (52,32,61)
速: 95 / 98 (62,59,27)
敏: 95 / 97 (61,59,43)
魔: 94 / 98 (62,25,58)
抗: 99 / 99 (63,30,63)
智: 92 / 92 (56,18,56)

#00 幻术: 00 / 00 (5,4,28,54)
#01 地裂: 12 / 22 (36,22,33,32)
#02 净化: 00 / 21 (41,9,35,31)
#03 反击: 00 / 10 (2,39,20,36)
#04 会心: 00 / 35 (60,45,2,45)
#05 蓄力: 00 / 11 (21,49,29,7)
#06 魅惑: 05 / 15 (27,15,25,42)
#07 铁壁: 88 / 90 (57,55,54,60)
    ↑末尾主动
#08 诅咒: 00 / 00 (9,0,11,8)
#09 守护: 03 / 31 (13,46,41,51)
#10 亡灵: 13 / 29 (48,44,23,39)
#11 垂死: 09 / 18 (19,34,28,57)
#12 隐匿: 00 / 00 (47,3,0,20)
#13 瘟疫: 00 / 00 (33,6,4,17)
#14 护盾: 00 / 00 (1,42,12,1)
#15 空技能";

        assert_eq!(run_base("霸绩往官脸瞒烤@Hell").expect("base should run"), expected);
    }

    #[test]
    fn base_rejects_multiple_names() {
        assert!(run_base("mario\nluigi").is_err());
    }
}
