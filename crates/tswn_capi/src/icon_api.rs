//! 图标导出：状态型接口返回 RGBA/PNG 缓冲，便捷接口返回 PNG data URL。
//!
//! 输入仅在调用期间借用，输出均由本库分配；错误通过线程局部通道查询。
use std::ffi::c_char;

use crate::{ffi_boundary, ffi_error, into_tswn_bytes, into_tswn_str, read_utf8, tswn_bytes_t, tswn_status_t, tswn_str_t};

/// 将原始名字渲染为按行排列的 16×16 RGBA 像素，共 1024 字节。
///
/// 成功后输出独立缓冲，调用方用 `tswn_bytes_free` 释放；失败时输出保持原值。
///
/// # Safety
///
/// `name_utf8` 指向调用期间有效的 NUL 结尾 C 字符串；非空 `out_bytes` 必须对齐且可写。
/// 输出位置不得持有尚未释放的库缓冲，也不得与输入重叠。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_icon_rgba(name_utf8: *const c_char, out_bytes: *mut tswn_bytes_t) -> tswn_status_t {
    ffi_boundary(|| {
        if out_bytes.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_bytes is null"));
        }
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        unsafe {
            *out_bytes = into_tswn_bytes(tswn_core::namerena::icon_render::render_icon_vec_from_name(&name));
        }
        Ok(())
    })
}

/// 将原始名字渲染为完整 PNG 文件字节，不含 Base64 或 data URL 前缀。
///
/// 成功后输出独立缓冲，调用方用 `tswn_bytes_free` 释放；失败时输出保持原值。
///
/// # Safety
///
/// `name_utf8` 指向调用期间有效的 NUL 结尾 C 字符串；非空 `out_bytes` 必须对齐且可写。
/// 输出位置不得持有尚未释放的库缓冲，也不得与输入重叠。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_png_bytes(name_utf8: *const c_char, out_bytes: *mut tswn_bytes_t) -> tswn_status_t {
    ffi_boundary(|| {
        if out_bytes.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_bytes is null"));
        }
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        unsafe {
            *out_bytes = into_tswn_bytes(tswn_core::namerena::icon_render::render_icon_png_from_name(&name));
        }
        Ok(())
    })
}

/// 返回 `data:image/png;base64,...` 格式的图标文本，而非裸 Base64。
///
/// 成功会清除当前线程的旧错误；输入错误或可捕获的 panic 返回 `{NULL, 0}`，
/// 可用 `tswn_last_error_code` / `tswn_last_error_message` 读取原因。成功结果按 `len`
/// 读取，不保证 NUL 结尾，且必须用 `tswn_str_free` 恰好释放一次。
///
/// # Safety
///
/// 非空 `name_utf8` 必须指向调用期间有效的 NUL 结尾 C 字符串；空指针返回错误。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_name_to_png_base64(name_utf8: *const c_char) -> tswn_str_t {
    let mut result = tswn_str_t::default();
    ffi_boundary(|| {
        let name = unsafe { read_utf8(name_utf8, "name_utf8")? };
        result = into_tswn_str(tswn_core::namerena::icon_render::render_icon_b64_from_name(&name));
        Ok(())
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error_code() -> String {
        let value = crate::tswn_last_error_code();
        let code = unsafe { std::slice::from_raw_parts(value.ptr.cast::<u8>(), value.len) };
        let code = std::str::from_utf8(code).unwrap().to_owned();
        unsafe { crate::tswn_str_free(value) };
        code
    }

    #[test]
    fn base64_preserves_input_error_codes() {
        let invalid_utf8 = [0xff_u8, 0];
        let value = unsafe { tswn_name_to_png_base64(invalid_utf8.as_ptr().cast()) };
        assert!(value.ptr.is_null() && value.len == 0);
        assert_eq!(last_error_code(), "INVALID_INPUT");

        let value = unsafe { tswn_name_to_png_base64(std::ptr::null()) };
        assert!(value.ptr.is_null() && value.len == 0);
        assert_eq!(last_error_code(), "INVALID_ARGUMENT");
    }

    #[test]
    fn base64_success_clears_previous_error_and_returns_owned_png() {
        crate::set_last_error("先前调用的错误");
        let value = unsafe { tswn_name_to_png_base64(c"alpha".as_ptr()) };
        assert!(!value.ptr.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(value.ptr.cast::<u8>(), value.len) };
        assert!(bytes.starts_with(b"data:image/png;base64,iVBORw0KGgo"));
        unsafe { crate::tswn_str_free(value) };
        assert_eq!(last_error_code(), "");
    }
}
