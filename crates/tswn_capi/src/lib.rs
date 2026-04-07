#![allow(non_camel_case_types, non_snake_case)]

use std::cell::RefCell;

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use tswn_core::engine::update::{RunUpdate, RunUpdates, UpdateType};
use tswn_core::player::PlrId;
use tswn_core::{PreparedRunner, Runner};

const PROFILE_WINRATE_SEED_START: usize = 33_554_431;
const TSWN_CAPI_ABI_VERSION: u32 = 1;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum tswn_status_t {
    TSWN_OK = 0,
    TSWN_ERR_NULL = 1,
    TSWN_ERR_INVALID_UTF8 = 2,
    TSWN_ERR_INVALID_ARGUMENT = 3,
    TSWN_ERR_RUNNER = 4,
    TSWN_ERR_PANIC = 255,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tswn_str_t {
    pub ptr: *const c_char,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tswn_bytes_t {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tswn_win_rate_result_t {
    pub wins: u64,
    pub total: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tswn_player_snapshot_t {
    pub id: u64,
    pub ptr: u64,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub move_point: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub agility: i32,
    pub magic: i32,
    pub resistance: i32,
    pub wisdom: i32,
    pub point: u32,
    pub all_sum: u32,
    pub name_factor: f64,
    pub at_boost: f64,
    pub attract: f64,
    pub frozen: u8,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default)]
pub enum tswn_update_type_t {
    Win  = 0,
    None = 1,
    #[default]
    NextLine = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tswn_update_snapshot_t {
    pub score: u32,
    pub param: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub caster_id: u64,
    pub target_id: u64,
    pub target_count: usize,
    pub has_param: u8,
    pub update_type: tswn_update_type_t,
}

pub struct tswn_runner_t {
    inner: Runner,
}

pub struct tswn_prepared_runner_t {
    inner: PreparedRunner,
}

pub struct tswn_updates_t {
    inner: RunUpdates,
}

struct FfiError {
    status: tswn_status_t,
    message: String,
}

type FfiResult<T> = Result<T, FfiError>;

fn ffi_error(status: tswn_status_t, message: impl Into<String>) -> FfiError {
    FfiError {
        status,
        message: message.into(),
    }
}

fn set_last_error(message: impl Into<String>) { LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(message.into())); }

fn clear_last_error() { LAST_ERROR.with(|slot| *slot.borrow_mut() = None); }

fn use_js_profile_seed_schedule(eval_rq: f64) -> bool { eval_rq == tswn_core::player::eval_name::WIN_RATE_EVAL_RQ }

fn update_type_to_c(update_type: UpdateType) -> tswn_update_type_t {
    match update_type {
        UpdateType::Win => tswn_update_type_t::Win,
        UpdateType::None => tswn_update_type_t::None,
        UpdateType::NextLine => tswn_update_type_t::NextLine,
    }
}

fn into_tswn_str(value: String) -> tswn_str_t {
    let boxed = value.into_bytes().into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *const u8 as *const c_char;
    tswn_str_t { ptr, len }
}

fn into_tswn_bytes(value: Vec<u8>) -> tswn_bytes_t {
    let boxed = value.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *const [u8] as *const u8;
    tswn_bytes_t { ptr, len }
}

unsafe fn free_boxed_bytes(ptr: *const u8, len: usize) {
    if !ptr.is_null() {
        let slice_ptr = ptr::slice_from_raw_parts_mut(ptr as *mut u8, len);
        unsafe {
            drop(Box::from_raw(slice_ptr));
        }
    }
}

fn ffi_boundary<F>(f: F) -> tswn_status_t
where
    F: FnOnce() -> FfiResult<()>,
{
    clear_last_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => tswn_status_t::TSWN_OK,
        Ok(Err(err)) => {
            set_last_error(err.message);
            err.status
        }
        Err(_) => {
            set_last_error("panic crossed FFI boundary");
            tswn_status_t::TSWN_ERR_PANIC
        }
    }
}

unsafe fn read_utf8(ptr: *const c_char, name: &str) -> FfiResult<String> {
    if ptr.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} is null")));
    }
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str
        .to_str()
        .map(|s| s.to_owned())
        .map_err(|_| ffi_error(tswn_status_t::TSWN_ERR_INVALID_UTF8, format!("{name} is not valid UTF-8")))
}

unsafe fn read_optional_utf8(ptr: *const c_char, name: &str) -> FfiResult<Vec<String>> {
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

fn player_snapshot(player: &tswn_core::player::Player) -> tswn_player_snapshot_t {
    let status = player.get_status();
    tswn_player_snapshot_t {
        id: player.id(),
        ptr: player.ptr() as u64,
        hp: status.hp,
        max_hp: status.max_hp,
        mp: status.mp,
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

fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, eval_rq: f64) -> FfiResult<tswn_win_rate_result_t> {
    let mut wins = 0u64;
    for i in 0..n {
        let seed = if use_js_profile_seed_schedule(eval_rq) {
            if i == 0 {
                Vec::new()
            } else {
                vec![format!("seed:{}@!", PROFILE_WINRATE_SEED_START + i)]
            }
        } else {
            vec![format!("seed:{i}@!")]
        };
        let mut runner = Runner::new_from_prepared_with_seed(prepared, &seed)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        let team0_roster = runner.input_groups.first().cloned().unwrap_or_default();
        runner.run_to_completion();
        if let Some(winners) = runner.world.winner.as_ref()
            && winners.iter().any(|winner| team0_roster.contains(winner))
        {
            wins += 1;
        }
    }
    Ok(tswn_win_rate_result_t { wins, total: n as u64 })
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

#[unsafe(no_mangle)]
pub extern "C" fn tswn_capi_abi_version() -> u32 { TSWN_CAPI_ABI_VERSION }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_version() -> tswn_str_t { into_tswn_str(tswn_core::version().to_owned()) }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_default_eval_rq() -> f64 { tswn_core::player::eval_name::DEFAULT_EVAL_RQ }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_win_rate_eval_rq() -> f64 { tswn_core::player::eval_name::WIN_RATE_EVAL_RQ }

/// # Safety
///
/// `prepared` 必须是由本库返回且仍然有效的 `tswn_prepared_runner_t` 句柄；
/// `out_result` 必须指向一个可写的 `tswn_win_rate_result_t` 输出位置。
/// 若 `prepared` 为 `NULL`、`out_result` 为 `NULL`，或传入悬空/伪造指针，会导致错误返回或未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_prepared_win_rate_with_eval_rq(prepared, n, tswn_win_rate_eval_rq(), out_result) }
}

/// # Safety
///
/// `prepared` 必须是由本库返回且仍然有效的 `tswn_prepared_runner_t` 句柄；
/// `out_result` 必须指向一个可写的 `tswn_win_rate_result_t` 输出位置。
/// 若 `prepared` 为 `NULL`、`out_result` 为 `NULL`，或传入悬空/伪造指针，会导致错误返回或未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate_with_eval_rq(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
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
        let result = run_prepared_win_rate(unsafe { &(*prepared).inner }, n, eval_rq)?;
        unsafe {
            *out_result = result;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tswn_last_error_message() -> tswn_str_t {
    LAST_ERROR.with(|slot| into_tswn_str(slot.borrow().clone().unwrap_or_default()))
}

#[unsafe(no_mangle)]
pub extern "C" fn tswn_clear_error() { clear_last_error(); }

/// # Safety
///
/// 调用方必须保证 `value` 来自本库返回的 `tswn_str_t`，且同一块内存只释放一次。
/// 对已释放、伪造或来自其他分配器的指针调用本函数会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_str_free(value: tswn_str_t) {
    unsafe {
        free_boxed_bytes(value.ptr as *const u8, value.len);
    }
}

/// # Safety
///
/// 调用方必须保证 `value` 来自本库返回的 `tswn_bytes_t`，且同一块内存只释放一次。
/// 对已释放、伪造或来自其他分配器的指针调用本函数会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_bytes_free(value: tswn_bytes_t) {
    unsafe {
        free_boxed_bytes(value.ptr, value.len);
    }
}

/// # Safety
///
/// 调用方必须保证 `ptr` 是由本库创建并返回的 `tswn_runner_t*`，并且只能释放一次。
/// 传入悬空指针、伪造指针或重复释放都会导致未定义行为。`NULL` 允许传入并被安全忽略。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_free(ptr: *mut tswn_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
///
/// 调用方必须保证 `ptr` 是由本库创建并返回的 `tswn_prepared_runner_t*`，并且只能释放一次。
/// 传入悬空指针、伪造指针或重复释放都会导致未定义行为。`NULL` 允许传入并被安全忽略。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_free(ptr: *mut tswn_prepared_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
///
/// 调用方必须保证 `ptr` 是由本库创建并返回的 `tswn_updates_t*`，并且只能释放一次。
/// 传入悬空指针、伪造指针或重复释放都会导致未定义行为。`NULL` 允许传入并被安全忽略。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_free(ptr: *mut tswn_updates_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// # Safety
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_runner` 必须是可写的有效二级指针地址，用于接收新建的 runner 句柄。
/// 调用方需要在不再使用时通过 `tswn_runner_free` 释放返回的句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    unsafe { tswn_runner_new_from_raw_with_eval_rq(raw_text_utf8, tswn_default_eval_rq(), out_runner) }
}

/// # Safety
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_runner` 必须是可写的有效二级指针地址，用于接收新建的 runner 句柄。
/// 调用方需要在不再使用时通过 `tswn_runner_free` 释放返回的句柄。
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
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_prepared` 必须是可写的有效二级指针地址，用于接收新建的 prepared 句柄。
/// 调用方需要在不再使用时通过 `tswn_prepared_runner_free` 释放返回的句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_prepared: *mut *mut tswn_prepared_runner_t,
) -> tswn_status_t {
    unsafe { tswn_prepared_runner_new_from_raw_with_eval_rq(raw_text_utf8, tswn_default_eval_rq(), out_prepared) }
}

/// # Safety
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_prepared` 必须是可写的有效二级指针地址，用于接收新建的 prepared 句柄。
/// 调用方需要在不再使用时通过 `tswn_prepared_runner_free` 释放返回的句柄。
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
///
/// `prepared` 必须是由本库返回且仍然有效的 prepared 句柄；
/// `seed_utf8` 为可选参数，若非 `NULL` 则必须指向一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_runner` 必须是可写的有效二级指针地址，用于接收新建的 runner 句柄。
/// 调用方需要在不再使用时通过 `tswn_runner_free` 释放返回的句柄。
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
        let seed = unsafe { read_optional_utf8(seed_utf8, "seed_utf8")? };
        let runner = Runner::new_from_prepared_with_seed(unsafe { &(*prepared).inner }, &seed)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_runner = Box::into_raw(Box::new(tswn_runner_t { inner: runner }));
        }
        Ok(())
    })
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`NULL` 会被视为“无赢家”并返回 `0`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_have_winner(runner: *const tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.have_winner() })
    }
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效、且可独占修改的 runner 句柄。
/// 若传入悬空指针、伪造指针或与其他线程/调用方并发共享同一 runner，会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_run_to_completion(runner: *mut tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.run_to_completion() })
    }
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效、且可独占修改的 runner 句柄；
/// `out_updates` 必须是可写的有效二级指针地址，用于接收新建的 updates 句柄。
/// 调用方需要在不再使用时通过 `tswn_updates_free` 释放返回的句柄。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`NULL` 会返回 `0`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.input_groups.len() }
    }
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`group_index` 越界时返回 `0`。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄；
/// `out_ids` 在 `cap > 0` 时必须指向一段至少可写入 `cap` 个 `u64` 的有效缓冲区。
/// 若传入悬空指针、伪造指针或错误容量，会导致未定义行为。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄；
/// `out_group_index` 必须指向一个可写的 `usize` 输出位置。
/// 若传入悬空指针、伪造指针或无效输出地址，会导致未定义行为。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`NULL` 会返回 `0`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_winner_len(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.world.winner.as_ref().map(|w| w.len()).unwrap_or(0) }
    }
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄；
/// `out_ids` 在 `cap > 0` 时必须指向一段至少可写入 `cap` 个 `u64` 的有效缓冲区。
/// 若传入悬空指针、伪造指针或错误容量，会导致未定义行为。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`NULL` 会返回 `0`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_all_player_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.all_plr_len() }
    }
}

/// # Safety
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄；
/// `out_ids` 在 `cap > 0` 时必须指向一段至少可写入 `cap` 个 `u64` 的有效缓冲区。
/// 若传入悬空指针、伪造指针或错误容量，会导致未定义行为。
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
///
/// `runner` 必须是由本库返回且仍然有效的 runner 句柄；
/// `out_snapshot` 必须指向一个可写的 `tswn_player_snapshot_t` 输出位置。
/// 若传入悬空指针、伪造指针或无效输出地址，会导致未定义行为。
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
///
/// `updates` 必须是由本库返回且仍然有效的 updates 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；`NULL` 会返回 `0`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_len(updates: *const tswn_updates_t) -> usize {
    if updates.is_null() {
        0
    } else {
        unsafe { (*updates).inner.updates.len() }
    }
}

/// # Safety
///
/// `updates` 必须是由本库返回且仍然有效的 updates 句柄；
/// `out_update` 必须指向一个可写的 `tswn_update_snapshot_t` 输出位置。
/// 若传入悬空指针、伪造指针或无效输出地址，会导致未定义行为。
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
///
/// `updates` 必须是由本库返回且仍然有效的 updates 句柄；
/// `out_ids` 在 `cap > 0` 时必须指向一段至少可写入 `cap` 个 `u64` 的有效缓冲区。
/// 若传入悬空指针、伪造指针或错误容量，会导致未定义行为。
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
///
/// `updates` 必须是由本库返回且仍然有效的 updates 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；返回的字符串需要由调用方用 `tswn_str_free` 释放。
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
///
/// `updates` 必须是由本库返回且仍然有效的 updates 句柄。
/// 若传入悬空指针或伪造指针会导致未定义行为；返回的字符串需要由调用方用 `tswn_str_free` 释放。
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
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_result` 必须指向一个可写的 `tswn_win_rate_result_t` 输出位置。
/// 若传入悬空指针、伪造指针或无效输出地址，会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate(
    raw_text_utf8: *const c_char,
    n: usize,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_win_rate_with_eval_rq(raw_text_utf8, n, tswn_win_rate_eval_rq(), out_result) }
}

/// # Safety
///
/// `raw_text_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_result` 必须指向一个可写的 `tswn_win_rate_result_t` 输出位置。
/// 若传入悬空指针、伪造指针或无效输出地址，会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate_with_eval_rq(
    raw_text_utf8: *const c_char,
    n: usize,
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
            *out_result = run_prepared_win_rate(&prepared, n, eval_rq)?;
        }
        Ok(())
    })
}

/// # Safety
///
/// `target_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// 当 `against_len > 0` 时，`against_utf8` 必须指向一个长度至少为 `against_len` 的有效指针数组，
/// 且其中每个元素都必须是有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_results` 在 `against_len > 0` 时必须指向一段至少可写入 `against_len` 个结果的有效缓冲区。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    out_results: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_group_win_rate_with_eval_rq(target_utf8, against_utf8, against_len, n, tswn_win_rate_eval_rq(), out_results) }
}

/// # Safety
///
/// `target_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// 当 `against_len > 0` 时，`against_utf8` 必须指向一个长度至少为 `against_len` 的有效指针数组，
/// 且其中每个元素都必须是有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_results` 在 `against_len > 0` 时必须指向一段至少可写入 `against_len` 个结果的有效缓冲区。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate_with_eval_rq(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
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
                *out_results.add(index) = run_prepared_win_rate(&prepared, n, eval_rq)?;
            }
        }
        Ok(())
    })
}

/// # Safety
///
/// `name_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_bytes` 必须指向一个可写的 `tswn_bytes_t` 输出位置。
/// 返回的字节缓冲区需要由调用方使用 `tswn_bytes_free` 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_icon_rgba(name_utf8: *const c_char, out_bytes: *mut tswn_bytes_t) -> tswn_status_t {
    ffi_boundary(|| {
        if out_bytes.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_bytes is null"));
        }
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        unsafe {
            *out_bytes = into_tswn_bytes(tswn_core::player::icon_render::render_icon_vec_from_name(&name));
        }
        Ok(())
    })
}

/// # Safety
///
/// `name_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串；
/// `out_bytes` 必须指向一个可写的 `tswn_bytes_t` 输出位置。
/// 返回的字节缓冲区需要由调用方使用 `tswn_bytes_free` 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_png_bytes(name_utf8: *const c_char, out_bytes: *mut tswn_bytes_t) -> tswn_status_t {
    ffi_boundary(|| {
        if out_bytes.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_bytes is null"));
        }
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        unsafe {
            *out_bytes = into_tswn_bytes(tswn_core::player::icon_render::render_icon_png_from_name(&name));
        }
        Ok(())
    })
}

/// # Safety
///
/// `name_utf8` 必须是一个有效的、以 `\0` 结尾的 UTF-8 C 字符串。
/// 若传入悬空指针、伪造指针或非 UTF-8 字符串会导致错误结果；返回的字符串需要由调用方使用 `tswn_str_free` 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_png_base64(name_utf8: *const c_char) -> tswn_str_t {
    match unsafe { read_utf8(name_utf8, "name_utf8") } {
        Ok(name) => into_tswn_str(tswn_core::player::icon_render::render_icon_b64_from_name(&name)),
        Err(err) => {
            set_last_error(err.message);
            tswn_str_t::default()
        }
    }
}
