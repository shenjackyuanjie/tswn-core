use crate::error::{Ds3Error, Ds3Result};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScoreMode {
    Bc = 1,
    Fz = 2,
    Wc = 3,
    Fs = 4,
    Pj = 5,
}

impl ScoreMode {
    pub const ALL: [Self; 5] = [Self::Bc, Self::Fz, Self::Wc, Self::Fs, Self::Pj];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bc => "bc",
            Self::Fz => "fz",
            Self::Wc => "wc",
            Self::Fs => "fs",
            Self::Pj => "pj",
        }
    }

    pub fn from_i32(value: i32) -> Ds3Result<Self> {
        match value {
            1 => Ok(Self::Bc),
            2 => Ok(Self::Fz),
            3 => Ok(Self::Wc),
            4 => Ok(Self::Fs),
            5 => Ok(Self::Pj),
            _ => Err(Ds3Error::parse(format!("invalid score mode: {value}"))),
        }
    }
}

pub fn init_pos_tables() -> ([usize; 46], [[Option<usize>; 46]; 46]) {
    let mut pos = [0usize; 46];
    let mut pos2 = [[None; 46]; 46];
    let mut count = 0usize;

    for p in &mut pos {
        count += 1;
        *p = count;
    }

    for i in 0..46 {
        for j in i..46 {
            if (i == 26 && j == 37) || (i == 26 && j == 45) || (i == 42 && j == 44) || (i == 44 && j == 44) {
                continue;
            }
            count += 1;
            pos2[i][j] = Some(count);
            pos2[j][i] = Some(count);
        }
    }

    (pos, pos2)
}

pub fn base_score(x: &[f64; 46], model: &[f64; 1124], pos: &[usize; 46], pos2: &[[Option<usize>; 46]; 46]) -> f64 {
    let mut score = model[0];
    for i in 0..46 {
        score += model[pos[i]] * x[i];
    }
    for i in 0..46 {
        for j in i..46 {
            if let Some(k) = pos2[i][j] {
                score += model[k] * x[i] * x[j];
            }
        }
    }
    score
}

pub fn gradient(x: &[f64; 46], model: &[f64; 1124], pos: &[usize; 46], pos2: &[[Option<usize>; 46]; 46]) -> [f64; 46] {
    let mut dt = [0.0_f64; 46];
    for i in 0..46 {
        let mut sum = model[pos[i]];
        for (j, value) in x.iter().enumerate() {
            if let Some(k) = pos2[i][j] {
                sum += (1.0 + if i == j { 1.0 } else { 0.0 }) * model[k] * *value;
            }
        }
        dt[i] = sum;
    }
    dt
}

#[derive(Debug, Clone)]
pub struct NameFeature {
    pub name: String,
    pub name_base: [u8; 128],
    pub skill: [u8; 40],
    pub val_base: [u8; 256],
    pub x: [f64; 46],
    pub last: i32,
    pub freq14: bool,
    pub freq15: bool,
    pub shadowi: f64,
    pub shadowcfz: i32,
}

impl NameFeature {
    pub fn from_full_name(full: &str) -> Ds3Result<Self> {
        let bytes = full.as_bytes();
        if bytes.is_empty() {
            return Err(Ds3Error::parse("invalid full name: empty"));
        }

        // Match C++ build() trimming rules:
        // - trim only leading/trailing ' ' and '+'
        // - keep other trailing chars (notably '\r')
        let mut l = 0i32;
        let mut r = bytes.len() as i32 - 1;
        while l <= r && (bytes[l as usize] == b' ' || bytes[l as usize] == 0 || bytes[l as usize] == b'+') {
            l += 1;
        }
        while l <= r && (bytes[r as usize] == b' ' || bytes[r as usize] == 0 || bytes[r as usize] == b'+') {
            r -= 1;
        }
        if l > r {
            return Err(Ds3Error::parse(format!("invalid full name: {full}")));
        }

        let mut at_pos: Option<usize> = None;
        for i in (l as usize)..=(r as usize) {
            if bytes[i] == b'@' {
                at_pos = Some(i);
            }
        }
        let at_pos = at_pos.ok_or_else(|| Ds3Error::parse(format!("name lacks @team: {full}")))?;
        if at_pos <= l as usize || at_pos >= r as usize {
            return Err(Ds3Error::parse(format!("invalid full name: {full}")));
        }

        let name = &bytes[(l as usize)..at_pos];
        let team = &bytes[(at_pos + 1)..=(r as usize)];

        let mut core = NameCore::new();
        core.load_team(team);

        let val_base = core.val_base;
        let mut main = NameCore {
            val_base,
            ..NameCore::new()
        };
        let mut shadow = NameCore {
            val_base,
            ..NameCore::new()
        };

        main.last = -1;
        main.load_name(name);
        let mut shadow_name = Vec::with_capacity(name.len() + 7);
        shadow_name.extend_from_slice(name);
        shadow_name.extend_from_slice(b"?shadow");
        shadow.load_shadowname(&shadow_name);
        main.shadowi = shadow.shadowi;
        main.shadowcfz = shadow.cfz;
        main.get_43();

        let normalized = String::from_utf8_lossy(&bytes[(l as usize)..=(r as usize)]).to_string();
        Ok(Self {
            name: normalized,
            name_base: main.name_base,
            skill: main.skill,
            val_base: main.val_base,
            x: main.x,
            last: main.last,
            freq14: main.freq14,
            freq15: main.freq15,
            shadowi: main.shadowi,
            shadowcfz: main.shadowcfz,
        })
    }

    pub fn recompute_x(&self) -> [f64; 46] {
        let mut core = NameCore::new();
        core.val_base = self.val_base;
        core.name_base = self.name_base;
        core.skill = self.skill;
        core.last = self.last;
        core.freq14 = self.freq14;
        core.freq15 = self.freq15;
        core.shadowi = self.shadowi;
        core.shadowcfz = self.shadowcfz;
        core.get_43();
        core.x
    }
}

#[derive(Debug, Clone)]
pub struct ScoreProfile {
    pub mode: ScoreMode,
    pub score: f64,
    pub potential: f64,
    pub feature: NameFeature,
}

impl ScoreProfile {
    pub fn to_line(&self) -> String { format!("{:.0} {:.0} {}", self.score, self.potential, self.feature.name) }
}

#[derive(Debug, Clone)]
pub struct ScoreContext {
    pub pos: [usize; 46],
    pub pos2: [[Option<usize>; 46]; 46],
}

impl ScoreContext {
    pub fn new() -> Self {
        let (pos, pos2) = init_pos_tables();
        Self { pos, pos2 }
    }
}

#[derive(Debug, Clone, Copy)]
struct NameCore {
    ual: [u8; 256],
    val: [u8; 256],
    val_base: [u8; 256],
    name_base: [u8; 128],
    freq: [u8; 16],
    skill: [u8; 40],
    p: u8,
    q: u8,
    base_hp: [u8; 10],
    last: i32,
    freq14: bool,
    freq15: bool,
    cfz: i32,
    shadowcfz: i32,
    shadowi: f64,
    x: [f64; 46],
}

impl NameCore {
    fn new() -> Self {
        Self {
            ual: [0; 256],
            val: [0; 256],
            val_base: [0; 256],
            name_base: [0; 128],
            freq: [0; 16],
            skill: [0; 40],
            p: 0,
            q: 0,
            base_hp: [0; 10],
            last: -1,
            freq14: false,
            freq15: false,
            cfz: 0,
            shadowcfz: 0,
            shadowi: 0.0,
            x: [0.0; 46],
        }
    }

    fn load_team(&mut self, team: &[u8]) {
        for i in 0..256 {
            self.val_base[i] = i as u8;
        }
        let t_len = team.len();
        let mut s = 0u8;
        let mut j = t_len;
        for i in 0..256 {
            let ch = if j == t_len { 0 } else { team[j] };
            s = s.wrapping_add(ch).wrapping_add(self.val_base[i]);
            self.val_base.swap(i, s as usize);
            j += 1;
            if j == t_len + 1 {
                j = 0;
            }
        }
    }

    fn load_shadowname(&mut self, name: &[u8]) {
        self.val = self.val_base;
        let t_len = name.len();
        for _ in 0..2 {
            let mut s = 0u8;
            let mut j = t_len;
            for i in 0..256 {
                let ch = if j == t_len { 0 } else { name[j] };
                s = s.wrapping_add(ch).wrapping_add(self.val[i]);
                self.val.swap(i, s as usize);
                j += 1;
                if j == t_len + 1 {
                    j = 0;
                }
            }
        }

        self.collect_name_base_limited(true);

        let mut prop = [0i32; 8];
        prop[0] = median(self.name_base[10], self.name_base[11], self.name_base[12]) as i32;
        prop[1] = median(self.name_base[13], self.name_base[14], self.name_base[15]) as i32;
        prop[2] = median(self.name_base[16], self.name_base[17], self.name_base[18]) as i32;
        prop[3] = median(self.name_base[19], self.name_base[20], self.name_base[21]) as i32;
        prop[4] = median(self.name_base[22], self.name_base[23], self.name_base[24]) as i32;
        prop[5] = median(self.name_base[25], self.name_base[26], self.name_base[27]) as i32;
        prop[6] = median(self.name_base[28], self.name_base[29], self.name_base[30]) as i32;

        let mut hp = [0u8; 10];
        hp.copy_from_slice(&self.name_base[0..10]);
        hp.sort_unstable();
        prop[7] = 154 + hp[3] as i32 + hp[4] as i32 + hp[5] as i32 + hp[6] as i32;
        self.cfz = (prop[0] - prop[1] + prop[2] + prop[4] - prop[5]) * 2 + prop[3] + prop[6] + 144;
        self.shadowi = prop[0] as f64 * 2.2
            + prop[1] as f64 * 0.75
            + prop[2] as f64 * 1.9
            + prop[3] as f64 * 1.3
            + prop[4] as f64 * 0.6
            + prop[5] as f64 * 1.2
            - prop[6] as f64 * 1.8
            + prop[7] as f64;
    }

    fn load_name(&mut self, name: &[u8]) {
        self.val = self.val_base;
        let t_len = name.len();
        for _ in 0..2 {
            let mut s = 0u8;
            let mut j = t_len;
            for i in 0..256 {
                let ch = if j == t_len { 0 } else { name[j] };
                s = s.wrapping_add(ch).wrapping_add(self.val[i]);
                self.val.swap(i, s as usize);
                j += 1;
                if j == t_len + 1 {
                    j = 0;
                }
            }
        }

        self.collect_name_base_limited(false);
        for i in 0..40 {
            self.skill[i] = i as u8;
        }
        self.freq = [0; 16];
        self.p = 0;
        self.q = 0;
        let mut s = 0usize;
        for _ in 0..2 {
            for i in 0..40 {
                s = (s + self.next_skill_index() as usize + self.skill[i] as usize) % 40;
                self.skill.swap(i, s);
            }
        }

        self.freq14 = false;
        self.freq15 = false;
        for (i, j) in (0..64).step_by(4).zip(0..16) {
            let p = self.name_base[64 + i]
                .min(self.name_base[64 + i + 1])
                .min(self.name_base[64 + i + 2])
                .min(self.name_base[64 + i + 3]);
            if p > 10 && self.skill[j] < 35 {
                if self.skill[j] < 25 {
                    self.last = j as i32;
                }
                if i == 56 {
                    self.freq14 = true;
                }
                if i == 60 {
                    self.freq15 = true;
                }
            }
        }
    }

    fn collect_name_base_limited(&mut self, prefer_96: bool) {
        let range_first = if prefer_96 { 0..96 } else { 0..256 };
        self.ual = [0; 256];
        self.name_base = [0; 128];

        for i in 0..256 {
            self.ual[i] = self.val[i].wrapping_mul(181).wrapping_add(160);
        }

        let mut q_len: i32 = -1;
        for i in range_first {
            if self.ual[i] >= 89 && self.ual[i] < 217 {
                q_len += 1;
                if q_len < 128 {
                    self.name_base[q_len as usize] = self.ual[i] & 63;
                }
                if prefer_96 && q_len >= 30 {
                    break;
                }
            }
        }

        if prefer_96 && q_len < 30 {
            for i in 96..256 {
                if self.ual[i] >= 89 && self.ual[i] < 217 {
                    q_len += 1;
                    if q_len < 128 {
                        self.name_base[q_len as usize] = self.ual[i] & 63;
                    }
                    if q_len >= 30 {
                        break;
                    }
                }
            }
        }
    }

    fn get_43(&mut self) {
        self.base_hp.copy_from_slice(&self.name_base[0..10]);
        self.base_hp.sort_unstable();
        self.x = [0.0; 46];

        self.x[0] =
            (154 + self.base_hp[3] as i32 + self.base_hp[4] as i32 + self.base_hp[5] as i32 + self.base_hp[6] as i32) as f64;
        self.x[1] = (36 + median(self.name_base[10], self.name_base[11], self.name_base[12]) as i32) as f64;
        self.x[2] = (36 + median(self.name_base[13], self.name_base[14], self.name_base[15]) as i32) as f64;
        self.x[3] = (36 + median(self.name_base[16], self.name_base[17], self.name_base[18]) as i32) as f64;
        self.x[4] = (36 + median(self.name_base[19], self.name_base[20], self.name_base[21]) as i32) as f64;
        self.x[5] = (36 + median(self.name_base[22], self.name_base[23], self.name_base[24]) as i32) as f64;
        self.x[6] = (36 + median(self.name_base[25], self.name_base[26], self.name_base[27]) as i32) as f64;
        self.x[7] = (36 + median(self.name_base[28], self.name_base[29], self.name_base[30]) as i32) as f64;
        self.cfz = ((self.x[1] - self.x[2] + self.x[3] + self.x[5] - self.x[6]) * 2.0 + self.x[4] + self.x[7]) as i32;

        for (i, j) in (0..64).step_by(4).zip(0..16) {
            let p = self.name_base[64 + i]
                .min(self.name_base[64 + i + 1])
                .min(self.name_base[64 + i + 2])
                .min(self.name_base[64 + i + 3]);
            self.freq[j] = if p > 10 && self.skill[j] < 35 { p - 10 } else { 0 };
        }
        if self.last != -1 {
            let idx = self.last as usize;
            self.freq[idx] = self.freq[idx].saturating_mul(2);
        }
        if self.freq14 && self.last != 14 {
            let add = self.name_base[60].min(self.name_base[61]).min(self.freq[14]);
            self.freq[14] = self.freq[14].saturating_add(add);
        }
        if self.freq15 && self.last != 15 {
            let add = self.name_base[62].min(self.name_base[63]).min(self.freq[15]);
            self.freq[15] = self.freq[15].saturating_add(add);
        }

        let mut zd = 1.0;
        let mut kill = 1.0;
        let skill_para = [
            1.0, 1.0, 1.0, 0.5, 0.75, 0.75, 1.0, 1.0, 0.75, 0.5, 1.0, 1.0, 1.0, 0.75, 1.0, 0.75, 0.2, 1.0, 0.75, 0.5, 0.3, 0.75,
            0.75, 0.3, 0.75,
        ];

        for k in 0..16 {
            let sk = self.skill[k] as usize;
            let freq = self.freq[k] as f64;
            if sk < 25 {
                let idx = sk + 8;
                if idx < 46 {
                    self.x[idx] = zd * freq;
                }
                zd *= 1.0 - freq * skill_para[sk] / 128.0;
            } else if sk == 31 || sk == 32 {
                let bounded = self.freq[k].min(64) as f64;
                let idx = sk + 8;
                if idx < 46 {
                    self.x[idx] = kill * bounded;
                }
                kill *= 1.0 - bounded * 0.8 / 128.0;
            } else {
                let idx = sk + 8;
                if idx < 46 {
                    self.x[idx] = freq;
                }
            }
        }

        if self.x[37] <= 60.0 {
            self.x[37] = self.x[37] * self.x[37] / 60.0;
        } else {
            self.x[37] = self.x[37] * 2.0 - 60.0;
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

    fn m(&mut self) -> u8 {
        self.p = self.p.wrapping_add(1);
        self.q = self.q.wrapping_add(self.val[self.p as usize]);
        self.val.swap(self.p as usize, self.q as usize);
        self.val[(self.val[self.p as usize].wrapping_add(self.val[self.q as usize])) as usize]
    }

    fn next_skill_index(&mut self) -> u8 {
        let u = self.m() as u16;
        let v = self.m() as u16;
        ((u << 8 | v) % 40) as u8
    }
}

#[inline(always)]
fn median(x: u8, y: u8, z: u8) -> u8 {
    if x < y {
        if x < z { if y < z { y } else { z } } else { x }
    } else if y < z {
        if x < z { x } else { z }
    } else {
        y
    }
}

#[cfg(test)]
mod tests {
    use super::NameFeature;

    #[test]
    fn parse_full_name_feature() {
        let feature = NameFeature::from_full_name("test@team").expect("feature parse");
        assert_eq!(feature.name, "test@team");
        assert!(feature.x.iter().any(|value| *value != 0.0));
    }
}
