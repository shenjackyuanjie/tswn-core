//! tswn-capi — tswn-core 的 C ABI 动态库绑定。
//!
//! 提供稳定的 C 语言接口，供 FFI 调用方（C/C++/其他语言）调用战斗引擎。
//! 所有公开函数均以 `tswn_` 前缀命名，并遵循 `tswn_status_t` 约定返回状态码。

#![allow(non_camel_case_types, non_snake_case)]

mod high_api;
mod icon_api;
mod runner_api;

use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use tswn_core::engine::update::RunUpdates;
use tswn_core::{PreparedRunner, Runner};

const TSWN_CAPI_ABI_VERSION: u32 = 3;

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
    pub magic_point: i32,
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

pub(crate) struct FfiError {
    status: tswn_status_t,
    message: String,
}

pub(crate) type FfiResult<T> = Result<T, FfiError>;

pub(crate) fn ffi_error(status: tswn_status_t, message: impl Into<String>) -> FfiError {
    FfiError {
        status,
        message: message.into(),
    }
}

pub(crate) fn set_last_error(message: impl Into<String>) { LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(message.into())); }

fn clear_last_error() { LAST_ERROR.with(|slot| *slot.borrow_mut() = None); }

pub(crate) fn into_tswn_str(value: String) -> tswn_str_t {
    let boxed = value.into_bytes().into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *const u8 as *const c_char;
    tswn_str_t { ptr, len }
}

pub(crate) fn into_tswn_bytes(value: Vec<u8>) -> tswn_bytes_t {
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

pub(crate) fn ffi_boundary<F>(f: F) -> tswn_status_t
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

pub(crate) unsafe fn read_utf8(ptr: *const c_char, name: &str) -> FfiResult<String> {
    if ptr.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} is null")));
    }
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str
        .to_str()
        .map(|s| s.to_owned())
        .map_err(|_| ffi_error(tswn_status_t::TSWN_ERR_INVALID_UTF8, format!("{name} is not valid UTF-8")))
}

pub(crate) unsafe fn read_utf8_array(ptr: *const *const c_char, len: usize, name: &str) -> FfiResult<Vec<String>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} is null")));
    }
    let mut values = Vec::with_capacity(len);
    for index in 0..len {
        let item_ptr = unsafe { *ptr.add(index) };
        values.push(unsafe { read_utf8(item_ptr, &format!("{name}[{index}]"))? });
    }
    Ok(values)
}

pub(crate) fn write_string_result(out: *mut tswn_str_t, value: String) -> FfiResult<()> {
    if out.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
    }
    unsafe {
        *out = into_tswn_str(value);
    }
    Ok(())
}

pub(crate) fn write_json_result<T: serde::Serialize>(out: *mut tswn_str_t, value: &T) -> FfiResult<()> {
    let json = serde_json::to_string(value)
        .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_PANIC, format!("failed to serialize JSON: {err}")))?;
    write_string_result(out, json)
}

#[unsafe(no_mangle)]
pub extern "C" fn tswn_capi_abi_version() -> u32 { TSWN_CAPI_ABI_VERSION }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_capi_version() -> tswn_str_t { into_tswn_str(env!("CARGO_PKG_VERSION").to_owned()) }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_core_version() -> tswn_str_t { into_tswn_str(tswn_core::version().to_owned()) }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_default_eval_rq() -> f64 { tswn_core::player::eval_name::DEFAULT_EVAL_RQ }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_win_rate_eval_rq() -> f64 { tswn_core::player::eval_name::WIN_RATE_EVAL_RQ }

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
