use std::ffi::c_char;

use serde::Serialize;
use tswn_core::cli_api::{self as core_cli_api, CliApiError};

use crate::{
    FfiError, ffi_boundary, ffi_error, read_utf8, read_utf8_array, tswn_status_t, tswn_str_t, write_json_result,
    write_string_result,
};

#[derive(Serialize)]
struct JsonWinRateResult {
    wins: usize,
    total: usize,
    win_rate: f64,
    init_nanos: u64,
    fight_nanos: u64,
}

#[derive(Serialize)]
struct JsonGroupWinRateResult {
    opponent: String,
    result: JsonWinRateResult,
}

#[derive(Serialize)]
struct JsonScoreResult {
    score: f64,
    wins: usize,
    total: usize,
    errors: usize,
    init_nanos: u64,
    fight_nanos: u64,
}

#[derive(Serialize)]
struct JsonNamerPfResult {
    group: Vec<String>,
    modes: Vec<String>,
    scores: Vec<f64>,
    total_score: f64,
}

#[derive(Serialize)]
struct JsonBatchRateResult {
    label: String,
    avg_win_rate: f64,
    aggregate_win_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    init_nanos: u64,
    fight_nanos: u64,
}

#[derive(Serialize)]
struct JsonPairRateEntry {
    name: String,
    rate: f64,
}

#[derive(Serialize)]
struct JsonPairRateResult {
    label: String,
    final_score: f64,
    head: usize,
    selected: usize,
    top_pairs: Vec<JsonPairRateEntry>,
    aggregate_win_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    init_nanos: u64,
    fight_nanos: u64,
}

#[derive(Serialize)]
struct JsonIconInfo {
    border_style: usize,
    shapes: Vec<usize>,
    bg_color_idx: usize,
    bg_color: [u8; 3],
    fg_color_indices: Vec<usize>,
    fg_colors: Vec<[u8; 3]>,
    colors_consumed: usize,
}

fn nanos_to_u64(value: u128) -> u64 { u64::try_from(value).unwrap_or(u64::MAX) }

fn cli_api_error(err: CliApiError) -> FfiError {
    match err {
        CliApiError::InvalidInput(message) => ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, message),
        CliApiError::Runner(err) => ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()),
    }
}

impl From<core_cli_api::WinRateResult> for JsonWinRateResult {
    fn from(value: core_cli_api::WinRateResult) -> Self {
        Self {
            wins: value.wins,
            total: value.total,
            win_rate: value.win_rate,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::GroupWinRateResult> for JsonGroupWinRateResult {
    fn from(value: core_cli_api::GroupWinRateResult) -> Self {
        Self {
            opponent: value.opponent,
            result: value.result.into(),
        }
    }
}

impl From<core_cli_api::ScoreResult> for JsonScoreResult {
    fn from(value: core_cli_api::ScoreResult) -> Self {
        Self {
            score: value.score,
            wins: value.wins,
            total: value.total,
            errors: value.errors,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::NamerPfResult> for JsonNamerPfResult {
    fn from(value: core_cli_api::NamerPfResult) -> Self {
        Self {
            group: value.group,
            modes: value.modes,
            scores: value.scores,
            total_score: value.total_score,
        }
    }
}

impl From<core_cli_api::BatchRateResult> for JsonBatchRateResult {
    fn from(value: core_cli_api::BatchRateResult) -> Self {
        Self {
            label: value.label,
            avg_win_rate: value.avg_win_rate,
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::PairRateEntry> for JsonPairRateEntry {
    fn from(value: core_cli_api::PairRateEntry) -> Self {
        Self {
            name: value.name,
            rate: value.rate,
        }
    }
}

impl From<core_cli_api::PairRateResult> for JsonPairRateResult {
    fn from(value: core_cli_api::PairRateResult) -> Self {
        Self {
            label: value.label,
            final_score: value.final_score,
            head: value.head,
            selected: value.selected,
            top_pairs: value.top_pairs.into_iter().map(Into::into).collect(),
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::IconInfo> for JsonIconInfo {
    fn from(value: core_cli_api::IconInfo) -> Self {
        Self {
            border_style: value.border_style,
            shapes: value.shapes,
            bg_color_idx: value.bg_color_idx,
            bg_color: value.bg_color,
            fg_color_indices: value.fg_color_indices,
            fg_colors: value.fg_colors,
            colors_consumed: value.colors_consumed,
        }
    }
}

/// # Safety
///
/// `raw_text_utf8` 必须是有效的 UTF-8 C 字符串；`out_json` 必须指向可写输出位置。
/// 返回的 JSON 字符串需要由调用方使用 `tswn_str_free` 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate_summary_json(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    unsafe { tswn_win_rate_summary_json_with_eval_rq(raw_text_utf8, n, thread, crate::tswn_win_rate_eval_rq(), out_json) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate_summary_json_with_eval_rq(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let result = core_cli_api::win_rate_summary(&raw, n, Some(eval_rq), thread).map_err(cli_api_error)?;
        write_json_result(out_json, &JsonWinRateResult::from(result))
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_team_win_rate_summary_json(
    team1_utf8: *const c_char,
    team2_utf8: *const c_char,
    n: usize,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    unsafe {
        tswn_team_win_rate_summary_json_with_eval_rq(team1_utf8, team2_utf8, n, thread, crate::tswn_win_rate_eval_rq(), out_json)
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_team_win_rate_summary_json_with_eval_rq(
    team1_utf8: *const c_char,
    team2_utf8: *const c_char,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let team1 = unsafe { read_utf8(team1_utf8, "team1_utf8")? };
        let team2 = unsafe { read_utf8(team2_utf8, "team2_utf8")? };
        let result = core_cli_api::team_win_rate_summary(&team1, &team2, n, Some(eval_rq), thread).map_err(cli_api_error)?;
        write_json_result(out_json, &JsonWinRateResult::from(result))
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate_summary_json(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    unsafe {
        tswn_group_win_rate_summary_json_with_eval_rq(
            target_utf8,
            against_utf8,
            against_len,
            n,
            thread,
            crate::tswn_win_rate_eval_rq(),
            out_json,
        )
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate_summary_json_with_eval_rq(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let target = unsafe { read_utf8(target_utf8, "target_utf8")? };
        let against = unsafe { read_utf8_array(against_utf8, against_len, "against_utf8")? };
        let result = core_cli_api::group_win_rate_summary(&target, &against, n, Some(eval_rq), thread).map_err(cli_api_error)?;
        let json = result.into_iter().map(JsonGroupWinRateResult::from).collect::<Vec<_>>();
        write_json_result(out_json, &json)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_score_json(
    raw_text_utf8: *const c_char,
    n: usize,
    mode_utf8: *const c_char,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    unsafe { tswn_score_json_with_eval_rq(raw_text_utf8, n, mode_utf8, thread, crate::tswn_win_rate_eval_rq(), out_json) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_score_json_with_eval_rq(
    raw_text_utf8: *const c_char,
    n: usize,
    mode_utf8: *const c_char,
    thread: u32,
    eval_rq: f64,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let mode = unsafe { read_utf8(mode_utf8, "mode_utf8")? };
        let result = core_cli_api::score(&raw, n, &mode, Some(eval_rq), thread).map_err(cli_api_error)?;
        write_json_result(out_json, &JsonScoreResult::from(result))
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_namer_pf_json(
    raw_text_utf8: *const c_char,
    n: usize,
    modes_utf8: *const *const c_char,
    modes_len: usize,
    keep_rq: u8,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let modes = unsafe { read_utf8_array(modes_utf8, modes_len, "modes_utf8")? };
        let result = core_cli_api::namer_pf(&raw, n, if modes.is_empty() { None } else { Some(modes) }, keep_rq != 0, thread)
            .map_err(cli_api_error)?;
        let json = result.into_iter().map(JsonNamerPfResult::from).collect::<Vec<_>>();
        write_json_result(out_json, &json)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_batch_rate_json(
    target_groups_utf8: *const *const c_char,
    target_groups_len: usize,
    player_groups_utf8: *const *const c_char,
    player_groups_len: usize,
    n: usize,
    player_labels_utf8: *const *const c_char,
    player_labels_len: usize,
    keep_rq: u8,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let target_groups = unsafe { read_utf8_array(target_groups_utf8, target_groups_len, "target_groups_utf8")? };
        let player_groups = unsafe { read_utf8_array(player_groups_utf8, player_groups_len, "player_groups_utf8")? };
        let player_labels = unsafe { read_utf8_array(player_labels_utf8, player_labels_len, "player_labels_utf8")? };
        let result = core_cli_api::batch_rate(
            &target_groups,
            &player_groups,
            n,
            if player_labels.is_empty() { None } else { Some(player_labels) },
            keep_rq != 0,
            thread,
        )
        .map_err(cli_api_error)?;
        let json = result.into_iter().map(JsonBatchRateResult::from).collect::<Vec<_>>();
        write_json_result(out_json, &json)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_pair_rate_json(
    target_groups_utf8: *const *const c_char,
    target_groups_len: usize,
    players_utf8: *const *const c_char,
    players_len: usize,
    teammates_utf8: *const *const c_char,
    teammates_len: usize,
    head: usize,
    n: usize,
    keep_rq: u8,
    thread: u32,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let target_groups = unsafe { read_utf8_array(target_groups_utf8, target_groups_len, "target_groups_utf8")? };
        let players = unsafe { read_utf8_array(players_utf8, players_len, "players_utf8")? };
        let teammates = unsafe { read_utf8_array(teammates_utf8, teammates_len, "teammates_utf8")? };
        let result = core_cli_api::pair_rate(&target_groups, &players, &teammates, head, n, keep_rq != 0, thread)
            .map_err(cli_api_error)?;
        let json = result.into_iter().map(JsonPairRateResult::from).collect::<Vec<_>>();
        write_json_result(out_json, &json)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_to_diy(
    name_utf8: *const c_char,
    old: u8,
    minions: u8,
    out_result: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        let value = core_cli_api::to_diy(&name, old != 0, minions != 0).map_err(cli_api_error)?;
        write_string_result(out_result, value)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_to_diy_batch_json(
    names_utf8: *const *const c_char,
    names_len: usize,
    old: u8,
    minions: u8,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let names = unsafe { read_utf8_array(names_utf8, names_len, "names_utf8")? };
        let value = core_cli_api::to_diy_batch(&names, old != 0, minions != 0).map_err(cli_api_error)?;
        write_json_result(out_json, &value)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_icon_info_json(name_utf8: *const c_char, out_json: *mut tswn_str_t) -> tswn_status_t {
    ffi_boundary(|| {
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        let value = JsonIconInfo::from(core_cli_api::icon_info(&name));
        write_json_result(out_json, &value)
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_parse_group_lines_json(
    content_utf8: *const c_char,
    double_plus: u8,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let content = unsafe { read_utf8(content_utf8, "content_utf8")? };
        let value = core_cli_api::parse_group_lines(&content, double_plus != 0);
        write_json_result(out_json, &value)
    })
}
