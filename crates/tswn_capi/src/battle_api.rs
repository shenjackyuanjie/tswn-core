//! C ownership and options validation for the canonical battle stream.
use crate::{
    FfiResult, ffi_boundary, ffi_error, high_api::cli_api_error, read_utf8, tswn_status_t, tswn_str_t, write_json_result,
};
use std::{ffi::c_char, mem::size_of, ptr};
use tswn_core::cli_api::battle::{BattleOptions, BattleSession, BattleStatus, BattleStopReason};

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

unsafe fn read_options(options: *const tswn_battle_options_t) -> FfiResult<BattleOptions> {
    if options.is_null() {
        return Ok(BattleOptions::default());
    }
    let struct_size = unsafe { options.cast::<u32>().read() };
    if struct_size < size_of::<tswn_battle_options_t>() as u32 {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "battle options struct_size is too small",
        ));
    }
    let options = unsafe { options.read() };
    if options.include_icons > 1 {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "include_icons must be 0 or 1",
        ));
    }
    Ok(BattleOptions {
        eval_rq: options.eval_rq,
        max_rounds: options.max_rounds,
        include_icons: options.include_icons != 0,
    })
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
/// `options` is null or points to writable storage for the current options struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_options_default(options: *mut tswn_battle_options_t) {
    if !options.is_null() {
        unsafe { options.write(tswn_battle_options_t::default()) };
    }
}

/// # Safety
/// Input is a valid UTF-8 C string; options is null or has at least struct_size readable
/// bytes. out_session is writable. The returned handle must be freed exactly once.
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
/// session is null or an owned, live handle returned by new, not used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_battle_session_free(session: *mut tswn_battle_session_t) {
    if !session.is_null() {
        unsafe { drop(Box::from_raw(session)) };
    }
}

/// # Safety
/// session is live and exclusively borrowed; output pointers are writable.
/// A returned string belongs to the caller and must be released with tswn_str_free.
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
/// session is live; output pointers are writable. Free returned JSON with tswn_str_free.
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
/// session is live and out_json is writable. Free returned JSON with tswn_str_free.
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
/// session is live and out_json is writable. Free returned JSON with tswn_str_free.
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
/// session is live and out_value is writable.
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
/// session is live and out_value is writable.
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
            // Only the prefix exists: rejecting it must not read beyond this allocation.
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
