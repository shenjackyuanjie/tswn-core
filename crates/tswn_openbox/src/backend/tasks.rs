//! 后端任务实现。
//!
//! 实现各工具的核心计算逻辑（`run_to_diy`、`run_namer_pf`、`run_batch_rate`、`run_pair`），
//! 通过回调向 GUI 收件箱或 CLI 通道推送进度日志和最终结果。

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use tswn_core::bench_sched::{low_accuracy_outer_workers, run_outer_parallel_ordered};
use tswn_core::cli_api;
use tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::namerena::{BuiltinSkillRef, NamerenaInput, PreparedPlayer, PreparedRoster};

use super::format::{format_pair_file_record, format_pair_screen_log, format_rate};
use super::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::{create_output_file, finalize_sorted_output_file};
use super::pair::{PairMatrixInput, run_pair_matrix, run_pair_matrix_observed};
use super::parse::{
    parse_factored_target_groups, parse_line_list, parse_namer_pf_groups, parse_player_groups_with_labels, parse_target_groups,
};
#[cfg(test)]
use super::score::bench_batch_rate_for_group;
use super::score::namer_pf_score;
use super::skill_board::{SkillBoardConfig, SkillBoardLine, evaluate_skill_board};
use super::types::{NamerPfInput, NamerPfMetric, NamerPfMetricOptions, PairInput, ProgressEvent};

/// 导出 `to-diy` 结果。
///
/// `details` 只影响日志：每一行都按 `+` 语义整行构队，再把这一行里的每个玩家各输出
/// 一个“原始信息”详情块（组队行也输出，单玩家行就是一个块）。输出文件内容、
/// `--old` / `--minions` 导出格式都不受它影响。
pub fn run_to_diy(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String, String> {
    run_to_diy_observed(raw, old, minions, details, output_file, cancel, None)
}

#[allow(clippy::too_many_arguments)]
pub fn run_to_diy_observed(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
    observer: ResultObserver<'_>,
) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    // 详情属于日志产物：选了输出文件时只写导出行，避免把详情混进文件。
    let details = details && output_file.is_none();
    let needs_text = observer.is_none() || output_file.is_some();
    let mut out = String::new();
    for (index, name) in names.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Ok("已停止。".to_string());
        }
        // 每行结果之间空一行：否则上一行的详情块会和下一行的导出行贴在一起。
        if needs_text && !out.is_empty() {
            let _ = writeln!(out);
        }
        let mut update = observer.map(|_| ResultUpdate::new(index, name, ResultKind::Diy, 0));
        let export = cli_api::to_diy(name, old, minions).map_err(|err| format!("导出 DIY 失败: {name}: {err}"))?;
        if needs_text {
            let _ = writeln!(out, "{export}");
        }
        if let Some(update) = &mut update {
            update.entries.push(ResultEntry {
                index: 0,
                label: "导出".into(),
                value: None,
                text: export,
                kind: EntryKind::Plain,
            });
        }

        if details {
            for detail in to_diy_details(name)? {
                if needs_text {
                    let _ = writeln!(out);
                    append_to_diy_details(&mut out, &detail);
                }
                if let Some(update) = &mut update {
                    for (attr_index, label) in ["攻", "防", "速", "敏", "魔", "抗", "智", "HP"].iter().enumerate() {
                        let delta = i64::from(detail.attrs[attr_index]) - i64::from(detail.solo_attrs[attr_index]);
                        update.entries.push(ResultEntry {
                            index: update.entries.len(),
                            label: format!("{} · {label}", detail.name),
                            value: Some(f64::from(detail.attrs[attr_index])),
                            text: format_delta(i64::from(detail.attrs[attr_index]), i64::from(detail.solo_attrs[attr_index])),
                            kind: if delta != 0 { EntryKind::Highlight } else { EntryKind::Plain },
                        });
                    }
                    update.entries.push(ResultEntry {
                        index: update.entries.len(),
                        label: format!("{} · 八围", detail.name),
                        value: None,
                        text: format_eight_ring(detail.attrs),
                        kind: EntryKind::Plain,
                    });
                    update.entries.push(ResultEntry::number(
                        update.entries.len(),
                        format!("{} · 嘲讽", detail.name),
                        taunt_value(detail.attrs) as f64,
                    ));
                    for skill in &detail.skills {
                        update.entries.push(ResultEntry {
                            index: update.entries.len(),
                            label: format!("{} · {}", detail.name, skill.name),
                            value: Some(f64::from(skill.level)),
                            text: format_delta(i64::from(skill.level), i64::from(skill.level) - skill.delta),
                            kind: EntryKind::Plain,
                        });
                    }
                }
            }
        }
        if let (Some(observer), Some(mut update)) = (observer, update) {
            update.finish = Some(ResultFinish {
                score: None,
                visible: true,
                highlight: false,
            });
            observer(update);
        }
    }

    let result = finish_output(output_file.as_deref(), out)?;
    Ok(if observer.is_some() && output_file.is_none() {
        "完成。".to_owned()
    } else {
        result
    })
}

/// 详情块里的一行技能：中文名、整队后的熟练度，以及相对单独构建的变化量。
#[derive(Debug)]
struct ToDiySkill {
    name: &'static str,
    level: u32,
    delta: i64,
}

/// 一个玩家的详情块内容。
///
/// `attrs` / `skills` 取自本行整队构建的结果（与导出行一致）；`solo_attrs` 是同一个
/// 成员单独构建的结果，用来标出“组队后与原属性”的差额。
struct ToDiyDetails {
    name: String,
    attrs: [u32; 8],
    solo_attrs: [u32; 8],
    skills: Vec<ToDiySkill>,
}

/// 为一行输入构建详情块：整行按 `+` 组成一队后再逐个玩家取值。
///
/// 必须按整行构队而不是单独构建某个名字：同队成员之间会互相升级属性
/// （例如 `1@team` 单独构队 HP 243，和 `2@team` 同队后是 245），
/// 只有整行构队才能和导出行逐项对齐。每个成员再各自单独构建一次（同 overlay、
/// 只是没有队友），两者之差就是该成员的“组队加成”，用来输出 `(+N)`。
fn to_diy_details(name: &str) -> Result<Vec<ToDiyDetails>, String> {
    let groups = parse_namer_pf_groups(name);
    let Some(group) = groups.first().filter(|group| !group.is_empty()) else {
        return Ok(Vec::new());
    };
    let group_input = NamerenaInput::from_raw_groups(std::slice::from_ref(group))
        .map_err(|err| format!("构建玩家失败: {}: {err}", group.join("+")))?;
    let roster = match PreparedRoster::build(&group_input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    let mut details = Vec::with_capacity(roster.players.len());
    for player in &roster.players {
        // `player.id` 就是该成员在本行 group 里的下标：单独构建时保留原字符串
        // （overlay 等后缀都在里面），只是没有队友。
        let solo_source = group.get(player.id).cloned().unwrap_or_else(|| player.id_key_name.clone());
        let solo = solo_player_of(&solo_source).map_err(|err| format!("构建玩家失败: {solo_source}: {err}"))?;
        let solo_levels = skill_levels(&solo);
        details.push(ToDiyDetails {
            name: player.id_key_name.clone(),
            attrs: export_attrs(player.attrs),
            solo_attrs: export_attrs(solo.attrs),
            skills: action_order_skills(player, &solo_levels),
        });
    }
    Ok(details)
}

/// 单独构建一个成员（保留它的 overlay 等后缀），返回它的玩家数据。
fn solo_player_of(raw: &str) -> Result<PreparedPlayer, String> {
    let input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]]).map_err(|err| err.to_string())?;
    let roster = match PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    roster.players.into_iter().next().ok_or_else(|| "无有效玩家".to_string())
}

/// 玩家实际生效的技能等级：key 是导出名（`skl*` / `summon:skl*` / `phantom:skl*`），
/// 与 `+ol` 导出用的是同一套命名，因此两次构建可以直接对齐。
fn skill_levels(player: &PreparedPlayer) -> Vec<(String, u32)> {
    player
        .skills
        .active_order
        .iter()
        .filter_map(|key| {
            let entry = player.skills.entries.iter().find(|entry| entry.key == *key && entry.level > 0)?;
            if entry.skill == BuiltinSkillRef::SummonShareDamage {
                return None;
            }
            Some((export_skill_name(entry), entry.level))
        })
        .collect()
}

/// 详情块的技能行：逐项与单独构建的同名技能比差额。
fn action_order_skills(player: &PreparedPlayer, solo_levels: &[(String, u32)]) -> Vec<ToDiySkill> {
    let mut skills = Vec::new();
    for (key, level) in skill_levels(player) {
        let Some(name) = cn_skill_name(&key) else {
            continue;
        };
        let solo = solo_levels
            .iter()
            .find_map(|(solo_key, solo_level)| (solo_key == &key).then_some(i64::from(*solo_level)))
            .unwrap_or(0);
        skills.push(ToDiySkill {
            name,
            level,
            delta: i64::from(level) - solo,
        });
    }
    skills
}

/// 导出名到中文名：只认技能榜同一张表里的常规技能。
fn cn_skill_name(export_name: &str) -> Option<&'static str> {
    let id = tswn_core::namerena::skill_name_to_id(export_name)?;
    SKILL_CN_NAMES.get(id).copied()
}

/// 技能导出名，与 `cli_api` 的 `+ol` 导出（`skill_export_name` 的 Player 分支）一致。
fn export_skill_name(entry: &tswn_core::namerena::SkillEntrySpec) -> String {
    tswn_core::namerena::classified_player_skill_name_for_export(entry.key).unwrap_or_else(|| match entry.skill {
        BuiltinSkillRef::Normal(skill_id) => tswn_core::namerena::skill_name_for_export(skill_id),
        _ => entry.key.to_string(),
    })
}

/// 把内部属性换算成导出口径：前七围 +36（与 `attrs_to_overlay_json` 一致），HP 原样。
fn export_attrs(attrs: [u32; 8]) -> [u32; 8] {
    let mut out = attrs;
    for value in &mut out[..7] {
        *value += 36;
    }
    out
}

/// 追加“单名详情”块。
///
/// 块内只描述构建后的玩家本体：名字、HP 与七围、八围、嘲讽，以及按行动顺序排列的
/// 技能行。属性与技能都按“组队后相对单独构建”的差额标注 `(+N)` / `(-N)`。
/// 整块只走日志，不写入输出文件。
fn append_to_diy_details(out: &mut String, details: &ToDiyDetails) {
    let attrs = details.attrs;
    let _ = writeln!(out, "=== 原始信息 ===");
    let _ = writeln!(out, "{}", details.name);
    let _ = writeln!(
        out,
        "HP {} 攻 {} 防 {} 速 {} 敏 {} 魔 {} 抗 {} 智 {} 八围 {} 嘲讽{}",
        format_delta(i64::from(attrs[7]), i64::from(details.solo_attrs[7])),
        format_delta(i64::from(attrs[0]), i64::from(details.solo_attrs[0])),
        format_delta(i64::from(attrs[1]), i64::from(details.solo_attrs[1])),
        format_delta(i64::from(attrs[2]), i64::from(details.solo_attrs[2])),
        format_delta(i64::from(attrs[3]), i64::from(details.solo_attrs[3])),
        format_delta(i64::from(attrs[4]), i64::from(details.solo_attrs[4])),
        format_delta(i64::from(attrs[5]), i64::from(details.solo_attrs[5])),
        format_delta(i64::from(attrs[6]), i64::from(details.solo_attrs[6])),
        format_eight_ring(attrs),
        taunt_value(attrs),
    );
    for skill in &details.skills {
        let solo = i64::from(skill.level) - skill.delta;
        let _ = writeln!(out, "  {} {}", skill.name, format_delta(i64::from(skill.level), solo));
    }
}

/// 一个数值：整队后的值，与单独构建有差额时追加 `(+N)` / `(-N)`。
fn format_delta(teamed: i64, solo: i64) -> String {
    match teamed.cmp(&solo) {
        std::cmp::Ordering::Equal => teamed.to_string(),
        std::cmp::Ordering::Greater => format!("{teamed}(+{})", teamed - solo),
        std::cmp::Ordering::Less => format!("{teamed}(-{})", solo - teamed),
    }
}

/// 八围 = 七围之和 + HP / 3，四舍五入到一位小数。
///
/// 用整数定点取到 0.1：`(total * 10 + 1) / 3`，其中 `total = 七围之和 * 3 + HP`
/// 是 `(七围之和 + HP / 3)` 的 3 倍；除数 3 用 `+1` 做四舍五入。全程整数，
/// 避免浮点格式化在边界上的抖动。
fn format_eight_ring(attrs: [u32; 8]) -> String {
    let seven = attrs[..7].iter().map(|value| u64::from(*value)).sum::<u64>();
    let total = seven * 3 + u64::from(attrs[7]);
    let tenths = (total * 10 + 1) / 3;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// 嘲讽值 = 防 * 2 + 抗 * 2 - 攻 * 2 - 魔 * 2 - 速 * 2 - 敏 - 智。
///
/// 公式本身给出负数（属性越强嘲讽越低），面板按数值大小显示，这里取绝对值。
fn taunt_value(attrs: [u32; 8]) -> u64 {
    let read = |index: usize| i64::from(attrs[index]);
    let raw = read(1) * 2 + read(5) * 2 - read(0) * 2 - read(4) * 2 - read(2) * 2 - read(3) - read(6);
    raw.unsigned_abs()
}

/// 技能 id 到中文名的对照表，顺序与 `skill_name_to_id` 一一对应：
/// 火球 冰冻 雷击 地裂 吸血 投毒 连击 会心 瘟疫 命轮 狂暴 魅惑 加速 减速 诅咒
/// 治愈 苏生 净化 铁壁 蓄力 聚气 潜行 血祭 分身 幻术 防御 守护 反弹 护符 护盾
/// 反击 吞噬 召灵 垂死 隐匿
const SKILL_CN_NAMES: [&str; 35] = [
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击", "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速", "诅咒",
    "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气", "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹", "护符", "护盾",
    "反击", "吞噬", "召灵", "垂死", "隐匿",
];

pub fn run_namer_pf(input: NamerPfInput, send: impl Fn(ProgressEvent)) { run_namer_pf_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_namer_pf_observed(input: NamerPfInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    let groups = parse_namer_pf_groups(&input.raw);
    if groups.is_empty() {
        send(ProgressEvent::Done(Err("namer-pf: 输入为空或无有效玩家。".to_string())));
        return;
    }
    if input.metrics.iter().all(|metric| !metric.screen && metric.output_file.is_none())
        && !input.skill_board.screen
        && input.skill_board.output_file.is_none()
    {
        send(ProgressEvent::Done(Err(
            "namer-pf: 请至少选择一个屏幕输出或输出文件。".to_string()
        )));
        return;
    }

    let mut outputs = Vec::with_capacity(input.metrics.len());
    for metric in &input.metrics {
        let output = match metric.output_file.as_deref() {
            Some(path) => match create_output_file(path) {
                Ok(file) => Some(file),
                Err(err) => {
                    send(ProgressEvent::Done(Err(err)));
                    return;
                }
            },
            None => None,
        };
        outputs.push(output);
    }
    let mut skill_board_output = match input.skill_board.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    let skill_board_config = if input.skill_board.screen || skill_board_output.is_some() {
        // GUI 不传 config，按 ./setting/score_now.toml 惯例加载；CLI 可显式指定。
        let loaded = match input.skill_board.config.as_deref() {
            Some(path) => SkillBoardConfig::load(path),
            None => SkillBoardConfig::load_default(),
        };
        match loaded {
            Ok(config) => Some(config),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        None
    };

    let n = input.count.max(1);
    let eval_rq = eval_rq(input.keep_rq);
    let precision = input.precision.min(9);
    let total = groups.len();
    let needs_skill_board_scores = skill_board_config.is_some();
    let needs_sum = metric_enabled(&input.metrics, NamerPfMetric::Sum);
    let needs_all_scores = needs_skill_board_scores || needs_sum;
    let needs_pp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pp);
    let needs_pd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pd);
    let needs_qp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qp);
    let needs_qd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qd);

    let score_settings = NamerPfScoreSettings {
        n,
        threads: input.threads,
        eval_rq,
        needs_pp,
        needs_pd,
        needs_qp,
        needs_qd,
    };
    let compute = |index: usize, group: &[String], settings: NamerPfScoreSettings| {
        let label = group.join("+");
        let mut visible = false;
        let mut result = compute_namer_pf_result(group, settings, |metric, value| {
            let Some(observer) = observer else { return };
            let Some(options) = input.metrics.iter().find(|option| option.metric == metric && option.screen) else {
                return;
            };
            if options.min_screen.is_some_and(|min| value < min) {
                return;
            }
            visible = true;
            let mut update = ResultUpdate::new(index, &label, ResultKind::Scores, precision);
            let mut entry = ResultEntry::number(
                NamerPfMetric::ALL.iter().position(|m| *m == metric).unwrap(),
                metric.label().to_owned(),
                value,
            );
            if should_highlight(value, options.min_screen, options.highlight_delta) {
                entry.kind = EntryKind::Highlight;
            }
            update.entries.push(entry);
            observer(update);
        });
        if let Some(config) = skill_board_config.as_ref() {
            result.skill_lines = evaluate_skill_board(group, &result.scores, config);
        }
        if let Some(observer) = observer {
            let mut update = ResultUpdate::new(index, &label, ResultKind::Scores, precision);
            if input.skill_board.screen {
                for (i, line) in result.skill_lines.iter().enumerate() {
                    let mut entry = ResultEntry::number(5 + i, line.title.clone(), line.score);
                    entry.kind = EntryKind::SkillBoard;
                    update.entries.push(entry);
                    visible = true;
                }
            }
            update.finish = Some(ResultFinish {
                score: None,
                visible,
                highlight: false,
            });
            observer(update);
        }
        result
    };
    let outer_workers = low_accuracy_outer_workers(n, total, outer_thread_spec(input.threads));
    let completed = if outer_workers > 1 {
        let score_settings = score_settings.with_threads(Some(1));
        let mut progress_done = 0usize;
        match run_outer_parallel_ordered(
            &groups,
            outer_workers,
            &input.cancel,
            |index, group, tick| {
                let result = compute(index, group, score_settings);
                tick();
                result
            },
            || {
                progress_done += 1;
                send(ProgressEvent::Progress {
                    done: progress_done,
                    total,
                });
            },
            |result| {
                emit_namer_pf_result(
                    &result,
                    &input.metrics,
                    &mut outputs,
                    SkillBoardEmitCfg {
                        config: skill_board_config.as_ref(),
                        output: &mut skill_board_output,
                        screen: input.skill_board.screen,
                    },
                    precision,
                    observer.is_none().then_some(&send as &dyn Fn(ProgressEvent)),
                )
            },
        ) {
            Ok(done) => done,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        let mut completed = 0usize;
        for (index, group) in groups.iter().enumerate() {
            if input.cancel.load(Ordering::Relaxed) {
                send(ProgressEvent::Done(Ok("已停止。".to_string())));
                return;
            }
            let result = compute(index, group, score_settings);
            if let Err(err) = emit_namer_pf_result(
                &result,
                &input.metrics,
                &mut outputs,
                SkillBoardEmitCfg {
                    config: skill_board_config.as_ref(),
                    output: &mut skill_board_output,
                    screen: input.skill_board.screen,
                },
                precision,
                observer.is_none().then_some(&send as &dyn Fn(ProgressEvent)),
            ) {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
            completed = index + 1;
            send(ProgressEvent::Progress { done: completed, total });
        }
        completed
    };

    if input.cancel.load(Ordering::Relaxed) && completed < total {
        send(ProgressEvent::Done(Ok("已停止。".to_string())));
        return;
    }

    let mut written = input
        .metrics
        .iter()
        .filter_map(|metric| {
            metric
                .output_file
                .as_ref()
                .map(|path| format!("{} -> {}", metric.metric.label(), path.display()))
        })
        .collect::<Vec<_>>();
    if let Some(path) = input.skill_board.output_file.as_ref() {
        written.push(format!("技能榜 -> {}", path.display()));
    }
    let message = if written.is_empty() {
        "完成。".to_string()
    } else {
        format!("完成，结果已写入: {}", written.join("; "))
    };
    send(ProgressEvent::Done(Ok(message)));
}

fn metric_enabled(metrics: &[super::types::NamerPfMetricOptions], metric: NamerPfMetric) -> bool {
    metrics
        .iter()
        .any(|options| options.metric == metric && (options.screen || options.output_file.is_some()))
}

#[derive(Clone, Copy)]
struct NamerPfScoreSettings {
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    needs_pp: bool,
    needs_pd: bool,
    needs_qp: bool,
    needs_qd: bool,
}

impl NamerPfScoreSettings {
    fn with_threads(self, threads: Option<usize>) -> Self { Self { threads, ..self } }
}

struct NamerPfJobResult {
    label: String,
    scores: NamerPfScores,
    skill_lines: Vec<SkillBoardLine>,
}

fn compute_namer_pf_result(
    group: &[String],
    settings: NamerPfScoreSettings,
    mut metric_done: impl FnMut(NamerPfMetric, f64),
) -> NamerPfJobResult {
    let pp = if settings.needs_pp {
        namer_pf_score(group, "\u{0002}", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_pp {
        metric_done(NamerPfMetric::Pp, pp);
    }
    let pd = if settings.needs_pd {
        namer_pf_score(group, "\u{0002}", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_pd {
        metric_done(NamerPfMetric::Pd, pd);
    }
    let qp = if settings.needs_qp {
        namer_pf_score(group, "!", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_qp {
        metric_done(NamerPfMetric::Qp, qp);
    }
    let qd = if settings.needs_qd {
        namer_pf_score(group, "!", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_qd {
        metric_done(NamerPfMetric::Qd, qd);
    }
    let scores = NamerPfScores {
        pp,
        pd,
        qp,
        qd,
        sum: pp + pd + qp + qd,
    };
    metric_done(NamerPfMetric::Sum, scores.sum);
    NamerPfJobResult {
        label: group.join("+"),
        scores,
        skill_lines: Vec::new(),
    }
}

struct SkillBoardEmitCfg<'a> {
    config: Option<&'a SkillBoardConfig>,
    output: &'a mut Option<File>,
    screen: bool,
}

fn emit_namer_pf_result(
    result: &NamerPfJobResult,
    metrics: &[NamerPfMetricOptions],
    outputs: &mut [Option<File>],
    skill_board: SkillBoardEmitCfg<'_>,
    precision: usize,
    send: Option<&dyn Fn(ProgressEvent)>,
) -> Result<(), String> {
    for (metric, output) in metrics.iter().zip(outputs.iter_mut()) {
        let score = result.scores.get(metric.metric);
        if let Some(send) = send.filter(|_| metric.screen && metric.min_screen.is_none_or(|limit| score >= limit)) {
            let score_text = format_rate(score, precision);
            let line = format!("{} {}:{}", result.label, metric.metric.label(), score_text);
            if should_highlight(score, metric.min_screen, metric.highlight_delta) {
                send(ProgressEvent::HighlightLog(line));
            } else {
                send(ProgressEvent::Log(line));
            }
        }
        if metric.min_file.is_none_or(|limit| score >= limit)
            && let Some(output) = output.as_mut()
            && let Err(err) = writeln!(output, "{} {}", format_rate(score, precision), result.label)
        {
            return Err(format!("写入输出文件失败: {err}"));
        }
    }
    if skill_board.config.is_some() {
        for line in &result.skill_lines {
            let score_text = format_rate(line.score, precision);
            if skill_board.screen
                && let Some(send) = send
            {
                send(ProgressEvent::SkillBoardLog(format!(
                    "{} {} {}",
                    line.title, score_text, result.label
                )));
            }
            if let Some(output) = skill_board.output.as_mut()
                && let Err(err) = writeln!(output, "{} {} {}", line.title, score_text, result.label)
            {
                return Err(format!("写入输出文件失败: {err}"));
            }
        }
    }
    Ok(())
}

/// 把 GUI 侧的 `Option<usize>` 线程设置转换成 [`low_accuracy_outer_workers`] 需要的 `thread` 语义。
pub(super) fn outer_thread_spec(threads: Option<usize>) -> u32 { threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0) }

#[derive(Debug, Clone, Copy)]
pub struct NamerPfScores {
    pub pp: f64,
    pub pd: f64,
    pub qp: f64,
    pub qd: f64,
    pub sum: f64,
}

impl NamerPfScores {
    fn get(&self, metric: NamerPfMetric) -> f64 {
        match metric {
            NamerPfMetric::Pp => self.pp,
            NamerPfMetric::Pd => self.pd,
            NamerPfMetric::Qp => self.qp,
            NamerPfMetric::Qd => self.qd,
            NamerPfMetric::Sum => self.sum,
        }
    }
}

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) { run_pair_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_pair_observed(input: PairInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    let (target_groups, target_factors) = match parse_pair_target_groups(&input.target_text, input.target_factor_enabled) {
        Ok(targets) => targets,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    let (teammate_groups, teammate_labels, teammate_factors) =
        match parse_pair_teammate_groups(&input.teammate_text, input.teammate_double_plus, input.teammate_factor_enabled) {
            Ok(value) => value,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        };
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 靶子列表为空。".to_string())));
        return;
    }
    if player_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 选手列表为空。".to_string())));
        return;
    }
    if teammate_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 队友列表为空。".to_string())));
        return;
    }

    let mut output = match input.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    if output.is_none() {
        send(ProgressEvent::Log("未选择输出文件，本次只输出到日志。".to_string()));
    }

    let n = input.options.count.max(1);
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let Some(total) = player_groups
        .len()
        .checked_mul(teammate_groups.len())
        .and_then(|n| n.checked_mul(target_groups.len()))
    else {
        send(ProgressEvent::Done(Err("pair: 配队矩阵大小溢出。".to_owned())));
        return;
    };
    let converted_players = match player_groups
        .iter()
        .map(|player| player_group_to_ol(player))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(players) => players,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };
    let matrix_input = PairMatrixInput {
        players: &converted_players,
        teammates: &teammate_groups,
        targets: &target_groups,
        target_factors: &target_factors,
        teammate_factors: &teammate_factors,
        target_factored: input.target_factor_enabled,
        teammate_factored: input.teammate_factor_enabled,
        n,
        eval_rq,
        threads: outer_thread_spec(input.options.threads),
        cancel: &input.cancel,
    };
    let mut last_progress = Instant::now();
    send(ProgressEvent::Progress { done: 0, total });
    let on_progress = |done| {
        // 短 matchup 可能每秒完成数万次；不让 GUI 消息积压反过来限制计算吞吐。
        if done == total || last_progress.elapsed() >= std::time::Duration::from_millis(40) {
            send(ProgressEvent::Progress { done, total });
            last_progress = Instant::now();
        }
    };
    let on_player = |player_index: usize, rates: Vec<(f64, usize)>| {
        if observer.is_some() && output.is_none() {
            return Ok(());
        }
        let player_label = &player_labels[player_index];
        let mut pair_rates = rates
            .into_iter()
            .map(|(rate, index)| (rate, teammate_labels[index].clone()))
            .collect::<Vec<_>>();
        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        if input.options.min_file.is_none_or(|limit| final_score >= limit)
            && let Some(output) = output.as_mut()
        {
            let line = format_pair_file_record(
                input.output_mode,
                player_label,
                final_score,
                selected_count,
                head,
                &pair_rates,
                precision,
            );
            writeln!(output, "{line}").map_err(|err| format!("写入输出文件失败: {err}"))?;
        }
        if observer.is_none() && input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                player_label,
                final_score,
                selected_count,
                &pair_rates,
                input.detail_mode,
                input.detail_min,
                precision,
            );
            if should_highlight(final_score, input.options.min_screen, input.highlight_delta) {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        Ok(())
    };
    let result = if let Some(observer) = observer {
        let mut pending = std::collections::HashMap::<usize, (usize, Vec<(f64, usize)>)>::new();
        run_pair_matrix_observed(&matrix_input, on_progress, on_player, &mut |player, teammate, rate| {
            let entry = pending.entry(player).or_default();
            entry.0 += 1;
            if let Some(rate) = rate {
                entry.1.push((rate, teammate));
            }
            let mut update = ResultUpdate::new(player, &player_labels[player], ResultKind::Pair, precision);
            update.top = (input.detail_mode == super::types::PairDetailMode::Top).then_some(head);
            if let Some(rate) = rate {
                let visible = match input.detail_mode {
                    super::types::PairDetailMode::None => false,
                    super::types::PairDetailMode::Top => true,
                    super::types::PairDetailMode::Every => input.detail_min.is_none_or(|min| rate >= min),
                };
                if visible {
                    update
                        .entries
                        .push(ResultEntry::number(teammate, teammate_labels[teammate].clone(), rate));
                }
            }
            if entry.0 == teammate_groups.len() {
                let (_, mut rates) = pending.remove(&player).unwrap();
                rates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let score = rates.iter().take(head).map(|(rate, _)| rate).sum();
                update.finish = Some(ResultFinish {
                    score: Some(score),
                    visible: input.options.min_screen.is_none_or(|min| score >= min),
                    highlight: should_highlight(score, input.options.min_screen, input.highlight_delta),
                });
            }
            if !update.entries.is_empty() || update.finish.is_some() {
                observer(update);
            }
        })
    } else {
        run_pair_matrix(&matrix_input, on_progress, on_player)
    };
    if let Err(err) = result {
        send(ProgressEvent::Done(Err(err)));
        return;
    }
    if input.cancel.load(Ordering::Relaxed) {
        send(ProgressEvent::Done(Ok("已停止。".to_owned())));
        return;
    }

    if let Err(err) = finalize_sorted_output_file(output.take(), input.output_file.as_deref(), input.output_mode) {
        send(ProgressEvent::Done(Err(err)));
        return;
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

fn parse_pair_target_groups(content: &str, factor_enabled: bool) -> Result<(Vec<String>, Vec<f64>), String> {
    if factor_enabled {
        parse_factored_target_groups(content)
    } else {
        let groups = parse_target_groups(content, false);
        let factors = vec![1.0; groups.len()];
        Ok((groups, factors))
    }
}

type ParsedTeammates = (Vec<String>, Vec<String>, Vec<f64>);

fn parse_pair_teammate_groups(content: &str, double_plus: bool, factor_enabled: bool) -> Result<ParsedTeammates, String> {
    if factor_enabled {
        let (groups, factors) = parse_factored_target_groups(content)?;
        let labels = groups.iter().map(|group| group.lines().collect::<Vec<_>>().join("+")).collect();
        Ok((groups, labels, factors))
    } else {
        let (groups, labels) = parse_player_groups_with_labels(content, double_plus);
        let factors = vec![1.0; groups.len()];
        Ok((groups, labels, factors))
    }
}

#[cfg(test)]
fn teammate_score(average_rate: f64, factor: f64, factor_enabled: bool) -> f64 {
    if factor_enabled { average_rate * factor } else { average_rate }
}

pub(super) fn should_highlight(score: f64, min_screen: Option<f64>, highlight_delta: Option<f64>) -> bool {
    highlight_delta.is_some_and(|delta| score >= min_screen.unwrap_or(0.0) + delta)
}

fn finish_output(output_file: Option<&Path>, out: String) -> Result<String, String> {
    match output_file {
        Some(path) => {
            let mut file = create_output_file(path)?;
            file.write_all(out.as_bytes())
                .and_then(|_| file.flush())
                .map_err(|err| format!("写入输出文件失败: {}: {err}", path.display()))?;
            Ok(format!("完成，结果已写入: {}", path.display()))
        }
        None => Ok(out),
    }
}

pub(super) fn eval_rq(keep_rq: bool) -> f64 {
    if keep_rq {
        tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ
    } else {
        WIN_RATE_EVAL_RQ
    }
}

fn player_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    cli_api::to_diy(raw, false, false).map_err(|err| format!("转换 player-list 名字为 +ol 失败: {raw}: {err}"))
}

fn player_group_to_ol(group: &str) -> Result<String, String> {
    group
        .lines()
        .map(player_to_ol)
        .collect::<Result<Vec<_>, _>>()
        .map(|players| players.join("\n"))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ;
    use tswn_core::namerena::{NamerenaInput, PreparedPlayer, PreparedRoster};

    use crate::backend::format::format_batch_screen_log;
    use crate::backend::{BatchRateInput, CommonBenchOptions, OutputMode, run_batch_rate};

    use super::{ProgressEvent, bench_batch_rate_for_group, parse_pair_target_groups, parse_pair_teammate_groups, run_to_diy};

    #[test]
    fn pair_parses_factored_targets_with_their_weights() {
        let raw = "[[targets]]\nfactor = 2\nplayers = [\"mario\", \"luigi\"]\n\n[[targets]]\nfactor = 0.5\nplayers = [\"peach\"]";
        let (groups, factors) = parse_pair_target_groups(raw, true).expect("factored targets should parse");
        assert_eq!(groups, vec!["mario\nluigi", "peach"]);
        assert_eq!(factors, vec![2.0, 0.5]);
    }

    #[test]
    fn pair_keeps_each_member_when_converting_a_multi_player_input_group() {
        let group = "+ol:player-a\n+ol:player-b";
        assert_eq!(super::player_group_to_ol(group).unwrap(), group);
    }

    #[test]
    fn teammate_factor_changes_the_score_used_for_head_sorting() {
        assert_eq!(super::teammate_score(80.0, 0.5, true), 40.0);
        assert_eq!(super::teammate_score(80.0, 0.5, false), 80.0);
    }

    #[test]
    fn pair_parses_factored_teammates_with_labels_and_weights() {
        let raw = "[[targets]]\nfactor = 2\nplayers = [\"mario\", \"luigi\"]";
        let (groups, labels, factors) = parse_pair_teammate_groups(raw, true, true).expect("valid teammates");
        assert_eq!(groups, vec!["mario\nluigi"]);
        assert_eq!(labels, vec!["mario+luigi"]);
        assert_eq!(factors, vec![2.0]);
    }

    /// 一行输入按整队构建后应该追加的全部详情块（含每个块前面的空行）。
    fn expected_group_blocks(raw: &str) -> String {
        let groups = super::parse_namer_pf_groups(raw);
        let input = NamerenaInput::from_raw_groups(&groups).expect("group should parse");
        let roster = PreparedRoster::build(&input, DEFAULT_EVAL_RQ).expect("group should build");
        roster
            .players
            .iter()
            .map(|player| format!("\n{}", expected_detail_block(player)))
            .collect()
    }

    /// 多行输入的全部输出：每行之间空一行，行内是导出行 + 该行每个成员的详情块。
    fn expected_multiline_output(raw: &str, old: bool) -> String {
        let mut out = String::new();
        for name in super::parse_line_list(raw) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&tswn_core::cli_api::to_diy(&name, old, false).expect("export should work"));
            out.push('\n');
            out.push_str(&expected_group_blocks(&name));
        }
        out
    }

    /// 详情块必须逐字段等于同一份导出 attrs / skills（含组队差额）的换算结果。
    fn expected_detail_block(player: &PreparedPlayer) -> String {
        let attrs = super::export_attrs(player.attrs);
        let solo = super::solo_player_of(&player.id_key_name).expect("solo build should work");
        let solo_attrs = super::export_attrs(solo.attrs);
        let solo_levels = super::skill_levels(&solo);
        let mut block = format!("=== 原始信息 ===\n{}\n", player.id_key_name);
        block.push_str(&format!(
            "HP {} 攻 {} 防 {} 速 {} 敏 {} 魔 {} 抗 {} 智 {} 八围 {} 嘲讽{}\n",
            super::format_delta(i64::from(attrs[7]), i64::from(solo_attrs[7])),
            super::format_delta(i64::from(attrs[0]), i64::from(solo_attrs[0])),
            super::format_delta(i64::from(attrs[1]), i64::from(solo_attrs[1])),
            super::format_delta(i64::from(attrs[2]), i64::from(solo_attrs[2])),
            super::format_delta(i64::from(attrs[3]), i64::from(solo_attrs[3])),
            super::format_delta(i64::from(attrs[4]), i64::from(solo_attrs[4])),
            super::format_delta(i64::from(attrs[5]), i64::from(solo_attrs[5])),
            super::format_delta(i64::from(attrs[6]), i64::from(solo_attrs[6])),
            super::format_eight_ring(attrs),
            super::taunt_value(attrs),
        ));
        for skill in super::action_order_skills(player, &solo_levels) {
            let level = i64::from(skill.level);
            block.push_str(&format!(
                "  {} {}\n",
                skill.name,
                super::format_delta(level, level - skill.delta)
            ));
        }
        block
    }

    /// 面板口径的完整示例：每行导出后，按该行整队逐个成员追加详情块；行与行之间空一行。
    #[test]
    fn to_diy_details_match_panel_layout() {
        let cancel = AtomicBool::new(false);
        let raw = "1@team+2@team\ntest";
        let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();

        assert_eq!(output, expected_multiline_output(raw, false));
    }

    /// 多行输入之间必须空行分隔，详情块不能和下一行的导出行贴在一起。
    #[test]
    fn to_diy_details_separate_lines() {
        let cancel = AtomicBool::new(false);
        let raw = "1@team+2@team\ntest";
        let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();
        let lines = output.lines().collect::<Vec<_>>();
        let first_export_of_second_line = lines.iter().position(|line| line.starts_with("test+ol:")).expect("second export");

        assert_eq!(lines[first_export_of_second_line - 1], "");
        assert_eq!(lines[first_export_of_second_line - 2], "  潜行 34");
        // 行内成员之间同样空行分隔。
        assert_eq!(lines[12], "");
        assert_eq!(lines[13], "=== 原始信息 ===");
    }

    /// 同队升级也会改技能熟练度，技能行同样要标差额。
    #[test]
    fn to_diy_details_mark_team_skill_delta() {
        let cancel = AtomicBool::new(false);
        let output = run_to_diy(
            "冥河 WyO8MUZPPtKH@Afterglow+光 jKLA6V5mirfs@Afterglow",
            false,
            false,
            true,
            None,
            &cancel,
        )
        .unwrap();
        let lines = output.lines().collect::<Vec<_>>();

        // 第一个成员：只有「智」被组队升级（+21），技能没有变化。
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "=== 原始信息 ===");
        assert_eq!(lines[3], "冥河 WyO8MUZPPtKH@Afterglow");
        assert_eq!(
            lines[4],
            "HP 311 攻 56 防 83 速 98 敏 65 魔 90 抗 96 智 94(+21) 八围 685.7 嘲讽289"
        );
        assert_eq!(lines[5], "  守护 20");
        assert_eq!(lines[13], "  魅惑 80");
        assert_eq!(lines[12], "  分身 56");
        // 第二个成员：护符从单独构建的 84 升到 98，技能行标 `(+14)`。
        assert_eq!(lines[14], "");
        assert_eq!(lines[15], "=== 原始信息 ===");
        assert_eq!(lines[16], "光 jKLA6V5mirfs@Afterglow");
        assert_eq!(lines[17], "HP 315 攻 95 防 73 速 92 敏 77 魔 89 抗 86 智 84 八围 701.0 嘲讽395");
        assert_eq!(lines[18], "  苏生 8");
        assert_eq!(lines[19], "  隐匿 1");
        assert_eq!(lines[20], "  吞噬 14");
        assert_eq!(lines[21], "  反击 6");
        assert_eq!(lines[22], "  命轮 20");
        assert_eq!(lines[23], "  分身 58");
        assert_eq!(lines[24], "  护符 98(+14)");
        assert_eq!(lines.len(), 25);
    }

    #[test]
    fn to_diy_details_reproduce_reported_example() {
        let cancel = AtomicBool::new(false);
        let output = run_to_diy("1@team+2@team\ntest", false, false, true, None, &cancel).unwrap();
        let lines = output.lines().collect::<Vec<_>>();

        // 组队行导出后，紧跟该队两个玩家的详情块。
        assert!(lines[0].starts_with("1@team+ol:{\"attrs\":[78,78,58,64,72,60,77,245]"));
        assert!(lines[0].contains("\"sklprotect\":19"));
        assert!(lines[0].contains("+2@team+ol:{\"attrs\":[81,83,59,70,71,61,61,271]"));
        assert!(lines[0].contains("\"sklassassinate\":\"2*17\""));
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "=== 原始信息 ===");
        assert_eq!(lines[3], "1@team");
        // 组队后 `1` 的 HP 是 245（单独构队 243），括号里标出 +2；八围 568.7、嘲讽 281。
        assert_eq!(
            lines[4],
            "HP 245(+2) 攻 78 防 78 速 58 敏 64 魔 72 抗 60 智 77 八围 568.7 嘲讽281"
        );
        assert_eq!(lines[5], "  守护 19");
        assert_eq!(lines[6], "  加速 14");
        assert_eq!(lines[7], "  诅咒 29");
        assert_eq!(lines[8], "  分身 4");
        assert_eq!(lines[9], "  聚气 2");
        assert_eq!(lines[10], "  反弹 1");
        assert_eq!(lines[11], "  护符 4");
        // 同一队的第二个玩家也有自己的块（属性与单独构建一致，所以没有括号）。
        assert_eq!(lines[12], "");
        assert_eq!(lines[13], "=== 原始信息 ===");
        assert_eq!(lines[14], "2@team");
        assert_eq!(lines[15], "HP 271 攻 81 防 83 速 59 敏 70 魔 71 抗 61 智 61 八围 576.3 嘲讽265");
        // 第二行单名：导出、空行、详情块。
        let solo_at = lines.iter().position(|line| line.starts_with("test+ol:")).expect("solo export line");
        assert_eq!(lines[solo_at + 1], "");
        assert_eq!(lines[solo_at + 2], "=== 原始信息 ===");
        assert_eq!(lines[solo_at + 3], "test");
        assert!(lines[solo_at + 4].starts_with("HP "));
        assert!(lines[solo_at + 4].contains(" 八围 "));
        assert!(lines[solo_at + 4].contains(" 嘲讽"));
        assert_eq!(lines.len(), solo_at + 5 + 7);
    }

    /// 用 overlay 固定输入，逐字节校验详情块的排版与算值。
    #[test]
    fn to_diy_details_follow_single_name_layout() {
        let cancel = AtomicBool::new(false);
        // overlay 的 attrs 会被解码成内部属性，导出行写回时再 +36，取 ≥36 保证与输入一致。
        let raw =
            r#"mario+ol:{"attrs":[40,50,60,70,80,90,100,200],"skills":{"sklfire":5,"sklheal":40},"name_factor_enabled":true}"#;
        let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();

        // 八围 = 490 + 200/3 = 556.7
        // 嘲讽 = 50*2 + 90*2 - 40*2 - 80*2 - 60*2 - 70 - 100 = -250
        assert_eq!(
            output,
            format!(
                "{raw}\n\n=== 原始信息 ===\nmario\nHP 200 攻 40 防 50 速 60 敏 70 魔 80 抗 90 智 100 八围 556.7 嘲讽250\n  火球 5\n  治愈 40\n"
            )
        );
    }

    #[test]
    fn to_diy_details_are_only_extra_log_lines() {
        let cancel = AtomicBool::new(false);
        let raw = r#"mario+diy[72,39,69,76,67,66,0,84]{"sklfire":5}"#;
        let without_details = run_to_diy(raw, true, false, false, None, &cancel).unwrap();
        let with_details = run_to_diy(raw, true, false, true, None, &cancel).unwrap();

        assert!(!without_details.contains("=== 原始信息 ==="));
        // 勾选详情只在导出行后面追加日志块，导出行本身逐字节不变。
        assert!(with_details.starts_with(&without_details));
        assert!(with_details.contains("=== 原始信息 ==="));
    }

    #[test]
    fn to_diy_details_apply_per_line_and_per_group_member() {
        let cancel = AtomicBool::new(false);
        let one_line = run_to_diy("mario@team", true, false, true, None, &cancel).unwrap();
        let two_lines = run_to_diy("mario@team\nluigi@team", true, false, true, None, &cancel).unwrap();
        // `+` 是组队分隔符：这行有两个成员，两个成员各出一个详情块。
        let group_line = run_to_diy("mario@team+fire", true, false, true, None, &cancel).unwrap();

        assert!(one_line.contains("=== 原始信息 ==="));
        assert_eq!(two_lines.matches("=== 原始信息 ===").count(), 2);
        assert!(two_lines.contains("mario@team\n"));
        assert!(two_lines.contains("luigi@team\n"));
        assert_eq!(group_line.matches("=== 原始信息 ===").count(), 2);
    }

    #[test]
    fn to_diy_details_never_reach_the_output_file() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("tswn_openbox_to_diy_details_{}_{seq}.txt", std::process::id()));
        let cancel = AtomicBool::new(false);

        let summary = run_to_diy("mario@team", true, false, true, Some(path.clone()), &cancel).expect("export should succeed");
        let written = std::fs::read_to_string(&path).expect("output file should exist");
        let _ = std::fs::remove_file(&path);

        assert!(summary.starts_with("完成，结果已写入"));
        assert!(!written.contains("=== 原始信息 ==="));
        assert_eq!(written, run_to_diy("mario@team", true, false, false, None, &cancel).unwrap());
    }

    #[test]
    fn to_diy_detail_taunt_uses_absolute_value() {
        assert_eq!(super::taunt_value([78, 78, 58, 64, 72, 60, 77, 245]), 281);
        assert_eq!(super::taunt_value([81, 83, 59, 70, 71, 61, 61, 271]), 265);
        assert_eq!(super::taunt_value([85, 72, 79, 76, 81, 41, 70, 236]), 410);
        assert_eq!(super::taunt_value([0, 0, 0, 0, 0, 0, 0, 0]), 0);
    }

    #[test]
    fn to_diy_detail_eight_ring_keeps_one_decimal() {
        // 面板样例：七围 487 + HP 245 → 487 + 81.7 = 568.7。
        assert_eq!(super::format_eight_ring([78, 78, 58, 64, 72, 60, 77, 245]), "568.7");
        // `2@team`：七围 486 + HP 271 → 486 + 90.3 = 576.3。
        assert_eq!(super::format_eight_ring([81, 83, 59, 70, 71, 61, 61, 271]), "576.3");
        // `test`：七围 504 + HP 236 → 504 + 78.7 = 582.7。
        assert_eq!(super::format_eight_ring([85, 72, 79, 76, 81, 41, 70, 236]), "582.7");
        // 仓库当前 `test` 的构建：七围 478 + HP 259 → 564.3。
        assert_eq!(super::format_eight_ring([71, 80, 59, 70, 75, 56, 67, 259]), "564.3");
        assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 0]), "0.0");
        // 四舍五入：HP 1470 / 3 = 490.0 正好落在整数上；1482 / 3 = 494.0。
        assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1470]), "490.0");
        assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1482]), "494.0");
        // 1472 / 3 = 490.66…：进位到 490.7。
        assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1472]), "490.7");
    }

    #[test]
    fn to_diy_plus_line_exports_team_group() {
        let cancel = AtomicBool::new(false);
        let output = run_to_diy("1@a\n2@a\n1@a+2@a", true, false, false, None, &cancel).unwrap();
        let lines = output.lines().collect::<Vec<_>>();

        // 每行结果之间空一行。
        assert_eq!(lines.len(), 5);
        assert!(lines[0].starts_with("1@a+diy["));
        assert_eq!(lines[1], "");
        assert!(lines[2].starts_with("2@a+diy["));
        assert_eq!(lines[3], "");
        assert!(lines[4].starts_with("1@a+diy["));
        assert!(lines[4].contains("+2@a+diy["));
    }

    #[test]
    fn batch_rate_runtime_matrix_matches_legacy_summary_and_order() {
        let players = ["alpha@red", "beta@blue"];
        let targets = ["gamma@green", "delta@yellow"];
        let mut expected = Vec::new();
        for player in players {
            let mut verbose = String::new();
            let cancel = AtomicBool::new(false);
            let summary = bench_batch_rate_for_group(
                player,
                &targets.map(str::to_owned),
                None,
                24,
                Some(1),
                DEFAULT_EVAL_RQ,
                false,
                &mut verbose,
                &cancel,
                |_, _, _, _| {},
            );
            expected.push(format_batch_screen_log(player, summary.avg, &[], 9));
        }

        let events = RefCell::new(Vec::new());
        run_batch_rate(
            BatchRateInput {
                target_text: targets.join("\n"),
                player_text: players.join("\n"),
                target_factor_enabled: false,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: false,
                highlight_delta: None,
                output_mode: OutputMode::Log,
                output_file: None,
                options: CommonBenchOptions {
                    count: 24,
                    threads: Some(4),
                    keep_rq: true,
                    verbose: false,
                    min_screen: None,
                    min_file: None,
                    wr_precision: 9,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
            |event| events.borrow_mut().push(event),
        );

        let events = events.into_inner();
        let actual = events
            .iter()
            .filter_map(|event| match event {
                ProgressEvent::Log(line) if players.iter().any(|player| line.contains(player)) => Some(line.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Progress { done: 4, total: 4 })));
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Done(Ok(_)))));
    }

    #[test]
    fn factored_mirror_match_is_weighted_as_fifty_percent() {
        let events = RefCell::new(Vec::new());
        run_batch_rate(
            BatchRateInput {
                target_text: "[[targets]]\nfactor = 2.5\nplayers = [\"mario\", \"luigi\"]".to_string(),
                player_text: "mario+luigi".to_string(),
                target_factor_enabled: true,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: true,
                highlight_delta: None,
                output_mode: OutputMode::Log,
                output_file: None,
                options: CommonBenchOptions {
                    count: 1,
                    threads: Some(1),
                    keep_rq: true,
                    verbose: false,
                    min_screen: None,
                    min_file: None,
                    wr_precision: 9,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
            |event| events.borrow_mut().push(event),
        );

        let events = events.into_inner();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProgressEvent::Log(log) if log.contains("50.000000000")))
        );
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Progress { done: 1, total: 1 })));
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Done(Ok(_)))));
    }
}
