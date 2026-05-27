use crate::player::skill::SkillBoost;

#[derive(Debug, Clone, Default)]
pub struct MinionOverlay {
    pub attrs: Option<[i32; 8]>,
    pub skills: Option<Vec<(String, SkillBoost)>>,
    pub reuse_skills_on_recast: bool,
    pub inherit_owner_def_res: bool,
}

/// DIY / OL overlay data parsed from a player's inline suffix.
#[derive(Debug, Clone)]
pub struct PlayerOverlay {
    /// Attribute override: [atk, def, spd, agi, mag, res, wis, max_hp].
    pub attrs: Option<[i32; 8]>,
    /// Ordered skill overrides. Order controls active skill attempt order.
    pub skills: Option<Vec<(String, SkillBoost)>>,
    /// Weapon name override. Ignored by the weapon system when attrs or skills
    /// are present, but kept for export/display compatibility.
    pub weapon: Option<String>,
    /// When false, forces name_factor to 0.
    pub name_factor_enabled: bool,
    /// Optional configs for combat minions spawned by this player.
    pub shadow: Option<MinionOverlay>,
    pub summon: Option<MinionOverlay>,
    pub zombie: Option<MinionOverlay>,
}

impl Default for PlayerOverlay {
    fn default() -> Self {
        Self {
            attrs: None,
            skills: None,
            weapon: None,
            name_factor_enabled: true,
            shadow: None,
            summon: None,
            zombie: None,
        }
    }
}

impl PlayerOverlay {
    pub fn parse_inline(segment: &str) -> Option<Self> {
        let segment = segment.trim();
        if let Some(rest) = segment.strip_prefix("diy[") {
            Self::parse_diy(rest)
        } else if let Some(rest) = segment.strip_prefix("ol:") {
            Self::parse_object(rest)
        } else {
            None
        }
    }

    fn parse_diy(rest: &str) -> Option<Self> {
        let (attrs_raw, suffix) = rest.split_once(']')?;
        let attrs = parse_attrs(attrs_raw)?;
        let mut overlay = Self {
            attrs: Some(decode_overlay_attrs(attrs)),
            ..Default::default()
        };
        let suffix = suffix.trim();
        if !suffix.is_empty() {
            overlay.skills = Some(parse_skill_map(suffix)?);
        }
        Some(overlay)
    }

    fn parse_object(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let raw = raw.strip_prefix('{')?.strip_suffix('}')?;
        let mut overlay = Self::default();
        let mut idx = 0usize;
        let bytes = raw.as_bytes();

        while idx < bytes.len() {
            skip_ws_and_commas(raw, &mut idx);
            if idx >= bytes.len() {
                break;
            }
            let (key, next_idx) = parse_quoted_string(raw, idx)?;
            idx = next_idx;
            skip_ws(raw, &mut idx);
            if bytes.get(idx).copied() != Some(b':') {
                return None;
            }
            idx += 1;
            skip_ws(raw, &mut idx);
            let (value, next_idx) = extract_json_value(raw, idx)?;

            match key.as_str() {
                "attrs" => overlay.attrs = Some(decode_overlay_attrs(parse_attrs(value)?)),
                "skills" => overlay.skills = Some(parse_skill_map(value)?),
                "weapon" => overlay.weapon = Some(parse_scalar_string(value)?),
                "name_factor_enabled" => overlay.name_factor_enabled = parse_bool(value)?,
                "shadow" | "phantom" | "幻影" => overlay.shadow = Some(parse_minion_overlay(value)?),
                "summon" | "familiar" | "使魔" => overlay.summon = Some(parse_minion_overlay(value)?),
                "zombie" | "丧尸" | "僵尸" => overlay.zombie = Some(parse_minion_overlay(value)?),
                _ => {}
            }
            idx = next_idx;
        }

        Some(overlay)
    }
}

fn decode_overlay_attrs(attrs: [i32; 8]) -> [i32; 8] {
    [
        (attrs[0] - 36).max(0),
        (attrs[1] - 36).max(0),
        (attrs[2] - 36).max(0),
        (attrs[3] - 36).max(0),
        (attrs[4] - 36).max(0),
        (attrs[5] - 36).max(0),
        (attrs[6] - 36).max(0),
        attrs[7],
    ]
}

fn parse_minion_overlay(raw: &str) -> Option<MinionOverlay> {
    let raw = raw.trim();
    let raw = raw.strip_prefix('{')?.strip_suffix('}')?;
    let mut overlay = MinionOverlay::default();
    let mut idx = 0usize;
    let bytes = raw.as_bytes();

    while idx < bytes.len() {
        skip_ws_and_commas(raw, &mut idx);
        if idx >= bytes.len() {
            break;
        }
        let (key, next_idx) = parse_quoted_string(raw, idx)?;
        idx = next_idx;
        skip_ws(raw, &mut idx);
        if bytes.get(idx).copied() != Some(b':') {
            return None;
        }
        idx += 1;
        skip_ws(raw, &mut idx);
        let (value, next_idx) = extract_json_value(raw, idx)?;

        match key.as_str() {
            "attrs" => overlay.attrs = Some(decode_overlay_attrs(parse_attrs(value)?)),
            "skills" => overlay.skills = Some(parse_skill_map(value)?),
            "reuse_skills_on_recast" => overlay.reuse_skills_on_recast = parse_bool(value)?,
            "inherit_owner_def_res" => overlay.inherit_owner_def_res = parse_bool(value)?,
            _ => {}
        }
        idx = next_idx;
    }

    Some(overlay)
}

fn parse_attrs(raw: &str) -> Option<[i32; 8]> {
    let mut values = [0_i32; 8];
    let mut count = 0usize;
    for part in raw.trim().trim_start_matches('[').trim_end_matches(']').split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if count >= values.len() {
            return None;
        }
        values[count] = part.parse().ok()?;
        count += 1;
    }
    (count == values.len()).then_some(values)
}

fn parse_skill_map(raw: &str) -> Option<Vec<(String, SkillBoost)>> {
    let raw = raw.trim();
    let raw = raw.strip_prefix('{')?.strip_suffix('}')?;
    let mut list = Vec::new();
    let mut idx = 0usize;
    let bytes = raw.as_bytes();

    while idx < bytes.len() {
        skip_ws_and_commas(raw, &mut idx);
        if idx >= bytes.len() {
            break;
        }
        let (key, next_idx) = parse_quoted_string(raw, idx)?;
        idx = next_idx;
        skip_ws(raw, &mut idx);
        if bytes.get(idx).copied() != Some(b':') {
            return None;
        }
        idx += 1;
        skip_ws(raw, &mut idx);

        let value = if bytes.get(idx).copied() == Some(b'"') {
            let (str_val, next_idx) = parse_quoted_string(raw, idx)?;
            idx = next_idx;
            SkillBoost::parse(&str_val)?
        } else {
            let start = idx;
            while idx < bytes.len() && bytes[idx] != b',' && bytes[idx] != b'}' {
                idx += 1;
            }
            SkillBoost::Normal(raw[start..idx].trim().parse().ok()?)
        };
        list.push((key, value));
    }

    Some(list)
}

fn parse_scalar_string(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('"') {
        let (value, _) = parse_quoted_string(raw, 0)?;
        Some(value)
    } else if raw.is_empty() {
        Some(String::new())
    } else {
        Some(raw.to_string())
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn extract_json_value(raw: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = raw.as_bytes();
    let mut idx = start;
    match bytes.get(idx).copied()? {
        b'"' => {
            let (_, end) = parse_quoted_string(raw, idx)?;
            Some((&raw[start..end], end))
        }
        b'[' => extract_balanced(raw, idx, '[', ']'),
        b'{' => extract_balanced(raw, idx, '{', '}'),
        _ => {
            while idx < bytes.len() && bytes[idx] != b',' {
                idx += 1;
            }
            Some((raw[start..idx].trim(), idx))
        }
    }
}

fn extract_balanced(raw: &str, start: usize, open: char, close: char) -> Option<(&str, usize)> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut begin = None;

    for (offset, ch) in raw[start..].char_indices() {
        let idx = start + offset;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            c if c == open => {
                if depth == 0 {
                    begin = Some(idx);
                }
                depth += 1;
            }
            c if c == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let begin = begin?;
                    let end = idx + ch.len_utf8();
                    return Some((&raw[begin..end], end));
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_quoted_string(raw: &str, start: usize) -> Option<(String, usize)> {
    let mut chars = raw[start..].char_indices();
    let (_, first) = chars.next()?;
    if first != '"' {
        return None;
    }

    let mut out = String::new();
    let mut escaped = false;
    for (offset, ch) in chars {
        let idx = start + offset;
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some((out, idx + ch.len_utf8())),
            _ => out.push(ch),
        }
    }
    None
}

fn skip_ws(raw: &str, idx: &mut usize) {
    while *idx < raw.len() {
        let Some(ch) = raw[*idx..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            *idx += ch.len_utf8();
        } else {
            break;
        }
    }
}

fn skip_ws_and_commas(raw: &str, idx: &mut usize) {
    while *idx < raw.len() {
        let Some(ch) = raw[*idx..].chars().next() else {
            break;
        };
        if ch.is_whitespace() || ch == ',' {
            *idx += ch.len_utf8();
        } else {
            break;
        }
    }
}
