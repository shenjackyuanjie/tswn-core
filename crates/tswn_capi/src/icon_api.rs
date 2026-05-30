use std::ffi::c_char;

use crate::{
    ffi_boundary, ffi_error, into_tswn_bytes, into_tswn_str, read_utf8, set_last_error, tswn_bytes_t, tswn_status_t, tswn_str_t,
};

/// # Safety
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
