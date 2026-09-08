use crate::{
    GenerateArgs,
    random::{self, Random},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};
use tswn_core::{
    namerena::{PlayerSpec, is_seed_line},
    runtime::RuntimeRunner,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub source: String,
    pub original_input: String,
    pub groups: Vec<Vec<String>>,
    pub matchup_id: String,
}

pub fn matchup_id(groups: &[Vec<String>]) -> String {
    let mut canonical = groups.to_vec();
    for team in &mut canonical {
        team.sort();
    }
    canonical.sort();
    random::hex(&random::digest(&[
        b"matchup-v1",
        &serde_json::to_vec(&canonical).expect("字符串数组可序列化"),
    ]))
}

fn case(source: String, original_input: String) -> Result<Case> {
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(original_input.clone());
    let groups: Vec<Vec<String>> = groups
        .into_iter()
        .map(|team| {
            team.into_iter()
                .filter(|line| !is_seed_line(line))
                .map(|line| Ok(PlayerSpec::parse(&line)?.raw))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|team| team.into_iter().filter(|line| !line.is_empty() && !is_seed_line(line)).collect())
        .filter(|team: &Vec<String>| !team.is_empty())
        .collect();
    ensure!(groups.len() >= 2, "{source}: 至少需要两支非空队伍（空行分队）");
    Ok(Case {
        source,
        original_input,
        matchup_id: matchup_id(&groups),
        groups,
    })
}

fn files(path: &Path, result: &mut Vec<std::path::PathBuf>) -> Result<()> {
    ensure!(
        !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "不跟随输入符号链接：{}",
        path.display()
    );
    if path.is_file() {
        result.push(path.to_owned());
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files(&entry.path(), result)?;
        } else if entry.file_type()?.is_file()
            && entry.path().extension().is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            result.push(entry.path());
        }
    }
    Ok(())
}

pub fn load(args: &GenerateArgs) -> Result<(Vec<Case>, String)> {
    ensure!(
        args.input.is_some() != args.names.is_some(),
        "必须且只能选择 --input 或 --names"
    );
    let mut result = Vec::new();
    let mut source_hashes = Vec::new();
    if let Some(input) = &args.input {
        let mut paths = Vec::new();
        files(input, &mut paths)?;
        paths.sort();
        let mut seen = BTreeSet::new();
        for path in paths {
            let raw = fs::read_to_string(&path).with_context(|| format!("读取 {}", path.display()))?;
            let source = path
                .strip_prefix(input)
                .ok()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            source_hashes.push(random::digest(&[source.as_bytes(), raw.as_bytes()]));
            let case = case(source, raw)?;
            // 完全相同的有序阵容去重；顺序变体仍保留，但共享切分组。
            if seen.insert(case.groups.clone()) {
                result.push(case);
            }
        }
    } else {
        let path = args.names.as_ref().unwrap();
        let raw = fs::read_to_string(path).with_context(|| format!("读取 {}", path.display()))?;
        source_hashes.push(random::digest(&[b"name-pool", raw.as_bytes()]));
        let mut seen = BTreeSet::new();
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        let pool: Vec<_> = normalized
            .lines()
            .filter(|line| !is_seed_line(line))
            .map(|line| Ok(PlayerSpec::parse(line)?.raw))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|line| !line.is_empty() && !is_seed_line(line))
            .filter(|line| seen.insert(line.clone()))
            .collect();
        ensure!(
            args.team_sizes.len() >= 2 && args.team_sizes.iter().all(|size| *size > 0),
            "--team-sizes 需要至少两个正整数"
        );
        let players = args
            .team_sizes
            .iter()
            .try_fold(0usize, |sum, size| sum.checked_add(*size))
            .context("人数溢出")?;
        ensure!(pool.len() >= players, "去重后的名字池人数不足，单局不重复抽取角色");
        let matchups = args.matchups.filter(|count| *count > 0).context("--matchups 必须为正整数")?;
        let pool_hash = random::hex(&random::digest(&[raw.as_bytes()]));
        for index in 0..matchups {
            let mut rng = Random::new(&[b"roster-v1", args.seed.as_bytes(), &(index as u64).to_le_bytes()]);
            let indices = rng.sample_indices(pool.len(), players);
            let mut offset = 0;
            let groups: Vec<Vec<String>> = args
                .team_sizes
                .iter()
                .map(|size| {
                    let team = indices[offset..offset + size].iter().map(|index| pool[*index].clone()).collect();
                    offset += size;
                    team
                })
                .collect();
            result.push(Case {
                source: format!("names:{pool_hash}:{index}"),
                original_input: groups.iter().map(|team| team.join("\n")).collect::<Vec<_>>().join("\n\n"),
                matchup_id: matchup_id(&groups),
                groups,
            });
        }
    }
    ensure!(!result.is_empty(), "未找到可生成的对局");
    for case in &result {
        RuntimeRunner::prepare_groups_with_eval_rq(&case.groups, args.eval_rq)
            .with_context(|| format!("预检 {}", case.source))?;
    }
    let hashes: Vec<_> = source_hashes.iter().map(|hash| hash.as_slice()).collect();
    Ok((result, random::hex(&random::digest(&hashes))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_key_preserves_boundaries_and_ignores_order_and_seed() {
        let first = case("a".into(), "a\nb\n\nc\nseed:one".into()).unwrap();
        let reordered = case("b".into(), "seed:two\nc\n\nb\na".into()).unwrap();
        assert_eq!(first.matchup_id, reordered.matchup_id);
        assert_ne!(first.matchup_id, case("c".into(), "a\n\nb\nc".into()).unwrap().matchup_id);
        assert_ne!(
            matchup_id(&[vec!["a".into()], vec!["b".into()]]),
            matchup_id(&[vec!["a".into(), "a".into()], vec!["b".into()]])
        );
    }

    #[test]
    fn ffa_and_long_ignored_seed_follow_engine_input_rules() {
        let input = case("ffa".into(), format!("a\rb\rseed:{}\r", "x".repeat(1000))).unwrap();
        assert_eq!(input.groups, vec![vec!["a"], vec!["b"]]);
        assert_eq!(input.matchup_id, case("other".into(), "b\n\na\n".into()).unwrap().matchup_id);
    }
}
