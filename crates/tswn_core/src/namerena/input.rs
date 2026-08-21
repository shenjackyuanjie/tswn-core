use std::fmt;

use super::PlayerOverlay;

pub const NAME_MAX_LEN: usize = 256;
pub const TEAM_MAX_LEN: usize = 256;
pub const SEED_PREFIX: &str = "seed:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerClass {
    Normal,
    Clone,
    Boss,
    Boost,
    Seed,
    Test1,
    Test2,
    TestEx,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSpec {
    pub raw: String,
    pub name: String,
    pub team: Option<String>,
    pub weapon: Option<String>,
    pub overlay: Option<PlayerOverlay>,
    pub class: PlayerClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerSpecError {
    NameTooLong { len: usize, max: usize },
    TeamTooLong { len: usize, max: usize },
}

impl fmt::Display for PlayerSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTooLong { len, max } => write!(f, "player name is too long: {len} bytes (max {max})"),
            Self::TeamTooLong { len, max } => write!(f, "team name is too long: {len} bytes (max {max})"),
        }
    }
}

impl std::error::Error for PlayerSpecError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamerenaInput {
    pub groups: Vec<Vec<PlayerSpec>>,
    pub seed: Vec<String>,
}

impl PlayerSpec {
    pub fn parse(raw: &str) -> Result<Self, PlayerSpecError> {
        let raw = trim_js_line_end(raw);
        let (identity, suffix) = raw.split_once('+').map_or((raw, None), |(identity, suffix)| (identity, Some(suffix)));
        let (name, team) = identity
            .split_once('@')
            .map_or((identity, None), |(name, team)| (name, Some(trim_js_line_end(team))));
        // JS `parse_names` 将带 `:` 的 `@` 后缀当作无队名处理；这类形式常被
        // `/namer-pf` 输入附带的元数据使用，不能参与 clan 的 RC4 与属性构造。
        let team = team.filter(|team| !team.is_empty() && !team.contains(':'));

        if name.len() > NAME_MAX_LEN {
            return Err(PlayerSpecError::NameTooLong {
                len: name.len(),
                max: NAME_MAX_LEN,
            });
        }
        if let Some(team) = team
            && team.len() > TEAM_MAX_LEN
        {
            return Err(PlayerSpecError::TeamTooLong {
                len: team.len(),
                max: TEAM_MAX_LEN,
            });
        }

        let mut weapon = None::<String>;
        let mut overlay = None;
        if let Some(suffix) = suffix {
            for segment in split_by_plus_outside_quotes(suffix) {
                let segment = trim_js_name_like(&segment);
                if segment.is_empty() {
                    continue;
                }
                if let Some(parsed) = PlayerOverlay::parse_inline(segment) {
                    overlay = Some(parsed);
                } else if let Some(existing) = &mut weapon {
                    existing.push('+');
                    existing.push_str(segment);
                } else {
                    weapon = Some(segment.to_owned());
                }
            }
        }

        let name = name.to_owned();
        let team = team.map(str::to_owned);
        let class = classify(&name, team.as_deref());
        Ok(Self {
            raw: raw.to_owned(),
            name,
            team,
            weapon,
            overlay,
            class,
        })
    }

    pub fn id_name(&self) -> String { raw_namerena_to_id_name(&self.raw) }
}

impl NamerenaInput {
    pub fn parse(raw_input: &str) -> Result<Self, PlayerSpecError> {
        let normalized = raw_input.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = normalized.split('\n').map(trim_js_line_end).map(str::to_owned).collect::<Vec<_>>();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        let seed = lines.iter().filter(|line| is_seed_line(line)).cloned().collect::<Vec<_>>();

        let raw_groups = if lines.iter().any(String::is_empty) {
            split_blank_line_groups(lines)
        } else {
            lines.into_iter().map(|line| vec![line]).collect()
        };
        Self::from_raw_groups_with_seed(&raw_groups, seed)
    }

    pub fn from_raw_groups(raw_groups: &[Vec<String>]) -> Result<Self, PlayerSpecError> {
        let seed = raw_groups.iter().flatten().filter(|line| is_seed_line(line)).cloned().collect();
        Self::from_raw_groups_with_seed(raw_groups, seed)
    }

    fn from_raw_groups_with_seed(raw_groups: &[Vec<String>], seed: Vec<String>) -> Result<Self, PlayerSpecError> {
        let mut groups = Vec::with_capacity(raw_groups.len());
        for raw_group in raw_groups {
            let mut group = Vec::with_capacity(raw_group.len());
            for raw in raw_group {
                if !is_seed_line(raw) {
                    group.push(PlayerSpec::parse(raw)?);
                }
            }
            if !group.is_empty() {
                groups.push(group);
            }
        }
        Ok(Self { groups, seed })
    }
}

pub fn is_seed_line(raw: &str) -> bool { raw.starts_with(SEED_PREFIX) }

pub fn raw_namerena_to_id_name(raw: &str) -> String {
    let mut output = String::new();
    raw_namerena_to_id_name_into(raw, &mut output);
    output
}

pub(crate) fn raw_namerena_to_id_name_into(raw: &str, output: &mut String) {
    output.clear();
    let no_weapon = raw.split_once('+').map_or(raw, |(left, _)| left);
    let Some((name, team)) = no_weapon.split_once('@') else {
        output.push_str(no_weapon);
        return;
    };
    if team.is_empty() || team == name || team.contains(':') {
        output.push_str(name);
    } else {
        output.push_str(name);
        output.push('@');
        output.push_str(team);
    }
}

fn classify(name: &str, team: Option<&str>) -> PlayerClass {
    const BOSS_NAMES: [&str; 12] = [
        "mario",
        "sonic",
        "mosquito",
        "yuri",
        "slime",
        "ikaruga",
        "conan",
        "aokiji",
        "lazy",
        "covid",
        "saitama",
        "testsubject",
    ];
    const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];
    match team {
        Some("!") if BOSS_NAMES.contains(&name) => PlayerClass::Boss,
        Some("!") if BOOST_NAMES.contains(&name) => PlayerClass::Boost,
        Some("!") if name.starts_with(SEED_PREFIX) => PlayerClass::Seed,
        Some("!") => PlayerClass::TestEx,
        Some("\u{0002}") => PlayerClass::Test1,
        Some("\u{0003}") => PlayerClass::Test2,
        Some(_) if name.starts_with(SEED_PREFIX) => PlayerClass::Seed,
        _ => PlayerClass::Normal,
    }
}

fn split_blank_line_groups(lines: Vec<String>) -> Vec<Vec<String>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.is_empty() {
            if !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn split_by_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == '+' {
            segments.push(raw[start..index].to_owned());
            start = index + ch.len_utf8();
        }
    }
    segments.push(raw[start..].to_owned());
    segments
}

fn is_js_regex_space(ch: char) -> bool {
    matches!(ch as u32, 9..=13 | 32 | 160 | 5760 | 8192..=8202 | 8232..=8233 | 8239 | 8287 | 12288 | 65279)
}

fn is_js_trim_name_char(ch: char) -> bool {
    matches!(ch as u32, 9..=13 | 32 | 133 | 160 | 5760 | 8192..=8202 | 8232..=8233 | 8239 | 8287 | 12288 | 65279)
}

pub(crate) fn trim_js_line_end(value: &str) -> &str { value.trim_end_matches(is_js_regex_space) }

fn trim_js_name_like(value: &str) -> &str {
    let trimmed = value.trim_matches(is_js_regex_space);
    let start = if trimmed.starts_with('\u{0085}') {
        trimmed
            .char_indices()
            .find_map(|(index, ch)| (!is_js_trim_name_char(ch)).then_some(index))
            .unwrap_or(trimmed.len())
    } else {
        0
    };
    let trimmed = &trimmed[start..];
    if trimmed.ends_with('\u{0085}') {
        let end = trimmed
            .char_indices()
            .rev()
            .find_map(|(index, ch)| (!is_js_trim_name_char(ch)).then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        &trimmed[..end]
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namerena::SkillBoost;

    #[test]
    fn parses_identity_weapon_and_overlay_without_splitting_quoted_plus() {
        let spec = PlayerSpec::parse(r#"alice@red+blade+ol:{"skills":{"fire":"40+30"},"weapon":"x+y"}"#).unwrap();
        assert_eq!(spec.name, "alice");
        assert_eq!(spec.team.as_deref(), Some("red"));
        assert_eq!(spec.weapon.as_deref(), Some("blade"));
        assert_eq!(
            spec.overlay.as_ref().and_then(|overlay| overlay.skills.as_ref()).unwrap()[0].1,
            SkillBoost::SlotBoost { base: 40, boost: 30 }
        );
        assert_eq!(spec.id_name(), "alice@red");
    }

    #[test]
    fn parses_raw_groups_and_extracts_seed_lines() {
        let input = NamerenaInput::parse("seed:7@!\r\na\r\n\r\nb@team\r\n").unwrap();
        assert_eq!(input.seed, ["seed:7@!"]);
        assert_eq!(input.groups.len(), 2);
        assert_eq!(input.groups[0][0].name, "a");
        assert_eq!(input.groups[1][0].id_name(), "b@team");
    }

    #[test]
    fn parses_trailing_empty_team_as_no_team() {
        let spec = PlayerSpec::parse("宗铭丸 #DCVIDQJX@").unwrap();
        assert_eq!(spec.name, "宗铭丸 #DCVIDQJX");
        assert_eq!(spec.team, None);
        assert_eq!(spec.id_name(), "宗铭丸 #DCVIDQJX");
    }

    #[test]
    fn parses_colon_team_suffix_as_no_team() {
        let spec = PlayerSpec::parse("荧屏上的梦想 NWKm7L_6@dream_on_screen qp:6070").unwrap();
        assert_eq!(spec.name, "荧屏上的梦想 NWKm7L_6");
        assert_eq!(spec.team, None);
        assert_eq!(spec.id_name(), "荧屏上的梦想 NWKm7L_6");
    }

    #[test]
    fn classifies_special_profiles() {
        assert_eq!(PlayerSpec::parse("covid@!").unwrap().class, PlayerClass::Boss);
        assert_eq!(PlayerSpec::parse("云剑狄卡敢@!").unwrap().class, PlayerClass::Boost);
        assert_eq!(PlayerSpec::parse("target@!").unwrap().class, PlayerClass::TestEx);
        assert_eq!(PlayerSpec::parse("target@\u{0002}").unwrap().class, PlayerClass::Test1);
    }
}
