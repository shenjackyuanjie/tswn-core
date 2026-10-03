//! 配队任务：输入准备、队友明细与最终排名。

mod matrix;

use super::format::{format_pair_file_record, format_pair_screen_log, should_highlight};
use super::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::{create_output_file, finalize_sorted_output_file};
use super::parse::{parse_factored_target_groups, parse_player_groups_with_labels, parse_target_groups};
use super::score::{eval_rq, outer_thread_spec};
use super::types::{PairInput, ProgressEvent};
use matrix::{PairMatrixInput, run_pair_matrix, run_pair_matrix_observed};
use std::io::Write as _;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tswn_core::cli_api;

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) { run_pair_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_pair_observed(input: PairInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    let PreparedPairGroups {
        target_groups,
        target_factors,
        players,
        player_labels,
        teammates,
        teammate_labels,
        teammate_factors,
    } = match prepare_pair_groups(&input) {
        Ok(prepared) => prepared,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };

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
    let Some(total) = players
        .len()
        .checked_mul(teammates.len())
        .and_then(|n| n.checked_mul(target_groups.len()))
    else {
        send(ProgressEvent::Done(Err("pair: 配队矩阵大小溢出。".to_owned())));
        return;
    };
    let matrix_input = PairMatrixInput {
        players: &players,
        teammates: &teammates,
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
    let on_player = |player_index: usize, mut rates: Vec<(f64, usize)>| {
        // 相同胜率保留输入顺序，文件和 observed 日志复用同一次排序、求和。
        rates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let selected_count = head.min(rates.len());
        let final_score = rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let player_label = &player_labels[player_index];
        let visible = input.options.min_screen.is_none_or(|limit| final_score >= limit);
        let highlight = should_highlight(final_score, input.options.min_screen, input.highlight_delta);
        let file_visible = output.is_some() && input.options.min_file.is_none_or(|limit| final_score >= limit);
        let pair_rates = if visible || file_visible {
            rates
                .into_iter()
                .map(|(rate, index)| (rate, teammate_labels[index].clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if file_visible && let Some(output) = output.as_mut() {
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
        let legacy_log = visible.then(|| {
            (
                format_pair_screen_log(
                    player_label,
                    final_score,
                    selected_count,
                    &pair_rates,
                    input.detail_mode,
                    input.detail_min,
                    precision,
                ),
                if highlight { EntryKind::Highlight } else { EntryKind::Plain },
            )
        });
        if let Some(observer) = observer {
            let mut update = ResultUpdate::new_with_legacy(player_index, player_label, ResultKind::Pair, precision);
            update.top = (input.detail_mode == super::types::PairDetailMode::Top).then_some(head);
            update.finish = Some(ResultFinish {
                score: Some(final_score),
                visible,
                highlight,
            });
            update.legacy_log = legacy_log;
            observer(update);
        } else if let Some((log, kind)) = legacy_log {
            if kind == EntryKind::Highlight {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        Ok(())
    };
    let result = if let Some(observer) = observer {
        run_pair_matrix_observed(&matrix_input, on_progress, on_player, &mut |player, teammate, rate| {
            let mut update = ResultUpdate::new_with_legacy(player, &player_labels[player], ResultKind::Pair, precision);
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
            if !update.entries.is_empty() {
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

/// pair 的输入准备结果：靶子与两侧成员文本均已就绪，其中选手和队友都完成按成员冻结。
struct PreparedPairGroups {
    target_groups: Vec<String>,
    target_factors: Vec<f64>,
    players: Vec<String>,
    player_labels: Vec<String>,
    teammates: Vec<String>,
    teammate_labels: Vec<String>,
    teammate_factors: Vec<f64>,
}

/// 解析并冻结 pair 的两侧输入。
///
/// 选手与队友都必须按“逐个成员单独构建”导出成 `+ol`：冻结后的属性与技能来自该成员
/// 单独构队的结果，同公会成员之间的组队加成不会再作用到它身上（core 的
/// `apply_team_upgrades` 只改 `name_base`，而 overlay 的 `attrs` / `skills` 会直接覆盖），
/// 这样每个（选手，队友）组合里的两侧都保持各自独立、可复现的强度，也不会互相污染。
/// 只冻结一侧会让另一侧单方面吃到加成：带权队友 TOML 里的普通名字正好是这种情况。
///
/// 已经是 `+diy` / `+ol` 的输入原样保留，不做二次导出；返回的标签仍是原始输入行，
/// 日志与文件输出不受影响。
fn prepare_pair_groups(input: &PairInput) -> Result<PreparedPairGroups, String> {
    let (target_groups, target_factors) = parse_pair_target_groups(&input.target_text, input.target_factor_enabled)?;
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    let (teammate_groups, teammate_labels, teammate_factors) =
        parse_pair_teammate_groups(&input.teammate_text, input.teammate_double_plus, input.teammate_factor_enabled)?;
    if target_groups.is_empty() {
        return Err("pair: 靶子列表为空。".to_string());
    }
    if player_groups.is_empty() {
        return Err("pair: 选手列表为空。".to_string());
    }
    if teammate_groups.is_empty() {
        return Err("pair: 队友列表为空。".to_string());
    }
    Ok(PreparedPairGroups {
        target_groups,
        target_factors,
        players: groups_to_ol(&player_groups)?,
        player_labels,
        teammates: groups_to_ol(&teammate_groups)?,
        teammate_labels,
        teammate_factors,
    })
}

/// 逐组按成员冻结；组内成员各自单独构建，成员之间不会互相加成。
fn groups_to_ol(groups: &[String]) -> Result<Vec<String>, String> { groups.iter().map(|group| group_to_ol(group)).collect() }

fn member_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    cli_api::to_diy(raw, false, false).map_err(|err| format!("转换名字为 +ol 失败: {raw}: {err}"))
}

/// 选手与队友共用：一组输入里的每个成员单独导出为 `+ol`，再用换行拼回同一组。
fn group_to_ol(group: &str) -> Result<String, String> {
    group
        .lines()
        .map(member_to_ol)
        .collect::<Result<Vec<_>, _>>()
        .map(|members| members.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::types::{CommonBenchOptions, OutputMode, PairDetailMode};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ;
    use tswn_core::namerena::{NamerenaInput, PreparedRoster};

    #[test]
    fn pair_keeps_each_member_when_converting_a_multi_player_input_group() {
        let group = "+ol:player-a\n+ol:player-b";
        assert_eq!(super::group_to_ol(group).unwrap(), group);
    }

    #[test]
    fn pair_parses_factored_teammates_with_labels_and_weights() {
        let raw = "[[targets]]\nfactor = 2\nplayers = [\"mario\", \"luigi\"]";
        let (groups, labels, factors) = parse_pair_teammate_groups(raw, true, true).expect("valid teammates");
        assert_eq!(groups, vec!["mario\nluigi"]);
        assert_eq!(labels, vec!["mario+luigi"]);
        assert_eq!(factors, vec![2.0]);
    }

    fn pair_input(player_text: &str, teammate_text: &str, teammate_factored: bool) -> PairInput {
        PairInput {
            target_text: "target@blue".into(),
            target_factor_enabled: false,
            player_text: player_text.into(),
            player_double_plus: false,
            teammate_text: teammate_text.into(),
            teammate_double_plus: true,
            teammate_factor_enabled: teammate_factored,
            head: 1,
            detail_mode: PairDetailMode::None,
            detail_min: None,
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
                wr_precision: 3,
            },
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 一组输入里指定成员的属性与技能，用 core 的构队结果取值。
    fn built_snapshot(groups: &[Vec<String>], name: &str) -> ([u32; 8], Vec<(usize, u32)>) {
        let input = NamerenaInput::from_raw_groups(groups).expect("groups should parse");
        let roster = PreparedRoster::build(&input, DEFAULT_EVAL_RQ).unwrap();
        let player = roster.players.iter().find(|player| player.id_key_name == name).expect("player missing");
        let mut skills = player
            .skills
            .entries
            .iter()
            .filter(|entry| entry.level > 0)
            .map(|entry| (entry.key, entry.level))
            .collect::<Vec<_>>();
        skills.sort();
        (player.attrs, skills)
    }

    /// 复现 matrix 的队伍拼接后，取该队伍里某个成员的构建结果。
    fn team_member_snapshot(player: &str, teammate: &str, name: &str) -> ([u32; 8], Vec<(usize, u32)>) {
        let team = format!("{player}\n{teammate}");
        let groups = vec![
            team.lines().map(|line| line.trim().to_owned()).collect::<Vec<_>>(),
            vec!["target@blue".to_owned()],
        ];
        built_snapshot(&groups, name)
    }

    /// 选手与队友都必须冻结成“单独构建”，否则同公会成员会单方面拿到组队加成：
    /// `1@team` 单独 HP 243，和 `2@team` 同队后是 245（+2 组队加成）。
    #[test]
    fn pair_freezes_both_sides_so_team_bonus_does_not_apply() {
        let solo_one = built_snapshot(&[vec!["1@team".to_owned()]], "1@team");
        let solo_two = built_snapshot(&[vec!["2@team".to_owned()]], "2@team");
        assert_eq!(solo_one.0[7], 243);
        assert_eq!(solo_two.0[7], 271);
        // 未冻结的原始组队确实存在加成，保证本测试不是空转。
        assert_eq!(
            built_snapshot(&[vec!["2@team".to_owned(), "1@team".to_owned()]], "1@team").0[7],
            245
        );

        // 选手带权靶子/队友两条解析路径都要冻结队友；选手侧顺带覆盖“被加成方是选手”。
        for (player_text, teammate_text, teammate_factored, teammate_label) in [
            ("2@team", "1@team", false, "1@team"),
            ("2@team", "[[targets]]\nfactor = 1\nplayers = [\"1@team\"]", true, "1@team"),
            ("1@team", "2@team", false, "2@team"),
        ] {
            let input = pair_input(player_text, teammate_text, teammate_factored);
            let prepared = prepare_pair_groups(&input).expect("prepare should succeed");
            assert!(
                prepared.players[0].contains("+ol:"),
                "选手应冻结成 +ol: {}",
                prepared.players[0]
            );
            assert!(
                prepared.teammates[0].contains("+ol:"),
                "队友应冻结成 +ol: {}",
                prepared.teammates[0]
            );
            assert_eq!(
                prepared.player_labels,
                vec![player_text.to_string()],
                "选手标签应保留原始输入行"
            );
            assert_eq!(
                prepared.teammate_labels,
                vec![teammate_label.to_string()],
                "队友标签应保留原始输入行"
            );
            assert_eq!(
                team_member_snapshot(&prepared.players[0], &prepared.teammates[0], "1@team"),
                solo_one
            );
            assert_eq!(
                team_member_snapshot(&prepared.players[0], &prepared.teammates[0], "2@team"),
                solo_two
            );
        }
    }

    /// 一行多名成员时逐个成员冻结：组内成员之间的组队加成同样不进入 pair 计算。
    #[test]
    fn pair_freezes_every_member_of_a_player_group() {
        let input = pair_input("1@team+2@team", "3@team", false);
        let prepared = prepare_pair_groups(&input).expect("prepare should succeed");
        assert_eq!(prepared.players[0].lines().count(), 2);
        assert!(prepared.players[0].lines().all(|line| line.contains("+ol:")));
        assert_eq!(
            team_member_snapshot(&prepared.players[0], &prepared.teammates[0], "1@team"),
            built_snapshot(&[vec!["1@team".to_owned()]], "1@team")
        );
        assert_eq!(
            team_member_snapshot(&prepared.players[0], &prepared.teammates[0], "2@team"),
            built_snapshot(&[vec!["2@team".to_owned()]], "2@team")
        );
    }

    /// 冻结只改构建文本，不参与身份判定：镜像 50% 与重名跳过仍按原名比较。
    #[test]
    fn pair_freeze_keeps_identity_for_mirror_and_duplicate_checks() {
        let prepared = prepare_pair_groups(&pair_input("1@team", "2@team", false)).expect("prepare should succeed");
        let team = format!("{}\n{}", prepared.players[0], prepared.teammates[0]);
        assert_eq!(
            crate::backend::parse::normalized_group_players(&team),
            vec!["1@team".to_string(), "2@team".to_string()],
            "冻结后的 +ol 文本仍应还原出原始身份"
        );
        assert_eq!(
            crate::backend::parse::first_duplicate_name_in_matchup(&[&team, "2@team"]),
            Some("2@team".to_string()),
            "重名跳过仍按原名判定"
        );
    }
}
