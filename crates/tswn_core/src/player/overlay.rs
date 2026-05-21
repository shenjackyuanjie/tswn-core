use crate::player::skill::SkillBoost;

/// 玩家 DIY / overlay 覆盖数据。
///
/// 允许通过名字后缀直接指定八围属性、技能等级和武器，
/// 跳过正常的随机生成流程，方便测试和调试。
///
/// 支持两种输入格式：
/// - `diy[72,39,69,76,67,66,0,84]{"sklfire":5}` — 紧凑格式，属性值 -36 后取非负
/// - `ol:{"attrs":[...],"skills":{...},"weapon":"..."}` — JSON 对象格式，属性值原样使用
///
/// 技能等级支持三种 SkillBoost 格式：
/// - 纯数字 `5` → [`SkillBoost::Normal`]
/// - 字符串 `"40+30"` → [`SkillBoost::SlotBoost`]（末尾座位加成）
/// - 字符串 `"2*40"` → [`SkillBoost::LastBoost`]（末尾主动技翻倍）
///
/// `skills` 为有序列表，技能在列表中的顺序决定行动时的尝试顺序。
#[derive(Debug, Clone)]
pub struct PlayerOverlay {
    /// 八围属性覆盖值（`[atk, def, spd, agi, mag, res, wis, maxhp]`）。
    /// `None` 表示不覆盖，走正常随机生成。
    pub attrs: Option<[i32; 8]>,
    /// 有序的技能列表：`(技能名, 加成类型和等级)`。
    ///
    /// 列表中的顺序决定行动时的技能尝试顺序（排在前面的先尝试）。
    /// 未列出的技能按默认固定顺序排在末尾。
    /// `None` 表示不覆盖，走正常名字推导技能等级。
    pub skills: Option<Vec<(String, SkillBoost)>>,
    /// 武器名覆盖。
    /// `None` 表示不覆盖，取名字中 `+` 后面的武器名。
    ///
    /// **注意**：DIY 模式下（`attrs` 或 `skills` 不为 `None` 时），武器**不计入**。
    /// 该字段仅在没有八围/技能覆盖时生效。
    pub weapon: Option<String>,
    /// 是否启用 name_factor 缩放。
    ///
    /// - `true`（默认）：八围属性按 name_factor 缩放（正常行为）。
    /// - `false`：强制 name_factor = 0，八围使用原始值不缩放。
    pub name_factor_enabled: bool,
}

impl Default for PlayerOverlay {
    fn default() -> Self {
        Self {
            attrs: None,
            skills: None,
            weapon: None,
            name_factor_enabled: true,
        }
    }
}

impl PlayerOverlay {
    /// 尝试将一个 `+` 后面的段解析为 overlay。
    ///
    /// 如果该段以 `diy[` 开头，按紧凑格式解析；
    /// 如果以 `ol:` 开头，按 JSON 对象格式解析；
    /// 否则返回 `None`，表示该段是普通武器名。
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

    /// 解析紧凑 DIY 格式：`72,39,69,76,67,66,0,84]{"sklfire":5,"reflect":2}`
    ///
    /// 前七围属性值会减去 36 后取非负（`(value - 36).max(0)`），
    /// 与 JS 侧只对索引 0~6 做 `-= 36` 的行为一致。
    /// HP（第 8 项）保持不变。
    fn parse_diy(rest: &str) -> Option<Self> {
        let (attrs_raw, suffix) = rest.split_once(']')?;
        let attrs = parse_attrs(attrs_raw)?;
        let mut overlay = Self {
            // JS 侧的 rand 值范围是 36~127（仅前七围），减 36 得到 0~91 的实际属性值。
            // HP 不减 36，原样保留。
            attrs: Some([
                (attrs[0] - 36).max(0),
                (attrs[1] - 36).max(0),
                (attrs[2] - 36).max(0),
                (attrs[3] - 36).max(0),
                (attrs[4] - 36).max(0),
                (attrs[5] - 36).max(0),
                (attrs[6] - 36).max(0),
                attrs[7], // HP 不减 36
            ]),
            ..Default::default()
        };
        let suffix = suffix.trim();
        if !suffix.is_empty() {
            overlay.skills = Some(parse_skill_map(suffix)?);
        }
        Some(overlay)
    }

    /// 解析 JSON 对象格式：`{"attrs":[1,2,3,4,5,6,7,8],"skills":{"fire":4},"weapon":"剁手刀"}`
    ///
    /// 使用简易手写解析器而非 `serde_json`，避免引入额外依赖。
    /// 支持 `attrs`（八属数组）、`skills`（有序技能列表，顺序决定行动时的尝试顺序）、
    /// `weapon`（武器名字符串）字段。
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
                "attrs" => {
                    let raw = parse_attrs(value)?;
                    // 前七围 -36（兼容 JS 编码范围），HP 不变
                    overlay.attrs = Some([
                        (raw[0] - 36).max(0),
                        (raw[1] - 36).max(0),
                        (raw[2] - 36).max(0),
                        (raw[3] - 36).max(0),
                        (raw[4] - 36).max(0),
                        (raw[5] - 36).max(0),
                        (raw[6] - 36).max(0),
                        raw[7],
                    ]);
                }
                "skills" => overlay.skills = Some(parse_skill_map(value)?),
                "weapon" => overlay.weapon = Some(parse_scalar_string(value)?),
                _ => {}
            }
            idx = next_idx;
        }
        Some(overlay)
    }
}

/// 解析八围属性数组（8 个 i32 值）。
///
/// 支持 `[1,2,3,4,5,6,7,8]` 和 `1,2,3,4,5,6,7,8` 两种形式。
/// 必须恰好解析出 8 个值，否则返回 `None`。
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

/// 解析技能名 → SkillBoost 的有序列表。
///
/// 输入如 `{"sklfire":5,"reflect":"40+30","shadow":"2*46"}`，
/// 返回 `Vec<(String, SkillBoost)>`，保持输入顺序。
/// 键必须是双引号字符串，值可以是整数或双引号字符串。
///
/// 值的解析规则：
/// - 纯数字 `5` → `SkillBoost::Normal(5)`
/// - 字符串 `"40+30"` → `SkillBoost::SlotBoost { base: 40, boost: 30 }`
/// - 字符串 `"2*40"` → `SkillBoost::LastBoost(40)`
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
        // 值可以是整数或双引号字符串
        let value = if bytes.get(idx).copied() == Some(b'"') {
            // 字符串值：解析为 SkillBoost（支持 "40+30"、"2*40" 等格式）
            let (str_val, next_idx) = parse_quoted_string(raw, idx)?;
            idx = next_idx;
            SkillBoost::parse(&str_val)?
        } else {
            // 整数值：直接作为 Normal 等级
            let start = idx;
            while idx < bytes.len() && bytes[idx] != b',' && bytes[idx] != b'}' {
                idx += 1;
            }
            let int_val: u32 = raw[start..idx].trim().parse().ok()?;
            SkillBoost::Normal(int_val)
        };
        list.push((key, value));
    }
    Some(list)
}

/// 解析标量字符串值（用于 ol: 格式中的 weapon 字段）。
///
/// 双引号字符串会做转义解析，否则整个原始串作为值。
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

/// 从 `start` 位置开始提取一个 JSON 值（字符串、数组、对象或标量）。
///
/// 返回 `(值的原始串引用, 下一个未消费的字节索引)`。
/// 数组和对象通过括号平衡来判定边界，字符串通过双引号配对判定。
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
            // 无引号标量值，扫描到下一个逗号为止
            while idx < bytes.len() && bytes[idx] != b',' {
                idx += 1;
            }
            Some((raw[start..idx].trim(), idx))
        }
    }
}

/// 提取一对括号（`open` / `close`）之间的内容，正确处理字符串和转义。
///
/// `start` 指向 `open` 字符本身。返回 `(从 open 到 close 的切片, close 之后的位置)`。
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

/// 解析一个双引号包裹的字符串，处理 `\"` 转义。
///
/// `start` 指向开头的 `"`。返回 `(解析后的字符串内容, 闭合引号之后的位置)`。
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

/// 跳过空白字符。
fn skip_ws(raw: &str, idx: &mut usize) {
    while let Some(ch) = raw[*idx..].chars().next() {
        if ch.is_whitespace() {
            *idx += ch.len_utf8();
        } else {
            break;
        }
    }
}

/// 跳过空白字符和逗号。
fn skip_ws_and_commas(raw: &str, idx: &mut usize) {
    while let Some(ch) = raw[*idx..].chars().next() {
        if ch.is_whitespace() || ch == ',' {
            *idx += ch.len_utf8();
        } else {
            break;
        }
    }
}
