//! C ABI 接入层：把 core 的战斗与工具 API 暴露为动态库或静态库。
//!
//! 本 crate 共享 core 战斗规则。C/C++ 调用方以分发包中的 `include/tswn_capi.h`
//! 为签名权威；Rust 应用直接依赖 `tswn_core` 更合适。
//! 当前 ABI 通过 [`tswn_capi_abi_version`] 查询，包装层和核心版本分别查询
//! [`tswn_capi_version`] / [`tswn_core_version`]。
//!
//! # 入口选择
//!
//! | 需求 | C 函数族 | 接入成本 |
//! | --- | --- | --- |
//! | 逐帧对局 | `tswn_battle_session_*` | 推荐；core 规范 JSON DTO |
//! | 一次完整回放 | `tswn_battle_replay_json` | 简单，但需保存整场回放 |
//! | 评分、胜率、DIY | `tswn_*_json` / `tswn_to_diy` | 高层校验与统一错误 |
//! | 自行控制低层对局 | `tswn_runner_*` / `tswn_prepared_*` | Advanced 句柄接口 |
//! | 图标 | `tswn_name_to_*` | RGBA、PNG 或 Base64 |
//!
//! # 资源的创建与释放
//!
//! - 输入为调用期间有效、以 NUL 结尾的 UTF-8 C 字符串；库不会在调用后保留这些指针。
//! - 输出切片按 `len` 读取，不保证 NUL 结尾；复制结构体不会复制分配或转移释放责任。
//! - 返回 [`tswn_str_t`] 用 [`tswn_str_free`]；返回 [`tswn_bytes_t`] 用 [`tswn_bytes_free`]。
//! - 会话和 runner 句柄使用对应 `*_free`；每块资源恰好释放一次。
//! - 使用同一份库的分配器完成创建与释放，保留原始 `ptr` / `len`，包括零长度缓冲。
//! - 输出槽不能覆盖尚未释放的缓冲或句柄；返回错误时仅在接口明确保证后才读取输出。
//! - 在一个执行上下文中推进同一可变句柄，并保持指针在调用期间有效。
//!
//! # 流式协议与错误
//!
//! `new → initial_states_json → next_frame_json 循环 → result_json → free`。
//! `next_frame_json` / `result_json` 通过 `has` 明确区分可选值。
//! 普通失败由 [`tswn_status_t`] 表示，再读取线程局部的
//! [`tswn_last_error_code`] / [`tswn_last_error_message`]；这些返回字符串同样需要释放。
//! 正常截断提供 result；sticky Runtime failure 保存在错误通道，处理后释放 session。
//! 返回状态码的入口捕获可展开的 Rust panic；`panic=abort`、分配失败和调用方指针
//! 违约不能转换为可恢复错误。部分低层布尔/消息入口不捕获 panic，详见各函数约定。
//!
//! options 的 `struct_size` 用于历史前缀兼容；用 `tswn_battle_options_default` 初始化，
//! 得到正确的结构尺寸。详细边界、完整可编译 C 示例及链接方法见
//! [C ABI 使用说明](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_capi)。
//! 默认 rustdoc 主要展示根类型；需要查看内部导出函数页时可加 `--document-private-items`。

#![allow(non_camel_case_types, non_snake_case)]

mod battle_api;
mod high_api;
mod icon_api;
mod runner_api;

use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use tswn_core::runtime::update::RunUpdates;
use tswn_core::{PreparedRunner, Runner};

const TSWN_CAPI_ABI_VERSION: u32 = 4;

thread_local! {
    static LAST_ERROR: RefCell<Option<LastError>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct LastError {
    code: &'static str,
    message: String,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// ABI 调用状态；仅 `TSWN_OK` 表示对应输出有效，详细原因从当前线程错误通道读取。
///
/// 同一状态可对应不同稳定错误码；不要按错误消息文本判断领域错误。
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
/// 本库分配的 UTF-8 字符串切片，不保证 NUL 结尾，必须按 `len` 读取。
///
/// 用 [`tswn_str_free`] 释放原始 `ptr` / `len`，不可修改长度后释放。即使长度为零，
/// 非空指针仍属于库分配；结构体的 `Copy` 只复制描述符，不产生第二份所有权。
pub struct tswn_str_t {
    pub ptr: *const c_char,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
/// 本库分配的只读二进制切片，以 [`tswn_bytes_free`] 释放原始指针和长度。
///
/// 缓冲与创建它的句柄独立；复制描述符不复制资源，同一分配只能释放一次。
pub struct tswn_bytes_t {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
/// 胜场与实际完成的试验总数；均为值拷贝，百分比由调用方在 total 非零时计算。
pub struct tswn_win_rate_result_t {
    pub wins: u64,
    pub total: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
/// 玩家属性的值拷贝，不借用 runner；`ptr` 为历史兼容的 id 别名，不是内存地址。
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
/// 单条更新的值拷贝；`has_param=0` 时忽略 param，目标数组通过 updates 接口另取。
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

/// 独占一场可变对局的 opaque 句柄；创建成功后由对应 runner free 释放。
pub struct tswn_runner_t {
    inner: Runner,
}

/// 固定评估口径的阵容模板；创建对局只借用模板，不转移其所有权。
pub struct tswn_prepared_runner_t {
    inner: PreparedRunner,
    eval_rq: f64,
}

/// 独立拥有一个主回合更新的 opaque 句柄，寿命不依赖原 runner。
pub struct tswn_updates_t {
    inner: RunUpdates,
}

pub(crate) struct FfiError {
    status: tswn_status_t,
    code: &'static str,
    message: String,
}

pub(crate) type FfiResult<T> = Result<T, FfiError>;

pub(crate) fn ffi_error(status: tswn_status_t, message: impl Into<String>) -> FfiError {
    ffi_error_with_code(status, error_code_for_status(status), message)
}

pub(crate) fn ffi_error_with_code(status: tswn_status_t, code: &'static str, message: impl Into<String>) -> FfiError {
    FfiError {
        status,
        code,
        message: message.into(),
    }
}

fn error_code_for_status(status: tswn_status_t) -> &'static str {
    match status {
        tswn_status_t::TSWN_ERR_NULL => tswn_core::cli_api::CliApiErrorCode::InvalidArgument.as_str(),
        tswn_status_t::TSWN_ERR_INVALID_UTF8 => tswn_core::cli_api::CliApiErrorCode::InvalidInput.as_str(),
        tswn_status_t::TSWN_ERR_INVALID_ARGUMENT => tswn_core::cli_api::CliApiErrorCode::InvalidArgument.as_str(),
        tswn_status_t::TSWN_ERR_RUNNER => tswn_core::cli_api::CliApiErrorCode::RunnerInitFailed.as_str(),
        tswn_status_t::TSWN_ERR_PANIC => tswn_core::cli_api::CliApiErrorCode::InternalError.as_str(),
        tswn_status_t::TSWN_OK => "",
    }
}

pub(crate) fn set_last_error(message: impl Into<String>) { set_last_error_with_code("INTERNAL_ERROR", message); }

pub(crate) fn set_last_error_with_code(code: &'static str, message: impl Into<String>) {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = Some(LastError {
            code,
            message: message.into(),
        });
    });
}

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
            set_last_error_with_code(err.code, err.message);
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

/// 返回 C ABI 版本，用于调用方核对头文件兼容性，不等同于包版本。
#[unsafe(no_mangle)]
pub extern "C" fn tswn_capi_abi_version() -> u32 { TSWN_CAPI_ABI_VERSION }

/// 返回包装层包版本的独立字符串；按 len 读取并用 [`tswn_str_free`] 释放。
#[unsafe(no_mangle)]
pub extern "C" fn tswn_capi_version() -> tswn_str_t { into_tswn_str(env!("CARGO_PKG_VERSION").to_owned()) }

/// 返回核心规则库版本的独立字符串；不会清除旧错误，结果需用 [`tswn_str_free`] 释放。
#[unsafe(no_mangle)]
pub extern "C" fn tswn_core_version() -> tswn_str_t { into_tswn_str(tswn_core::version().to_owned()) }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_default_eval_rq() -> f64 { tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ }

#[unsafe(no_mangle)]
pub extern "C" fn tswn_win_rate_eval_rq() -> f64 { tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ }

/// 复制当前线程最近的错误消息；没有错误时返回空字符串，读取不会消费或清除错误。
/// 每次调用均返回需由 [`tswn_str_free`] 释放的独立缓冲。
#[unsafe(no_mangle)]
pub extern "C" fn tswn_last_error_message() -> tswn_str_t {
    LAST_ERROR.with(|slot| into_tswn_str(slot.borrow().as_ref().map(|error| error.message.clone()).unwrap_or_default()))
}

pub(crate) unsafe fn read_f64_array(ptr: *const f64, len: usize, name: &str) -> FfiResult<Vec<f64>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} is null")));
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec())
}

/// 复制当前线程最近的稳定错误码；没有错误时为空，结果需用 [`tswn_str_free`] 释放。
/// 应在下一次状态型调用之前读取，避免该调用清除或替换错误。
#[unsafe(no_mangle)]
pub extern "C" fn tswn_last_error_code() -> tswn_str_t {
    LAST_ERROR.with(|slot| into_tswn_str(slot.borrow().as_ref().map(|error| error.code.to_owned()).unwrap_or_default()))
}

#[unsafe(no_mangle)]
pub extern "C" fn tswn_clear_error() { clear_last_error(); }

/// 释放库字符串；空指针无操作，零长度的非空缓冲仍会释放。
///
/// # Safety
///
/// 调用方必须保留本库返回的 `tswn_str_t` 的原始指针与长度，且同一分配只释放一次。
/// 对已释放、伪造或来自其他分配器的指针调用本函数会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_str_free(value: tswn_str_t) {
    unsafe {
        free_boxed_bytes(value.ptr as *const u8, value.len);
    }
}

/// 释放库字节缓冲；调用后描述符的所有副本均失效。
///
/// # Safety
///
/// 调用方必须保留本库返回的 `tswn_bytes_t` 的原始指针与长度，且同一分配只释放一次。
/// 对已释放、伪造或来自其他分配器的指针调用本函数会导致未定义行为。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_bytes_free(value: tswn_bytes_t) {
    unsafe {
        free_boxed_bytes(value.ptr, value.len);
    }
}
