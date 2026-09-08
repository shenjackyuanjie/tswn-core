//! 规范战斗流的 C 所有权与选项校验。
use crate::{
    FfiResult, ffi_boundary, ffi_error, high_api::cli_api_error, read_utf8, tswn_status_t, tswn_str_t, write_json_result,
};
use std::{ffi::c_char, mem::size_of, ptr};
use tswn_core::cli_api::battle::{BattleOptions, BattleSession, BattleStatus, BattleStopReason};

// 永久冻结的历史 V1 prefix；禁止增加字段。每个后续版本也须永久保留其 prefix size。
// 未来字段 offset 必须 >= BATTLE_OPTIONS_V1_SIZE，必要时显式 padding，禁止复用 V1 tail padding。
// 按 struct_size >= field_end_offset 单独读取扩展字段；缺失字段保留 core 默认值。
// 最小尺寸永远是 V1_SIZE，不能因 public struct 增长而拒绝旧 caller。
#[repr(C)]
#[derive(Clone, Copy)]
struct BattleOptionsV1 {
    struct_size: u32,
    eval_rq: f64,
    max_rounds: usize,
    include_icons: u8,
}
const BATTLE_OPTIONS_V1_SIZE: usize = size_of::<BattleOptionsV1>();

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tswn_battle_options_t {
    pub struct_size: u32,
    pub eval_rq: f64,
    pub max_rounds: usize,
    pub include_icons: u8,
}
impl Default for tswn_battle_options_t {
    fn default() -> Self {
        let defaults = BattleOptions::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            eval_rq: defaults.eval_rq,
            max_rounds: defaults.max_rounds,
            include_icons: u8::from(defaults.include_icons),
        }
    }
}
pub struct tswn_battle_session_t {
    inner: BattleSession,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum tswn_battle_status_t {
    TSWN_BATTLE_RUNNING = 0,
    TSWN_BATTLE_FINISHED = 1,
    TSWN_BATTLE_TRUNCATED = 2,
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum tswn_battle_stop_reason_t {
    TSWN_BATTLE_STOP_NONE = 0,
    TSWN_BATTLE_STOP_WINNER = 1,
    TSWN_BATTLE_STOP_MAX_ROUNDS = 2,
    TSWN_BATTLE_STOP_NO_PROGRESS = 3,
}

#[allow(
    clippy::field_reassign_with_default,
    reason = "历史 prefix 未包含的未来字段必须保留 core 默认值"
)]
unsafe fn read_options(options: *const tswn_battle_options_t) -> FfiResult<BattleOptions> {
    if options.is_null() {
        return Ok(BattleOptions::default());
    }
    let struct_size = unsafe { options.cast::<u32>().read_unaligned() } as usize;
    if struct_size < BATTLE_OPTIONS_V1_SIZE {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "battle options struct_size is too small",
        ));
    }
    let options = unsafe { options.cast::<BattleOptionsV1>().read_unaligned() };
    if options.include_icons > 1 {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "include_icons must be 0 or 1",
        ));
    }
    let mut result = BattleOptions::default();
    result.eval_rq = options.eval_rq;
    result.max_rounds = options.max_rounds;
    result.include_icons = options.include_icons != 0;
    Ok(result)
}
unsafe fn session_ref<'a>(session: *const tswn_battle_session_t) -> FfiResult<&'a BattleSession> {
    unsafe { session.as_ref() }
        .map(|s| &s.inner)
        .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_NULL, "session is null"))
}
unsafe fn reset_optional(out_has: *mut u8, out_json: *mut tswn_str_t) -> FfiResult<()> {
    if !out_has.is_null() {
        unsafe { out_has.write(0) };
    }
    if !out_json.is_null() {
        unsafe { out_json.write(tswn_str_t::default()) };
    }
    if out_has.is_null() || out_json.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "output pointer is null"));
    }
    Ok(())
}

/// # Safety
/// `options` 为 null，或至少指向 V1_SIZE 字节可写存储。
/// 此历史初始化函数永久只写 V1；未来更大的选项需新增带容量参数的初始化接口。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_options_default(options: *mut tswn_battle_options_t) {
    if !options.is_null() {
        let defaults = BattleOptions::default();
        let v1 = BattleOptionsV1 {
            struct_size: BATTLE_OPTIONS_V1_SIZE as u32,
            eval_rq: defaults.eval_rq,
            max_rounds: defaults.max_rounds,
            include_icons: u8::from(defaults.include_icons),
        };
        unsafe { options.cast::<BattleOptionsV1>().write_unaligned(v1) };
    }
}

/// # Safety
/// 输入是有效的 UTF-8 C 字符串；options 为 null，或至少有 struct_size 个可读字节。out_session 可写。
/// 返回的句柄必须恰好释放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_new(
    raw_text_utf8: *const c_char,
    options: *const tswn_battle_options_t,
    out_session: *mut *mut tswn_battle_session_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_session.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_session is null"));
        }
        unsafe { out_session.write(ptr::null_mut()) };
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let options = unsafe { read_options(options)? };
        let inner = BattleSession::new(&raw, options).map_err(cli_api_error)?;
        unsafe { out_session.write(Box::into_raw(Box::new(tswn_battle_session_t { inner }))) };
        Ok(())
    })
}

/// # Safety
/// session 为 null，或是由 new 返回的、归调用方所有的存活句柄；本次调用后不得再使用它。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_free(session: *mut tswn_battle_session_t) {
    if !session.is_null() {
        unsafe { drop(Box::from_raw(session)) };
    }
}

/// 查询 sticky Runtime failure；NULL 返回 0，不修改 last_error。
/// # Safety
/// session 为 NULL 或存活且可共享借用的句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_is_failed(session: *const tswn_battle_session_t) -> u8 {
    u8::from(unsafe { session.as_ref() }.is_some_and(|session| session.inner.is_failed()))
}

/// # Safety
/// session 存活且被独占借用；输出指针可写。返回的字符串归调用方所有，必须用 tswn_str_free 释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_next_frame_json(
    session: *mut tswn_battle_session_t,
    out_has_frame: *mut u8,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        unsafe { reset_optional(out_has_frame, out_json)? };
        let session = unsafe { session.as_mut() }.ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_NULL, "session is null"))?;
        if let Some(frame) = session.inner.next_frame().map_err(cli_api_error)? {
            write_json_result(out_json, &frame)?;
            unsafe { out_has_frame.write(1) };
        }
        Ok(())
    })
}

/// # Safety
/// session 存活；输出指针可写。用 tswn_str_free 释放返回的 JSON。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_result_json(
    session: *const tswn_battle_session_t,
    out_has_result: *mut u8,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        unsafe { reset_optional(out_has_result, out_json)? };
        if let Some(result) = unsafe { session_ref(session)? }.result() {
            write_json_result(out_json, &result)?;
            unsafe { out_has_result.write(1) };
        }
        Ok(())
    })
}

/// # Safety
/// session 存活且 out_json 可写。用 tswn_str_free 释放返回的 JSON。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_initial_states_json(
    session: *const tswn_battle_session_t,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_json.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_json is null"));
        }
        unsafe { out_json.write(tswn_str_t::default()) };
        write_json_result(out_json, &unsafe { session_ref(session)? }.initial_states())
    })
}

/// # Safety
/// session 存活且 out_json 可写。用 tswn_str_free 释放返回的 JSON。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_current_states_json(
    session: *const tswn_battle_session_t,
    out_json: *mut tswn_str_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_json.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_json is null"));
        }
        unsafe { out_json.write(tswn_str_t::default()) };
        write_json_result(out_json, &unsafe { session_ref(session)? }.current_states())
    })
}

/// # Safety
/// session 存活且 out_value 可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_status(
    session: *const tswn_battle_session_t,
    out_value: *mut tswn_battle_status_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_value.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_value is null"));
        }
        let inner = unsafe { session_ref(session)? };
        unsafe {
            out_value.write(match inner.status() {
                BattleStatus::Running => tswn_battle_status_t::TSWN_BATTLE_RUNNING,
                BattleStatus::Finished => tswn_battle_status_t::TSWN_BATTLE_FINISHED,
                BattleStatus::Truncated => tswn_battle_status_t::TSWN_BATTLE_TRUNCATED,
            })
        };
        Ok(())
    })
}

/// # Safety
/// session 存活且 out_value 可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_stop_reason(
    session: *const tswn_battle_session_t,
    out_value: *mut tswn_battle_stop_reason_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_value.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_value is null"));
        }
        let inner = unsafe { session_ref(session)? };
        unsafe {
            out_value.write(match inner.stop_reason() {
                None => tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_NONE,
                Some(BattleStopReason::Winner) => tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_WINNER,
                Some(BattleStopReason::MaxRounds) => tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_MAX_ROUNDS,
                Some(BattleStopReason::NoProgress) => tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_NO_PROGRESS,
            })
        };
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::ffi::CString;

    #[test]
    fn c_session_exposes_real_sticky_runtime_failure() {
        let raw = CString::new("alpha@red+bed2[3000]\n\nbeta@blue").unwrap();
        let mut handle = ptr::null_mut();
        unsafe {
            assert_eq!(tswn_battle_session_is_failed(ptr::null()), 0);
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), ptr::null(), &mut handle),
                tswn_status_t::TSWN_OK
            );
            assert_eq!(tswn_battle_session_is_failed(handle), 0);
            (*handle).inner.invalidate_runtime_for_test();
            let mut previous_error = None;
            for _ in 0..2 {
                let mut has = 99;
                let mut json = tswn_str_t::default();
                assert_eq!(
                    tswn_battle_session_next_frame_json(handle, &mut has, &mut json),
                    tswn_status_t::TSWN_ERR_RUNNER
                );
                assert_eq!(tswn_battle_session_is_failed(handle), 1);
                assert_eq!(has, 0);
                assert!(json.ptr.is_null() && json.len == 0);
                let code = crate::tswn_last_error_code();
                assert_eq!(std::slice::from_raw_parts(code.ptr.cast::<u8>(), code.len), b"RUNTIME_FAILED");
                crate::tswn_str_free(code);
                let message = crate::tswn_last_error_message();
                let text = std::slice::from_raw_parts(message.ptr.cast::<u8>(), message.len).to_vec();
                crate::tswn_str_free(message);
                if let Some(previous) = &previous_error {
                    assert_eq!(&text, previous);
                }
                previous_error = Some(text);
                assert_eq!(
                    tswn_battle_session_result_json(handle, &mut has, &mut json),
                    tswn_status_t::TSWN_OK
                );
                assert_eq!(has, 0);
                assert!(json.ptr.is_null() && json.len == 0);
                let mut reason = tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_WINNER;
                assert_eq!(tswn_battle_session_stop_reason(handle, &mut reason), tswn_status_t::TSWN_OK);
                assert_eq!(reason, tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_NONE);
                assert!(!(*handle).inner.is_done());
                assert_eq!((*handle).inner.status(), BattleStatus::Running);
            }
            tswn_battle_session_free(handle);
        }
    }

    // 保持独立于 public struct；以后 public struct 扩展也必须接受此历史 caller。
    #[repr(C)]
    struct SimulatedOldBattleOptionsV1 {
        struct_size: u32,
        eval_rq: f64,
        max_rounds: usize,
        include_icons: u8,
    }

    #[test]
    fn legacy_options_initializer_only_writes_frozen_v1() {
        let mut buffer = vec![0xA5u8; BATTLE_OPTIONS_V1_SIZE + 65];
        unsafe {
            let options = buffer.as_mut_ptr().add(1).cast();
            tswn_battle_options_default(options);
            assert_eq!(read_options(options).ok(), Some(BattleOptions::default()));
            assert_eq!(options.cast::<u32>().read_unaligned() as usize, BATTLE_OPTIONS_V1_SIZE);
        }
        assert_eq!(buffer[0], 0xA5);
        assert!(buffer[BATTLE_OPTIONS_V1_SIZE + 1..].iter().all(|&byte| byte == 0xA5));
    }

    #[test]
    fn options_v1_layout_is_frozen() {
        use std::mem::{align_of, offset_of};
        let eval_offset = 4usize.next_multiple_of(align_of::<f64>());
        let rounds_offset = eval_offset + 8;
        let icons_offset = rounds_offset + size_of::<usize>();
        let v1_size = (icons_offset + 1).next_multiple_of(align_of::<BattleOptionsV1>());
        assert_eq!(offset_of!(BattleOptionsV1, struct_size), 0);
        assert_eq!(offset_of!(BattleOptionsV1, eval_rq), eval_offset);
        assert_eq!(offset_of!(BattleOptionsV1, max_rounds), rounds_offset);
        assert_eq!(offset_of!(BattleOptionsV1, include_icons), icons_offset);
        assert_eq!(BATTLE_OPTIONS_V1_SIZE, v1_size);
        assert_eq!(size_of::<SimulatedOldBattleOptionsV1>(), v1_size);
        assert_eq!(offset_of!(tswn_battle_options_t, struct_size), 0);
        assert_eq!(offset_of!(tswn_battle_options_t, eval_rq), eval_offset);
        assert_eq!(offset_of!(tswn_battle_options_t, max_rounds), rounds_offset);
        assert_eq!(offset_of!(tswn_battle_options_t, include_icons), icons_offset);
        #[cfg(target_pointer_width = "64")]
        assert_eq!((eval_offset, rounds_offset, icons_offset, v1_size), (8, 16, 24, 32));
    }

    #[test]
    fn options_accept_null_exact_v1_old_caller_and_unaligned_future_buffer() {
        let raw = CString::new("a\n\nb").unwrap();
        let expected = BattleOptions {
            eval_rq: 0.5,
            max_rounds: 1,
            include_icons: true,
        };
        let old = SimulatedOldBattleOptionsV1 {
            struct_size: size_of::<SimulatedOldBattleOptionsV1>() as u32,
            eval_rq: expected.eval_rq,
            max_rounds: expected.max_rounds,
            include_icons: 1,
        };
        unsafe {
            assert_eq!(
                read_options(ptr::null()).ok().expect("valid V1 options"),
                BattleOptions::default()
            );
            let mut exact = tswn_battle_options_t::default();
            tswn_battle_options_default(&mut exact);
            assert_eq!(exact.struct_size as usize, BATTLE_OPTIONS_V1_SIZE);
            let mut handle = ptr::null_mut();
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), &exact, &mut handle),
                tswn_status_t::TSWN_OK
            );
            tswn_battle_session_free(handle);
            let old_ptr = (&old as *const SimulatedOldBattleOptionsV1).cast();
            assert_eq!(read_options(old_ptr).ok().expect("valid V1 options"), expected);

            // 多分配一字节，故意使用非对齐地址；所有 padding 和未来尾部初始化为非零。
            let mut buffer = vec![0xA5u8; BATTLE_OPTIONS_V1_SIZE + 65];
            let future = buffer.as_mut_ptr().add(1).cast::<BattleOptionsV1>();
            ptr::addr_of_mut!((*future).struct_size).write_unaligned((BATTLE_OPTIONS_V1_SIZE + 64) as u32);
            ptr::addr_of_mut!((*future).eval_rq).write_unaligned(old.eval_rq);
            ptr::addr_of_mut!((*future).max_rounds).write_unaligned(old.max_rounds);
            ptr::addr_of_mut!((*future).include_icons).write_unaligned(old.include_icons);
            assert_eq!(read_options(future.cast()).ok().expect("valid V1 options"), expected);
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), future.cast(), &mut handle),
                tswn_status_t::TSWN_OK
            );
            let mut canonical = BattleSession::new("a\n\nb", expected).unwrap();
            assert_eq!((*handle).inner.initial_states(), canonical.initial_states());
            assert_eq!((*handle).inner.next_frame().unwrap(), canonical.next_frame().unwrap());
            assert_eq!((*handle).inner.result(), canonical.result());
            tswn_battle_session_free(handle);
        }
    }

    #[test]
    fn options_reject_one_byte_short_of_v1_with_stable_code() {
        let raw = CString::new("a\n\nb").unwrap();
        let buffer = vec![(BATTLE_OPTIONS_V1_SIZE - 1) as u32; BATTLE_OPTIONS_V1_SIZE.div_ceil(4)];
        let mut handle = ptr::null_mut();
        unsafe {
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), buffer.as_ptr().cast(), &mut handle),
                tswn_status_t::TSWN_ERR_INVALID_ARGUMENT
            );
            assert!(handle.is_null());
            let code = crate::tswn_last_error_code();
            assert_eq!(std::slice::from_raw_parts(code.ptr.cast::<u8>(), code.len), b"INVALID_ARGUMENT");
            crate::tswn_str_free(code);
        }
    }

    unsafe fn take_json(value: tswn_str_t) -> Value {
        let json = serde_json::from_slice(unsafe { std::slice::from_raw_parts(value.ptr.cast(), value.len) }).unwrap();
        unsafe { crate::tswn_str_free(value) };
        json
    }

    #[test]
    fn c_session_matches_canonical_frames_and_result() {
        let fixtures = [
            include_str!("../../tswn_test/cases/runtime_stress/1v1-0f92cb76cc37fdc5.txt"),
            include_str!("../../tswn_test/cases/runtime_stress/2v2-554f4128af707167.txt"),
            include_str!("../../tswn_test/cases/runtime_stress/ffa_8-16d11de1ebe1df41.txt"),
            include_str!("../../tswn_test/cases/runtime_stress/3v3v3-0ace5df17b84e26a.txt"),
        ];
        for raw in fixtures {
            for max_rounds in [1, 20_000] {
                let mut canonical = BattleSession::new(
                    raw,
                    BattleOptions {
                        max_rounds,
                        ..BattleOptions::default()
                    },
                )
                .unwrap();
                let options = tswn_battle_options_t {
                    max_rounds,
                    ..Default::default()
                };
                let raw = CString::new(raw).unwrap();
                let mut handle = ptr::null_mut();
                unsafe {
                    assert_eq!(
                        tswn_battle_session_new(raw.as_ptr(), &options, &mut handle),
                        tswn_status_t::TSWN_OK
                    );
                    let mut json = tswn_str_t::default();
                    assert_eq!(
                        tswn_battle_session_initial_states_json(handle, &mut json),
                        tswn_status_t::TSWN_OK
                    );
                    assert_eq!(take_json(json), serde_json::to_value(canonical.initial_states()).unwrap());
                    let mut has = 99;
                    assert_eq!(
                        tswn_battle_session_result_json(handle, &mut has, &mut json),
                        tswn_status_t::TSWN_OK
                    );
                    assert_eq!(has, 0);
                    assert!(json.ptr.is_null() && json.len == 0);
                    while let Some(frame) = canonical.next_frame().unwrap() {
                        assert_eq!(
                            tswn_battle_session_next_frame_json(handle, &mut has, &mut json),
                            tswn_status_t::TSWN_OK
                        );
                        assert_eq!(has, 1);
                        assert_eq!(take_json(json), serde_json::to_value(frame).unwrap());
                    }
                    for _ in 0..3 {
                        assert_eq!(
                            tswn_battle_session_next_frame_json(handle, &mut has, &mut json),
                            tswn_status_t::TSWN_OK
                        );
                        assert_eq!(has, 0);
                        assert!(json.ptr.is_null() && json.len == 0);
                    }
                    assert_eq!(
                        tswn_battle_session_result_json(handle, &mut has, &mut json),
                        tswn_status_t::TSWN_OK
                    );
                    assert_eq!(has, 1);
                    assert_eq!(take_json(json), serde_json::to_value(canonical.result().unwrap()).unwrap());
                    assert_eq!(
                        tswn_battle_session_current_states_json(handle, &mut json),
                        tswn_status_t::TSWN_OK
                    );
                    assert_eq!(take_json(json), serde_json::to_value(canonical.current_states()).unwrap());
                    let mut status = tswn_battle_status_t::TSWN_BATTLE_RUNNING;
                    let mut reason = tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_NONE;
                    assert_eq!(tswn_battle_session_status(handle, &mut status), tswn_status_t::TSWN_OK);
                    assert_eq!(tswn_battle_session_stop_reason(handle, &mut reason), tswn_status_t::TSWN_OK);
                    assert_eq!(
                        status,
                        if canonical.is_finished() {
                            tswn_battle_status_t::TSWN_BATTLE_FINISHED
                        } else {
                            tswn_battle_status_t::TSWN_BATTLE_TRUNCATED
                        }
                    );
                    assert_ne!(reason, tswn_battle_stop_reason_t::TSWN_BATTLE_STOP_NONE);
                    tswn_battle_session_free(handle);
                }
            }
        }
    }

    #[test]
    fn versioned_options_accept_future_tails_and_reject_short_prefix() {
        assert_eq!(crate::tswn_capi_abi_version(), 4);
        let raw = CString::new("a\n\nb").unwrap();
        let mut handle = ptr::null_mut();
        unsafe {
            // 仅存在前缀：拒绝它时不得读取到这块分配之外。
            let short = 4u32;
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), (&short as *const u32).cast(), &mut handle),
                tswn_status_t::TSWN_ERR_INVALID_ARGUMENT
            );
            assert!(handle.is_null());
            let mut options = tswn_battle_options_t::default();
            options.struct_size += 16;
            #[repr(C)]
            struct FutureOptions {
                options: tswn_battle_options_t,
                tail: [u8; 16],
            }
            let future = FutureOptions { options, tail: [0; 16] };
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), &future.options, &mut handle),
                tswn_status_t::TSWN_OK
            );
            tswn_battle_session_free(handle);
            for bad in [
                tswn_battle_options_t {
                    max_rounds: 0,
                    ..Default::default()
                },
                tswn_battle_options_t {
                    include_icons: 2,
                    ..Default::default()
                },
            ] {
                assert_eq!(
                    tswn_battle_session_new(raw.as_ptr(), &bad, &mut handle),
                    tswn_status_t::TSWN_ERR_INVALID_ARGUMENT
                );
                assert!(handle.is_null());
            }
            tswn_battle_options_default(ptr::null_mut());
            tswn_battle_options_default(&mut options);
            assert_eq!(options.struct_size as usize, size_of::<tswn_battle_options_t>());
            assert_eq!(options.max_rounds, 20_000);
        }
    }

    #[test]
    fn null_outputs_do_not_advance_session_and_absent_outputs_are_reset() {
        let raw = CString::new("a\n\nb").unwrap();
        let mut handle = ptr::null_mut();
        unsafe {
            assert_eq!(
                tswn_battle_session_new(raw.as_ptr(), ptr::null(), &mut handle),
                tswn_status_t::TSWN_OK
            );
            let mut has = 55;
            assert_eq!(
                tswn_battle_session_next_frame_json(handle, &mut has, ptr::null_mut()),
                tswn_status_t::TSWN_ERR_NULL
            );
            assert_eq!(has, 0);
            assert_eq!((*handle).inner.rounds_advanced(), 0);
            let mut json = tswn_str_t::default();
            assert_eq!(
                tswn_battle_session_next_frame_json(ptr::null_mut(), &mut has, &mut json),
                tswn_status_t::TSWN_ERR_NULL
            );
            assert!(json.ptr.is_null() && json.len == 0);
            tswn_battle_session_free(handle);
            tswn_battle_session_free(ptr::null_mut());
        }
    }
}
