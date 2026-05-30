use std::ffi::c_char;

use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::player::PlrId;
use tswn_core::{PreparedRunner, Runner};

use crate::{
    FfiResult, ffi_boundary, ffi_error, into_tswn_str, read_utf8, set_last_error, tswn_player_snapshot_t, tswn_prepared_runner_t,
    tswn_runner_t, tswn_status_t, tswn_str_t, tswn_update_snapshot_t, tswn_update_type_t, tswn_updates_t, tswn_win_rate_result_t,
};

fn read_optional_utf8(ptr: *const c_char, name: &str) -> FfiResult<Vec<String>> {
    if ptr.is_null() {
        Ok(Vec::new())
    } else {
        Ok(vec![unsafe { read_utf8(ptr, name)? }])
    }
}

fn ensure_groups_for_win_rate(groups: &[Vec<String>]) -> FfiResult<()> {
    let count = groups.iter().filter(|group| !group.is_empty()).count();
    if count < 2 {
        Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "win_rate requires at least two non-empty groups",
        ))
    } else {
        Ok(())
    }
}

fn update_type_to_c(update_type: UpdateType) -> tswn_update_type_t {
    match update_type {
        UpdateType::Win => tswn_update_type_t::Win,
        UpdateType::None => tswn_update_type_t::None,
        UpdateType::NextLine => tswn_update_type_t::NextLine,
    }
}

fn player_snapshot(player: &tswn_core::player::Player) -> tswn_player_snapshot_t {
    let status = player.get_status();
    tswn_player_snapshot_t {
        id: player.id(),
        ptr: player.ptr() as u64,
        hp: status.hp,
        max_hp: status.max_hp,
        magic_point: status.magic_point,
        move_point: status.move_point,
        attack: status.attack,
        defense: status.defense,
        speed: status.speed,
        agility: status.agility,
        magic: status.magic,
        resistance: status.resistance,
        wisdom: status.wisdom,
        point: status.point,
        all_sum: status.all_sum,
        name_factor: player.get_name_factor(),
        at_boost: status.at_boost,
        attract: status.attract,
        frozen: u8::from(status.frozen),
    }
}

fn update_snapshot(update: &RunUpdate) -> tswn_update_snapshot_t {
    tswn_update_snapshot_t {
        score: update.score,
        param: update.param.unwrap_or_default(),
        delay0: update.delay0,
        delay1: update.delay1,
        caster_id: update.caster as u64,
        target_id: update.target as u64,
        target_count: update.targets.len(),
        has_param: u8::from(update.param.is_some()),
        update_type: update_type_to_c(update.update_type),
    }
}

fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, eval_rq: f64, thread: u32) -> FfiResult<tswn_win_rate_result_t> {
    let summary = tswn_core::win_rate::prepared_win_rate(prepared, n, eval_rq, thread)
        .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
    Ok(tswn_win_rate_result_t {
        wins: summary.wins as u64,
        total: summary.total as u64,
    })
}

fn copy_ids(ids: &[PlrId], out: *mut u64, cap: usize, name: &str) -> FfiResult<()> {
    if ids.len() > cap {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            format!("{name} buffer too small"),
        ));
    }
    if ids.is_empty() {
        return Ok(());
    }
    if out.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} output is null")));
    }
    for (index, id) in ids.iter().enumerate() {
        unsafe {
            *out.add(index) = *id as u64;
        }
    }
    Ok(())
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
    thread: u32,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_prepared_win_rate_with_eval_rq(prepared, n, thread, crate::tswn_win_rate_eval_rq(), out_result) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate_with_eval_rq(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "prepared is null"));
        }
        if out_result.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
        }
        let result = run_prepared_win_rate(unsafe { &(*prepared).inner }, n, eval_rq, thread)?;
        unsafe {
            *out_result = result;
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_free(ptr: *mut tswn_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_free(ptr: *mut tswn_prepared_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_free(ptr: *mut tswn_updates_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    unsafe { tswn_runner_new_from_raw_with_eval_rq(raw_text_utf8, crate::tswn_default_eval_rq(), out_runner) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_raw_with_eval_rq(
    raw_text_utf8: *const c_char,
    eval_rq: f64,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_runner is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let (groups, seed) = Runner::split_namerena_into_groups(raw);
        let runner = Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_runner = Box::into_raw(Box::new(tswn_runner_t { inner: runner }));
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_prepared: *mut *mut tswn_prepared_runner_t,
) -> tswn_status_t {
    unsafe { tswn_prepared_runner_new_from_raw_with_eval_rq(raw_text_utf8, crate::tswn_default_eval_rq(), out_prepared) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_new_from_raw_with_eval_rq(
    raw_text_utf8: *const c_char,
    eval_rq: f64,
    out_prepared: *mut *mut tswn_prepared_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_prepared is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let groups = Runner::split_namerena_into_groups(raw).0;
        let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_prepared = Box::into_raw(Box::new(tswn_prepared_runner_t { inner: prepared }));
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_prepared(
    prepared: *const tswn_prepared_runner_t,
    seed_utf8: *const c_char,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "prepared is null"));
        }
        if out_runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_runner is null"));
        }
        let seed = read_optional_utf8(seed_utf8, "seed_utf8")?;
        let runner = Runner::new_from_prepared_with_seed(unsafe { &(*prepared).inner }, &seed)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_runner = Box::into_raw(Box::new(tswn_runner_t { inner: runner }));
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_have_winner(runner: *const tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.have_winner() })
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_run_to_completion(runner: *mut tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.run_to_completion() })
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_main_round(
    runner: *mut tswn_runner_t,
    out_updates: *mut *mut tswn_updates_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_updates is null"));
        }
        let updates = unsafe { (*runner).inner.main_round() };
        unsafe {
            *out_updates = Box::into_raw(Box::new(tswn_updates_t { inner: updates }));
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.input_groups.len() }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_len(runner: *const tswn_runner_t, group_index: usize) -> usize {
    if runner.is_null() {
        0
    } else {
        let input_groups = unsafe { &(*runner).inner.input_groups };
        input_groups.get(group_index).map(|g| g.len()).unwrap_or(0)
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_copy(
    runner: *const tswn_runner_t,
    group_index: usize,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let input_groups = unsafe { &(*runner).inner.input_groups };
        let group = input_groups.get(group_index).cloned().unwrap_or_default();
        copy_ids(&group, out_ids, cap, "out_ids")
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_player_input_group_index(
    runner: *const tswn_runner_t,
    player_id: u64,
    out_group_index: *mut usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_group_index.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_group_index is null"));
        }

        let player_id = player_id as PlrId;
        let input_groups = unsafe { &(*runner).inner.input_groups };
        let group_index = input_groups
            .iter()
            .position(|group| group.contains(&player_id))
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "player not found in input groups"))?;

        unsafe {
            *out_group_index = group_index;
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_winner_len(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.world.winner.as_ref().map(|w| w.len()).unwrap_or(0) }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_winner_copy(runner: *const tswn_runner_t, out_ids: *mut u64, cap: usize) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let winners = unsafe { (*runner).inner.world.winner.clone().unwrap_or_default() };
        copy_ids(&winners, out_ids, cap, "out_ids")
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_all_player_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.all_plr_len() }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_all_player_ids_copy(
    runner: *const tswn_runner_t,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let ids = unsafe { (*runner).inner.all_plrs() };
        copy_ids(&ids, out_ids, cap, "out_ids")
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_player_snapshot(
    runner: *const tswn_runner_t,
    player_id: u64,
    out_snapshot: *mut tswn_player_snapshot_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_snapshot.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_snapshot is null"));
        }
        let player = unsafe { (*runner).inner.storage.get_player(&(player_id as PlrId)) }
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "player not found"))?;
        unsafe {
            *out_snapshot = player_snapshot(player);
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_len(updates: *const tswn_updates_t) -> usize {
    if updates.is_null() {
        0
    } else {
        unsafe { (*updates).inner.updates.len() }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_get(
    updates: *const tswn_updates_t,
    index: usize,
    out_update: *mut tswn_update_snapshot_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "updates is null"));
        }
        if out_update.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_update is null"));
        }
        let updates_ref = unsafe { &(*updates).inner.updates };
        let update = updates_ref
            .get(index)
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "update index out of range"))?;
        unsafe {
            *out_update = update_snapshot(update);
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_targets_copy(
    updates: *const tswn_updates_t,
    index: usize,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "updates is null"));
        }
        let updates_ref = unsafe { &(*updates).inner.updates };
        let update = updates_ref
            .get(index)
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "update index out of range"))?;
        copy_ids(update.targets.as_slice(), out_ids, cap, "out_ids")
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_message(updates: *const tswn_updates_t, index: usize) -> tswn_str_t {
    if updates.is_null() {
        set_last_error("updates is null");
        return tswn_str_t::default();
    }
    let updates_ref = unsafe { &(*updates).inner.updates };
    match updates_ref.get(index) {
        Some(update) => into_tswn_str(update.message.to_string()),
        None => {
            set_last_error("update index out of range");
            tswn_str_t::default()
        }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_message_rendered(updates: *const tswn_updates_t, index: usize) -> tswn_str_t {
    if updates.is_null() {
        set_last_error("updates is null");
        return tswn_str_t::default();
    }
    let updates_ref = unsafe { &(*updates).inner.updates };
    match updates_ref.get(index) {
        Some(update) => into_tswn_str(update.msg()),
        None => {
            set_last_error("update index out of range");
            tswn_str_t::default()
        }
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_win_rate_with_eval_rq(raw_text_utf8, n, thread, crate::tswn_win_rate_eval_rq(), out_result) }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate_with_eval_rq(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_result.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let groups = Runner::split_namerena_into_groups(raw).0;
        ensure_groups_for_win_rate(&groups)?;
        let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_result = run_prepared_win_rate(&prepared, n, eval_rq, thread)?;
        }
        Ok(())
    })
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    out_results: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe {
        tswn_group_win_rate_with_eval_rq(
            target_utf8,
            against_utf8,
            against_len,
            n,
            thread,
            crate::tswn_win_rate_eval_rq(),
            out_results,
        )
    }
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate_with_eval_rq(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_results: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let target = unsafe { read_utf8(target_utf8, "target_utf8")? };
        if against_len > 0 && against_utf8.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "against_utf8 is null"));
        }
        if against_len > 0 && out_results.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_results is null"));
        }
        for index in 0..against_len {
            let opponent_ptr = unsafe { *against_utf8.add(index) };
            let opponent = unsafe { read_utf8(opponent_ptr, "against_utf8[index]")? };
            let raw = format!("{target}\n\n{opponent}");
            let groups = Runner::split_namerena_into_groups(raw).0;
            ensure_groups_for_win_rate(&groups)?;
            let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
                .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
            unsafe {
                *out_results.add(index) = run_prepared_win_rate(&prepared, n, eval_rq, thread)?;
            }
        }
        Ok(())
    })
}
