
// This lib is disabled for now since I don't use this.
// Constants are captured in one shot during snapshot.
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use global_state::GLOBAL_STATE;

use device_state::dev_state;

pub unsafe extern "system" fn hook_set_vertex_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount);
    if hr == 0 {
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
