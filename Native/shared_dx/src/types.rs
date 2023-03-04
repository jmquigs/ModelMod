/*
Contains combinations of types from both DX9 and 11, most notably `HookDeviceState` which
carries the device specific state for one or the other (but not both) at runtime.
 */
use std::ptr::null_mut;
 use winapi::shared::windef::HWND;
use winapi::shared::d3d9::IDirect3DDevice9;
use winapi::um::d3d11::ID3D11Device;
use crate::types_dx9::HookDirect3D9Device;
use crate::types_dx9::HookDirect3D9;
use crate::types_dx11::HookDirect3D11;

pub struct HookD3D9State {
    pub d3d9: Option<HookDirect3D9>,
    pub device: Option<HookDirect3D9Device>,
}

pub struct HookD3D11State {
    pub hooks: HookDirect3D11,
    /// In DX11 the device pointer is stored as part of this state because we generally
    /// do most of the work in device context functions, which don't get the device pointer.
    pub devptr: DevicePointer,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]

/// A copyable enum which stores a device pointer.  Used to pass through this pointer
/// to functions that don't need to know what the type is.  A function that actually needs
/// to do work with the pointer will need to match on the type implement code to handle each
/// API device type.
pub enum DevicePointer {
    D3D9(*mut IDirect3DDevice9),
    D3D11(*mut ID3D11Device),
}

impl DevicePointer {
    /// Return the current reference count of the device pointer.  If the pointer is null,
    /// returns zero.
    pub fn get_ref_count(&self) -> u32 {
        match self {
            DevicePointer::D3D9(d3d9) if *d3d9 != null_mut() =>
                unsafe { (**d3d9).AddRef(); (**d3d9).Release() },
            DevicePointer::D3D11(d3d11) if *d3d11 != null_mut() =>
                unsafe { (**d3d11).AddRef(); (**d3d11).Release() },
            _ => 0,
        }
    }
    /// Returns true if pointer is null.  Use in case some asshole (me) constructs this with a null pointer.
    pub fn is_null(&self) -> bool {
        match self {
            DevicePointer::D3D9(d3d9) if *d3d9 != null_mut() => false,
            DevicePointer::D3D11(d3d11) if *d3d11 != null_mut() => false,
            _ => true,
        }
    }
    /// Returns the pointer value as a u64.
    pub fn as_u64(&self) -> u64 {
        match self {
            DevicePointer::D3D9(d3d9) => *d3d9 as u64,
            DevicePointer::D3D11(d3d11) => *d3d11 as u64,
        }
    }
}