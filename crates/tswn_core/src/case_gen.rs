//! 测试用例生成工具。
//!
//! [`CaseMode`] 枚举描述测试用例的来源方式（文件/内联/随机生成），
//! 以及从各来源加载玩家列表的辅助函数，供 `track_*` 二进制工具使用。

use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CaseMode {
    OneVsOne,
    TwoVsTwo,
    ThreeVsThreeVsThree,
    FreeForAll(usize),
}

impl CaseMode {
    pub fn label(self) -> String {
        match self {
            Self::OneVsOne => "1v1".to_string(),
            Self::TwoVsTwo => "2v2".to_string(),
            Self::ThreeVsThreeVsThree => "3v3v3".to_string(),
            Self::FreeForAll(size) => format!("ffa_{size}"),
        }
    }

    pub fn total_players(self) -> usize {
        match self {
            Self::OneVsOne => 2,
            Self::TwoVsTwo => 4,
            Self::ThreeVsThreeVsThree => 9,
            Self::FreeForAll(size) => size,
        }
    }

    pub fn build_input(self, players: &[String]) -> String {
        match self {
            Self::OneVsOne | Self::FreeForAll(_) => players.join("\n"),
            Self::TwoVsTwo => format!("{}\n\n{}", players[..2].join("\n"), players[2..4].join("\n")),
            Self::ThreeVsThreeVsThree => format!(
                "{}\n\n{}\n\n{}",
                players[..3].join("\n"),
                players[3..6].join("\n"),
                players[6..9].join("\n")
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedCase {
    pub mode: CaseMode,
    pub players: Vec<String>,
    pub input: String,
    pub input_hash: u64,
}

pub fn load_library(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_file() {
        return Err(format!("号库文件不存在: {}", path.display()));
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("读取号库失败: {e}"))?;
    let raw = strip_utf8_bom(&raw);
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            names.push(trimmed.to_string());
        }
    }
    Ok(names)
}

fn strip_utf8_bom(s: &str) -> &str { s.strip_prefix('\u{feff}').unwrap_or(s) }

pub fn deterministic_shuffle(values: &mut [String], seed: u64) {
    if values.len() < 2 {
        return;
    }
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    for idx in (1..values.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let swap_idx = (state % ((idx + 1) as u64)) as usize;
        values.swap(idx, swap_idx);
    }
}

pub fn generate_cases_for_mode(names: &[String], mode: CaseMode, case_offset: usize, max_cases: usize) -> Vec<GeneratedCase> {
    let total = mode.total_players();
    if names.len() < total {
        return Vec::new();
    }

    let mut cases = Vec::new();
    let mut seen_hashes = HashSet::new();
    let mut skipped = 0usize;

    for step in candidate_steps(names.len()) {
        for offset_idx in 0..names.len() {
            if cases.len() >= max_cases {
                return cases;
            }
            let start = (offset_idx * step) % names.len();
            let players = sample_unique_window(names, start, total, step);
            let input = mode.build_input(&players);
            let input_hash = stable_hash(&input);
            if !seen_hashes.insert(input_hash) {
                continue;
            }
            if skipped < case_offset {
                skipped += 1;
                continue;
            }
            cases.push(GeneratedCase {
                mode,
                players,
                input,
                input_hash,
            });
        }
    }

    cases
}

fn candidate_steps(len: usize) -> Vec<usize> {
    let mut steps = Vec::new();
    for candidate in [1usize, 7, 11, 13, 17, 19, 23, 29, 31] {
        if candidate < len && gcd(candidate, len) == 1 {
            steps.push(candidate);
        }
    }
    if steps.is_empty() {
        steps.push(1);
    }
    steps
}

fn gcd(mut lhs: usize, mut rhs: usize) -> usize {
    while rhs != 0 {
        let rem = lhs % rhs;
        lhs = rhs;
        rhs = rem;
    }
    lhs
}

fn sample_unique_window(names: &[String], start: usize, total: usize, step: usize) -> Vec<String> {
    (0..total).map(|idx| names[(start + idx * step) % names.len()].clone()).collect()
}

pub fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub fn case_id(mode: CaseMode, input_hash: u64) -> String { format!("{}-{:016x}", mode.label(), input_hash) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_cases_for_mode_can_expand_beyond_library_len() {
        let names: Vec<String> = (0..80).map(|idx| format!("P{idx}")).collect();
        let cases = generate_cases_for_mode(&names, CaseMode::OneVsOne, 0, 500);

        assert!(cases.len() > names.len());
        assert_eq!(cases.len(), 500);
    }

    #[test]
    fn generated_cases_keep_players_unique_within_case() {
        let names: Vec<String> = (0..80).map(|idx| format!("P{idx}")).collect();
        let cases = generate_cases_for_mode(&names, CaseMode::FreeForAll(8), 0, 128);

        for case in cases {
            let unique: HashSet<_> = case.players.iter().collect();
            assert_eq!(unique.len(), case.players.len(), "duplicate player in {:?}", case.players);
        }
    }

    #[test]
    fn generate_cases_for_mode_offset_matches_stable_slice() {
        let names: Vec<String> = (0..80).map(|idx| format!("P{idx}")).collect();
        let full = generate_cases_for_mode(&names, CaseMode::TwoVsTwo, 0, 40);
        let offset = generate_cases_for_mode(&names, CaseMode::TwoVsTwo, 12, 10);

        let expected: Vec<_> = full.into_iter().skip(12).take(10).map(|case| case.input_hash).collect();
        let actual: Vec<_> = offset.into_iter().map(|case| case.input_hash).collect();
        assert_eq!(actual, expected);
    }
}
