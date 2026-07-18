//! Runtime corpus 的冻结 legacy 基线。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tswn_core::runtime::{RuntimeRunner, default_custom_runtime_import_config};

use crate::{EngineAdapter, EventSnapshot, RuntimeEngine, SnapshotKind};

pub const STRESS_GOLDEN_SCHEMA_VERSION: u32 = 1;
pub const EXACT_TRACE_CASE_COUNT: usize = 87;
pub const STRESS_CASE_COUNT: usize = 37;
pub const MAX_GOLDEN_ROUNDS: usize = 20_000;
pub const CANONICAL_DIGEST_FORMAT: &str =
    "sha256-v1(round-index,event-kind,message,caster-name,score,post-round-rc4,winner-team)";

const STRESS_GOLDEN_JSON: &str = include_str!("../cases/runtime_stress/golden.json");

#[derive(Debug, Clone, Copy)]
pub struct StressCaseSpec {
    pub file_name: &'static str,
    pub input: &'static str,
    pub eval_rq: f64,
    pub unescape_x02: bool,
}

impl StressCaseSpec {
    pub fn effective_input(self) -> String {
        if self.unescape_x02 {
            self.input.replace("\\x02", "\x02")
        } else {
            self.input.to_string()
        }
    }
}

macro_rules! stress_case {
    ($file_name:literal) => {
        StressCaseSpec {
            file_name: $file_name,
            input: include_str!(concat!("../cases/runtime_stress/", $file_name)),
            eval_rq: tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ,
            unescape_x02: false,
        }
    };
    ($file_name:literal, eval_rq = $eval_rq:expr) => {
        StressCaseSpec {
            file_name: $file_name,
            input: include_str!(concat!("../cases/runtime_stress/", $file_name)),
            eval_rq: $eval_rq,
            unescape_x02: false,
        }
    };
    ($file_name:literal, unescape_x02) => {
        StressCaseSpec {
            file_name: $file_name,
            input: include_str!(concat!("../cases/runtime_stress/", $file_name)),
            eval_rq: tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ,
            unescape_x02: true,
        }
    };
}

pub const STRESS_CASES: &[StressCaseSpec] = &[
    stress_case!("1v1-0f92cb76cc37fdc5.txt"),
    stress_case!("2v2-554f4128af707167.txt"),
    stress_case!("2v2-a47d6d806a77e593.txt"),
    stress_case!("3v3v3-0ace5df17b84e26a.txt"),
    stress_case!("3v3v3-0f20f33db97a2e10.txt"),
    stress_case!("3v3v3-35070658d93637e1.txt"),
    stress_case!("3v3v3-3d4a0f9a1de8fa32.txt"),
    stress_case!("3v3v3-42186ce5db28b886.txt"),
    stress_case!("3v3v3-59f9ed82b34e7760.txt"),
    stress_case!("3v3v3-5e1dd340fc7876ac.txt"),
    stress_case!("3v3v3-6c9aabe2aa79d92f.txt"),
    stress_case!("3v3v3-711d7e09df7b7da3.txt"),
    stress_case!("3v3v3-7dfb2a54a1cd12d7.txt"),
    stress_case!("3v3v3-908ecb326213b7aa.txt"),
    stress_case!("3v3v3-947fbfff2b995a89.txt"),
    stress_case!("3v3v3-d6dc36b3d836cf28.txt"),
    stress_case!("3v3v3-db3944e8cf92d8d9.txt"),
    stress_case!("3v3v3-e1541883a7613633.txt"),
    stress_case!("3v3v3-ea03011602664398.txt"),
    stress_case!("cqd-p06-t26-r0135.txt"),
    stress_case!("cqd-p19-t26-r0336.txt"),
    stress_case!("cqd-p21-t31-r5997-guard.txt"),
    stress_case!("cqd-p21-t39-r0107.txt"),
    stress_case!("cqd-p28-t26-r0447.txt"),
    stress_case!("ffa_4-37fc802e0ef650a3.txt"),
    stress_case!("ffa_4-4cdab500cdcdd9bf.txt"),
    stress_case!("ffa_4-b9ba9b639670ceb3.txt"),
    stress_case!("ffa_6-3ae9dd4b7b788849.txt"),
    stress_case!("ffa_6-9af1d802c0fc1ad4.txt"),
    stress_case!("ffa_8-16d11de1ebe1df41.txt"),
    stress_case!("ffa_8-3d155c15a9d6ec4b.txt"),
    stress_case!("ffa_8-76f0580bd07d1405.txt"),
    stress_case!("ffa_8-b9a9c7882f4f1f99.txt"),
    stress_case!("ffa_8-cce83cebc69761de.txt"),
    stress_case!("ffa_8-fee2c41a2508ad16.txt"),
    stress_case!("score_round_7.txt", eval_rq = tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ),
    stress_case!("score-mario-r11350.txt", unescape_x02),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rc4Golden {
    pub i: u64,
    pub j: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressCaseGolden {
    pub file_name: String,
    pub input_sha256: String,
    pub eval_rq: f64,
    pub winner_team_index: usize,
    pub rounds: usize,
    pub total_score: u64,
    pub final_rc4: Rc4Golden,
    pub round_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressGoldenSet {
    pub schema_version: u32,
    pub exact_trace_case_count: usize,
    pub stress_case_count: usize,
    pub canonical_digest_format: String,
    pub cases: Vec<StressCaseGolden>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing into String cannot fail");
    }
    output
}

fn input_sha256_hex(raw: &str) -> String {
    if raw.as_bytes().contains(&b'\r') {
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        sha256_hex(normalized.as_bytes())
    } else {
        sha256_hex(raw.as_bytes())
    }
}

fn feed_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn snapshot_kind_tag(kind: SnapshotKind) -> u8 {
    match kind {
        SnapshotKind::Event => 0,
        SnapshotKind::NextLine => 1,
        SnapshotKind::Win => 2,
    }
}

fn canonical_round_digest(
    round_index: usize,
    events: &[EventSnapshot],
    rc4: Option<(usize, usize)>,
    winner_team_index: Option<usize>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tswn-runtime-stress-round\0v1\0");
    hasher.update((round_index as u64).to_le_bytes());
    hasher.update((events.len() as u64).to_le_bytes());
    for event in events {
        hasher.update([snapshot_kind_tag(event.kind)]);
        feed_string(&mut hasher, &event.message);
        feed_string(&mut hasher, &event.caster_name);
        hasher.update(event.score.to_le_bytes());
    }
    match rc4 {
        Some((i, j)) => {
            hasher.update([1]);
            hasher.update((i as u64).to_le_bytes());
            hasher.update((j as u64).to_le_bytes());
        }
        None => hasher.update([0]),
    }
    match winner_team_index {
        Some(team) => {
            hasher.update([1]);
            hasher.update((team as u64).to_le_bytes());
        }
        None => hasher.update([0]),
    }
    sha256_hex(&hasher.finalize())
}

fn capture_case<E: EngineAdapter>(mut runner: E::Runner, spec: StressCaseSpec) -> StressCaseGolden {
    let input = spec.effective_input();
    let mut round_digests = Vec::new();
    let mut total_score = 0u64;

    while !E::have_winner(&runner) && round_digests.len() < MAX_GOLDEN_ROUNDS {
        let events = E::main_round(&mut runner);
        total_score += events
            .iter()
            .filter(|event| !matches!(event.kind, SnapshotKind::NextLine))
            .map(|event| event.score as u64)
            .sum::<u64>();
        round_digests.push(canonical_round_digest(
            round_digests.len(),
            &events,
            E::rc4_state(&runner),
            E::winner_team_index(&runner),
        ));
    }

    assert!(
        round_digests.len() < MAX_GOLDEN_ROUNDS,
        "{} did not finish within {MAX_GOLDEN_ROUNDS} rounds",
        spec.file_name
    );
    let winner_team_index =
        E::winner_team_index(&runner).unwrap_or_else(|| panic!("{} finished without a winner team", spec.file_name));
    let (i, j) = E::rc4_state(&runner).unwrap_or_else(|| panic!("{} did not expose final RC4 state", spec.file_name));

    StressCaseGolden {
        file_name: spec.file_name.to_string(),
        input_sha256: input_sha256_hex(&input),
        eval_rq: spec.eval_rq,
        winner_team_index,
        rounds: round_digests.len(),
        total_score,
        final_rc4: Rc4Golden {
            i: i as u64,
            j: j as u64,
        },
        round_digests,
    }
}

fn build_runtime_runner(spec: StressCaseSpec) -> RuntimeRunner {
    let config = default_custom_runtime_import_config()
        .unwrap_or_else(|error| panic!("Runtime golden profile failed to build: {error:?}"));
    RuntimeRunner::from_custom_mixed_namerena_raw_with_eval_rq(spec.effective_input(), spec.eval_rq, config)
        .unwrap_or_else(|error| panic!("Runtime golden input {} failed to build: {error:?}", spec.file_name))
}

fn parse_frozen_goldens() -> StressGoldenSet {
    serde_json::from_str(STRESS_GOLDEN_JSON).expect("frozen Runtime stress golden JSON must be valid")
}

fn assert_inventory(goldens: &StressGoldenSet) {
    assert_eq!(
        goldens.schema_version, STRESS_GOLDEN_SCHEMA_VERSION,
        "stress golden schema changed"
    );
    assert_eq!(
        goldens.exact_trace_case_count, EXACT_TRACE_CASE_COUNT,
        "JS exact trace inventory changed"
    );
    assert_eq!(goldens.stress_case_count, STRESS_CASE_COUNT, "stress golden inventory changed");
    assert_eq!(goldens.cases.len(), STRESS_CASE_COUNT, "stress golden case count changed");
    assert_eq!(STRESS_CASES.len(), STRESS_CASE_COUNT, "compiled stress case inventory changed");
    assert_eq!(goldens.canonical_digest_format, CANONICAL_DIGEST_FORMAT);

    let golden_files = goldens.cases.iter().map(|case| case.file_name.as_str()).collect::<HashSet<_>>();
    let compiled_files = STRESS_CASES.iter().map(|case| case.file_name).collect::<HashSet<_>>();
    assert_eq!(
        golden_files.len(),
        STRESS_CASE_COUNT,
        "stress golden contains duplicate file names"
    );
    assert_eq!(
        compiled_files.len(),
        STRESS_CASE_COUNT,
        "compiled stress inventory contains duplicate file names"
    );
    assert_eq!(
        golden_files, compiled_files,
        "stress golden and compiled input inventories differ"
    );
}

fn find_spec_by_input(raw: &str, eval_rq: f64) -> StressCaseSpec {
    let input_sha256 = input_sha256_hex(raw);
    STRESS_CASES
        .iter()
        .copied()
        .find(|spec| spec.eval_rq == eval_rq && input_sha256_hex(&spec.effective_input()) == input_sha256)
        .unwrap_or_else(|| panic!("stress input {input_sha256} with eval_rq={eval_rq} is not frozen"))
}

pub fn assert_runtime_matches_frozen_golden(raw: &str, case_name: &str) {
    assert_runtime_matches_frozen_golden_with_eval_rq(raw, case_name, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ);
}

pub fn assert_runtime_matches_frozen_golden_with_eval_rq(raw: &str, case_name: &str, eval_rq: f64) {
    let goldens = parse_frozen_goldens();
    assert_inventory(&goldens);
    let spec = find_spec_by_input(raw, eval_rq);
    let expected = goldens
        .cases
        .iter()
        .find(|case| case.file_name == spec.file_name)
        .unwrap_or_else(|| panic!("{case_name}: missing frozen golden for {}", spec.file_name));
    assert_eq!(
        expected.input_sha256,
        input_sha256_hex(raw),
        "{case_name}: input SHA-256 changed"
    );
    assert_eq!(expected.eval_rq, eval_rq, "{case_name}: eval_rq changed");

    let actual = capture_case::<RuntimeEngine>(build_runtime_runner(spec), spec);
    assert_eq!(
        actual.winner_team_index, expected.winner_team_index,
        "{case_name}: winner team changed"
    );
    assert_eq!(actual.rounds, expected.rounds, "{case_name}: round count changed");
    assert_eq!(actual.total_score, expected.total_score, "{case_name}: total score changed");
    assert_eq!(actual.final_rc4, expected.final_rc4, "{case_name}: final RC4 changed");
    if actual.round_digests != expected.round_digests {
        let mismatch = actual
            .round_digests
            .iter()
            .zip(expected.round_digests.iter())
            .position(|(actual, expected)| actual != expected)
            .unwrap_or_else(|| actual.round_digests.len().min(expected.round_digests.len()));
        panic!(
            "{case_name}: canonical round digest changed at round {mismatch}: actual={:?}, expected={:?}",
            actual.round_digests.get(mismatch),
            expected.round_digests.get(mismatch),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::input_sha256_hex;

    #[test]
    fn input_hash_normalizes_platform_line_endings() {
        let expected = input_sha256_hex("alpha\nbeta\n");
        assert_eq!(input_sha256_hex("alpha\r\nbeta\r\n"), expected);
        assert_eq!(input_sha256_hex("alpha\rbeta\r"), expected);
    }
}
