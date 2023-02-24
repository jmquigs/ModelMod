/*
Contains combinations of types from both DX9 and 11, most notably `HookDeviceState` which
carries the device specific state for one or the other (but not both) at runtime.
 */
use winapi::shared::windef::HWND;
use crate::types_dx9::HookDirect3D9Device;
use crate::types_dx9::HookDirect3D9;
use crate::types_dx11::HookDirect3D911Context;

pub struct HookD3D9State {
    pub d3d9: Option<HookDirect3D9>,
    pub device: Option<HookDirect3D9Device>,
}

pub struct HookD3D11State {
    pub context: HookDirect3D911Context,
}

pub enum HookDeviceState {
    D3D9(HookD3D9State),
    D3D11(HookD3D11State),
}

pub struct DeviceState {
    pub hook: Option<HookDeviceState>,
    pub d3d_window: HWND,
    pub d3d_resource_count: u32, // TODO: this should be tracked per device pointer.
}
