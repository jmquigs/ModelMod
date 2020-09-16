// This module provides hook functions to store constants as they are set, rather than grabbing
// a range of them at snapshot time.  
//
// Currently this is disabled (not compiled) because:
// 1) some games won't use the Set* functions to set constants, I guess they are modifying the 
// constant table in the shader or something.
// 2) Grabbing them as they are set doesn't really help since you can't really be sure what the 
// "active" set of constants is, at DIP time (without inspecting the shaders), 
// you'd have to consider any constant that was set previously as still valid, so you still 
// end up with a lot of extra constants.
// 3) Grabbing them as they are set is probably slower.

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use hookd3d9::{ dev_state, GLOBAL_STATE };
use shared_dx9::util;

pub use std::collections::HashMap;

pub unsafe extern "system" fn hook_set_vertex_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    util::write_log_file(&format!("hook_set_vertex_sc_f: {} {}", StartRegister, Vector4fCount));
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount);
    if hr == 0 {
        let is_snapping = GLOBAL_STATE.is_snapping;
        if is_snapping && Vector4fCount > 0 {
            util::write_log_file(&format!("snapping vf const {}, {} count: {} {} {} {}...",
                StartRegister,
                Vector4fCount,
                *pConstantData,
                *pConstantData.offset(1),
                *pConstantData.offset(2),
                *pConstantData.offset(3),
            ));
        }
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.floats.set(StartRegister, pConstantData, Vector4fCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_vertex_sc_i(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_i)(THIS, StartRegister, pConstantData, Vector4iCount);
    if hr == 0 {
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.ints.set(StartRegister, pConstantData, Vector4iCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_vertex_sc_b(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_b)(THIS, StartRegister, pConstantData, BoolCount);
    if hr == 0 {
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.bools.set(StartRegister, pConstantData, BoolCount);
        });
    }
    hr
}
// pixel functions:
pub unsafe extern "system" fn hook_set_pixel_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.floats.set(StartRegister, pConstantData, Vector4fCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_pixel_sc_i(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_i)(THIS, StartRegister, pConstantData, Vector4iCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.ints.set(StartRegister, pConstantData, Vector4iCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_pixel_sc_b(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_b)(THIS, StartRegister, pConstantData, BoolCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.bools.set(StartRegister, pConstantData, BoolCount);
        });
    }
    hr
}
