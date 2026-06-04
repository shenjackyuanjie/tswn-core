//! Rust 内置版等效熟练度计算。
//!
//! 这个模块是按用户提供的修复版 `SP_skill_eq_group_sp1_fixed3.cpp` 逻辑移植进
//! `tswn_lane_ranker` 的 Rust 实现；运行时不编译、不调用 C++。
//! 为了保持数值一致，这里保留了原算法里的 signed-char 字节行为和 C 字符串末尾
//! `\0` 参与一次打乱的细节。

use crate::model::SkillValue;

const N: usize = 256;
const M: usize = 128;
const K: usize = 64;
const SKILL_CNT: usize = 40;
const TYPE_SKILL_THRESHOLD: f64 = 25.0;

const SKILL_NAME_MAP: [&str; 35] = [
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击",
    "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速",
    "诅咒", "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气",
    "背刺", "血祭", "分身", "幻术", "防御", "守护", "反弹",
    "护符", "护盾", "反击", "吞噬", "召灵", "垂死", "隐匿",
];

#[derive(Debug, Clone)]
pub struct GroupSkillSummary {
    /// UI / 导出展示用组合名。双人组按原算法 cfz 从低到高显示。
    pub display_canonical: String,
    /// UI 展示用类型：每个成员取等效熟练度 >= 25 的技能，按等效熟练度降序拼接；成员之间用 `+` 分隔。
    /// 双人组的成员类型顺序与 `display_canonical` 一致。
    pub type_label: String,
    /// 该组合所有成员的每个技能等效熟练度合计；导出技能总表时使用。
    pub skill_totals: Vec<SkillValue>,
}

pub fn compute_group_skill_summary_from_canonical(canonical: &str) -> GroupSkillSummary {
    let members: Vec<String> = canonical
        .split('+')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    compute_group_skill_summary(&members)
}

pub fn compute_group_skill_summary(members: &[String]) -> GroupSkillSummary {
    let raw_members: Vec<String> = members
        .iter()
        .map(|member| trim_line(member))
        .filter(|member| !member.is_empty())
        .collect();

    let mut group: Vec<Name> = raw_members
        .iter()
        .map(|member| build_single_raw(member))
        .collect();

    if group.len() >= 2 {
        group = apply_group_bonus(&group);
    }

    for member in &mut group {
        member.get_43();
    }

    let display_order = member_display_order_by_cfz(&raw_members, &group);
    let display_canonical = if display_order.is_empty() {
        String::new()
    } else {
        display_order
            .iter()
            .map(|&idx| raw_members[idx].as_str())
            .collect::<Vec<_>>()
            .join("+")
    };

    let mut totals = [0.0_f64; 35];
    let mut member_type_labels = Vec::with_capacity(group.len());

    for &member_idx in &display_order {
        let member = &group[member_idx];
        let mut major_skills = Vec::new();

        for i in 0..35 {
            let value = member.effective_skill_value(i);
            totals[i] += value;
            if value >= TYPE_SKILL_THRESHOLD {
                major_skills.push((i, value));
            }
        }

        major_skills.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| a.0.cmp(&b.0))
        });

        if major_skills.is_empty() {
            member_type_labels.push("高八维".to_string());
        } else {
            member_type_labels.push(
                major_skills
                    .iter()
                    .map(|(idx, _)| SKILL_NAME_MAP[*idx])
                    .collect::<Vec<_>>()
                    .join(""),
            );
        }
    }

    let type_label = if member_type_labels.is_empty() {
        "高八维".to_string()
    } else {
        member_type_labels.join("+")
    };

    let skill_totals = (0..35)
        .map(|idx| SkillValue {
            name: SKILL_NAME_MAP[idx].to_string(),
            value: totals[idx],
        })
        .collect();

    GroupSkillSummary {
        display_canonical,
        type_label,
        skill_totals,
    }
}

fn member_display_order_by_cfz(raw_members: &[String], group: &[Name]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..group.len()).collect();

    // 只改双人组展示顺序：cfz 低的在前，cfz 高的在后；并列时按原组合名稳定排序。
    if group.len() == 2 {
        order.sort_by(|&a, &b| {
            group[a]
                .cfz
                .cmp(&group[b].cfz)
                .then_with(|| raw_members[a].cmp(&raw_members[b]))
        });
    }

    order
}

#[derive(Clone)]
struct Name {
    ual: [u8; N],
    val: [u8; N],
    val_base: [u8; N],
    name_base: [u8; M],
    freq: [u8; 16],
    skill: [u8; SKILL_CNT],
    p: u8,
    q: u8,
    q_len: i32,
    last: i32,
    cfz: i32,
    shadowcfz: i32,
    shadowi: f64,
    x: [f64; 50],
}

impl Default for Name {
    fn default() -> Self {
        Self {
            ual: [0; N],
            val: [0; N],
            val_base: [0; N],
            name_base: [0; M],
            freq: [0; 16],
            skill: [0; SKILL_CNT],
            p: 0,
            q: 0,
            q_len: -1,
            last: -1,
            cfz: 0,
            shadowcfz: 0,
            shadowi: 0.0,
            x: [0.0; 50],
        }
    }
}

impl Name {
    #[inline]
    fn m(&mut self) -> u8 {
        self.p = self.p.wrapping_add(1);
        self.q = self.q.wrapping_add(self.val[self.p as usize]);
        self.val.swap(self.p as usize, self.q as usize);
        let idx = self.val[self.p as usize].wrapping_add(self.val[self.q as usize]);
        self.val[idx as usize]
    }

    #[inline]
    fn next_skill_index(&mut self) -> usize {
        let u = self.m() as usize;
        ((u << 8) | self.m() as usize) % SKILL_CNT
    }

    fn load_team(&mut self, team: &str) {
        let bytes = team.as_bytes();
        for i in 0..N {
            self.val_base[i] = i as u8;
        }

        let mut s = 0u8;
        let mut j = bytes.len();
        for i in 0..N {
            s = s
                .wrapping_add(byte_value(c_string_byte(bytes, j)) as u8)
                .wrapping_add(self.val_base[i]);
            self.val_base.swap(i, s as usize);
            j = next_c_string_index(bytes.len(), j);
        }
    }

    fn load_shadowname(&mut self, name: &str) {
        self.val = self.val_base;
        self.q_len = -1;

        let bytes = name.as_bytes();
        for _ in 0..2 {
            let mut s = 0u8;
            let mut j = bytes.len();
            for i in 0..N {
                s = s
                    .wrapping_add(byte_value(c_string_byte(bytes, j)) as u8)
                    .wrapping_add(self.val[i]);
                self.val.swap(i, s as usize);
                j = next_c_string_index(bytes.len(), j);
            }
        }

        self.q_len = -1;

        for i in (0..96).step_by(8) {
            for j in 0..8 {
                self.ual[i + j] = self.val[i + j].wrapping_mul(181).wrapping_add(160);
            }
        }

        for i in 0..96 {
            if self.ual[i] >= 89 && self.ual[i] < 217 && self.q_len < 30 {
                self.q_len += 1;
                self.name_base[self.q_len as usize] = self.ual[i] & 63;
            }
        }

        if self.q_len < 30 {
            for i in (96..N).step_by(8) {
                for j in 0..8 {
                    self.ual[i + j] = self.val[i + j].wrapping_mul(181).wrapping_add(160);
                }
            }

            for i in 96..N {
                if self.ual[i] >= 89 && self.ual[i] < 217 && self.q_len < 30 {
                    self.q_len += 1;
                    self.name_base[self.q_len as usize] = self.ual[i] & 63;
                }
            }
        }

        let prop0 = med3(self.name_base[10] as i32, self.name_base[11] as i32, self.name_base[12] as i32);
        let prop1 = med3(self.name_base[13] as i32, self.name_base[14] as i32, self.name_base[15] as i32);
        let prop2 = med3(self.name_base[16] as i32, self.name_base[17] as i32, self.name_base[18] as i32);
        let prop3 = med3(self.name_base[19] as i32, self.name_base[20] as i32, self.name_base[21] as i32);
        let prop4 = med3(self.name_base[22] as i32, self.name_base[23] as i32, self.name_base[24] as i32);
        let prop5 = med3(self.name_base[25] as i32, self.name_base[26] as i32, self.name_base[27] as i32);
        let prop6 = med3(self.name_base[28] as i32, self.name_base[29] as i32, self.name_base[30] as i32);

        self.name_base[..10].sort();
        let prop7 = 154
            + self.name_base[3] as i32
            + self.name_base[4] as i32
            + self.name_base[5] as i32
            + self.name_base[6] as i32;

        self.cfz = (prop0 - prop1 + prop2 + prop4 - prop5) * 2 + prop3 + prop6 + 144;
        self.shadowi = prop0 as f64 * 2.8
            + prop1 as f64 * 0.6
            + prop2 as f64 * 2.5
            + prop3 as f64 * 1.2
            + prop4 as f64
            + prop5 as f64
            - 1.2 * prop6 as f64
            + 0.8 * prop7 as f64;
    }

    fn load_name(&mut self, name: &str) {
        self.val = self.val_base;
        self.q_len = -1;
        self.last = -1;

        let bytes = name.as_bytes();
        for _ in 0..2 {
            let mut s = 0u8;
            let mut j = bytes.len();
            for i in 0..N {
                s = s
                    .wrapping_add(byte_value(c_string_byte(bytes, j)) as u8)
                    .wrapping_add(self.val[i]);
                self.val.swap(i, s as usize);
                j = next_c_string_index(bytes.len(), j);
            }
        }

        self.q_len = -1;

        for i in (0..N).step_by(8) {
            for j in 0..8 {
                self.ual[i + j] = self.val[i + j].wrapping_mul(181).wrapping_add(160);
            }
        }

        for i in 0..N {
            if self.ual[i] >= 89 && self.ual[i] < 217 {
                self.q_len += 1;
                if (self.q_len as usize) < M {
                    self.name_base[self.q_len as usize] = self.ual[i] & 63;
                }
            }
        }

        for i in 0..SKILL_CNT {
            self.skill[i] = i as u8;
        }

        self.freq = [0; 16];
        self.p = 0;
        self.q = 0;

        let mut s = 0usize;
        for _ in 0..2 {
            for i in 0..SKILL_CNT {
                s = (s + self.next_skill_index() + self.skill[i] as usize) % SKILL_CNT;
                self.skill.swap(i, s);
            }
        }

        let a = K;
        for (j, i) in (0..K).step_by(4).enumerate() {
            let mn = min4(
                self.name_base[a + i],
                self.name_base[a + i + 1],
                self.name_base[a + i + 2],
                self.name_base[a + i + 3],
            );
            if mn > 10 && self.skill[j] < 35 && self.skill[j] < 25 {
                self.last = j as i32;
            }
        }
    }

    fn effective_skill_value(&self, idx: usize) -> f64 {
        // 对齐修复版 CPP 的 write_sparse_skill_row：
        // get_43() 中如果存在护盾，会把 x[26] 移到 x[45] 并清零 x[26]。
        // x[26] 对应技能“铁壁”；x[45] 是模型特征拆分，不代表铁壁技能不存在。
        if idx == 18 && self.x[45] != 0.0 {
            self.x[45]
        } else {
            self.x[8 + idx]
        }
    }

    fn get_43(&mut self) {
        self.name_base[..10].sort();

        self.x = [0.0; 50];
        self.freq = [0; 16];

        self.x[0] = 154.0
            + self.name_base[3] as f64
            + self.name_base[4] as f64
            + self.name_base[5] as f64
            + self.name_base[6] as f64;
        self.x[1] = 36.0 + med3(self.name_base[10] as i32, self.name_base[11] as i32, self.name_base[12] as i32) as f64;
        self.x[2] = 36.0 + med3(self.name_base[13] as i32, self.name_base[14] as i32, self.name_base[15] as i32) as f64;
        self.x[3] = 36.0 + med3(self.name_base[16] as i32, self.name_base[17] as i32, self.name_base[18] as i32) as f64;
        self.x[4] = 36.0 + med3(self.name_base[19] as i32, self.name_base[20] as i32, self.name_base[21] as i32) as f64;
        self.x[5] = 36.0 + med3(self.name_base[22] as i32, self.name_base[23] as i32, self.name_base[24] as i32) as f64;
        self.x[6] = 36.0 + med3(self.name_base[25] as i32, self.name_base[26] as i32, self.name_base[27] as i32) as f64;
        self.x[7] = 36.0 + med3(self.name_base[28] as i32, self.name_base[29] as i32, self.name_base[30] as i32) as f64;

        self.cfz = ((self.x[1] - self.x[2] + self.x[3] + self.x[5] - self.x[6]) * 2.0
            + self.x[4]
            + self.x[7]) as i32;

        let a = K;
        for (j, i) in (0..K).step_by(4).enumerate() {
            let mn = min4(
                self.name_base[a + i],
                self.name_base[a + i + 1],
                self.name_base[a + i + 2],
                self.name_base[a + i + 3],
            );
            self.freq[j] = if mn > 10 && self.skill[j] < 35 {
                mn - 10
            } else {
                0
            };
        }

        if self.last != -1 {
            let idx = self.last as usize;
            self.freq[idx] = self.freq[idx].wrapping_shl(1);
        }

        if self.freq[14] != 0 && self.last != 14 {
            self.freq[14] = self.freq[14].wrapping_add(min3_u8(self.name_base[60], self.name_base[61], self.freq[14]));
        }

        if self.freq[15] != 0 && self.last != 15 {
            self.freq[15] = self.freq[15].wrapping_add(min3_u8(self.name_base[62], self.name_base[63], self.freq[15]));
        }

        let mut zd = 1.0_f64;
        let mut kill = 1.0_f64;

        for k in 0..16 {
            let sk = self.skill[k] as usize;
            let freq = self.freq[k] as f64;

            if sk == 9 || sk == 16 {
                self.x[sk + 8] = zd * freq;
                zd *= 1.0 - freq * 0.3 / 128.0;
            } else if sk == 18 {
                self.x[sk + 8] = zd * freq;
                zd *= 1.0 - freq * 0.35 / 128.0;
            } else if sk == 19 || sk == 23 {
                self.x[sk + 8] = zd * freq;
                zd *= 1.0 - freq * 0.6 / 128.0;
            } else if sk == 20 || sk == 22 {
                self.x[sk + 8] = zd * freq;
                zd *= 1.0 - freq * 0.7 / 128.0;
            } else if sk < 25 {
                self.x[sk + 8] = zd * freq;
                zd *= 1.0 - freq / 128.0;
            } else if sk == 31 || sk == 32 {
                self.x[sk + 8] = kill * freq;
                kill *= 1.0 - freq / 128.0;
            } else if sk < 35 {
                self.x[sk + 8] = freq;
            }
        }

        if self.x[37] <= 70.0 {
            self.x[37] = self.x[37] * self.x[37] / 70.0;
        } else {
            self.x[37] = self.x[37] * 2.0 - 70.0;
        }

        if self.x[32] > 0.0 {
            self.x[43] = self.shadowi * self.x[32] / 100.0;
        } else {
            self.x[43] = 0.0;
        }

        if self.x[42] > 0.0 {
            self.x[44] = 1.0;
        }

        if self.x[37] > 0.0 {
            self.x[45] = self.x[26];
            self.x[26] = 0.0;
        }
    }
}

fn build_single_raw(raw: &str) -> Name {
    let s = trim_line(raw);
    let (name, team) = split_name_team(&s);

    let mut x = Name::default();
    let mut y = Name::default();

    x.load_team(&team);
    y.val_base = x.val_base;

    x.load_name(&name);
    y.load_shadowname(&(name + "?shadow"));

    x.shadowi = y.shadowi;
    x.shadowcfz = y.cfz;
    x
}

fn apply_group_bonus(original: &[Name]) -> Vec<Name> {
    let mut boosted = original.to_vec();

    for i in 0..original.len() {
        for j in (i + 1)..original.len() {
            for k in 7..M {
                if original[j].name_base[k - 1] == original[i].name_base[k] {
                    boosted[i].name_base[k] = boosted[i].name_base[k].max(original[j].name_base[k]);
                }

                if original[i].name_base[k - 1] == original[j].name_base[k] {
                    boosted[j].name_base[k] = boosted[j].name_base[k].max(original[i].name_base[k]);
                }
            }
        }
    }

    boosted
}

fn split_name_team(s: &str) -> (String, String) {
    if let Some(at) = s.rfind('@') {
        // 对齐修复版 CPP：build() 不会修剪 @ 两侧的空格。
        // 例如 "ez^}'B @Hell" 中 B 后面的空格属于名字，会影响技能结果。
        (s[..at].to_string(), s[at + 1..].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

fn trim_line(s: &str) -> String {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    s.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '\0')
        .to_string()
}

#[inline]
fn byte_value(b: u8) -> i32 {
    (b as i8) as i32
}

#[inline]
fn c_string_byte(bytes: &[u8], index: usize) -> u8 {
    if index == bytes.len() {
        0
    } else {
        bytes[index]
    }
}

#[inline]
fn next_c_string_index(len: usize, index: usize) -> usize {
    if index == len {
        0
    } else {
        index + 1
    }
}

#[inline]
fn med3(x: i32, y: i32, z: i32) -> i32 {
    if x < y {
        if x < z {
            if y < z { y } else { z }
        } else {
            x
        }
    } else if y < z {
        if x < z { x } else { z }
    } else {
        y
    }
}

#[inline]
fn min4(a: u8, b: u8, c: u8, d: u8) -> u8 {
    a.min(b).min(c).min(d)
}

#[inline]
fn min3_u8(a: u8, b: u8, c: u8) -> u8 {
    a.min(b).min(c)
}
