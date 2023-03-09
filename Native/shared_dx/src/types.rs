/*
Contains combinations of types from both DX9 and 11, most notably `HookDeviceState` which
carries the device specific state for one or the other (but not both) at runtime.
 */
use std::ptr::null_mut;
use std::time::SystemTime;
use fnv::FnvHashMap;
use winapi::shared::windef::HWND;
use winapi::shared::d3d9::IDirect3DDevice9;
use winapi::um::d3d11::ID3D11Device;
use crate::types_dx9::HookDirect3D9Device;
use crate::types_dx9::HookDirect3D9;
use crate::types_dx11::HookDirect3D11;
use crate::dx11rs::DX11RenderState;

pub struct HookD3D9State {
    pub d3d9: Option<HookDirect3D9>,
    pub device: Option<HookDirect3D9Device>,
}

#[derive(Debug)]
pub enum MetricsDrawStatus {
    /// Pair of (mod type, count of times referenced)
    Referenced(i32,u32),
    /// Pair of (name of mod, count of times referenced)
    LoadReq(String,u32)
}

impl MetricsDrawStatus {
    pub fn incr_count(&mut self) {
        match self {
            MetricsDrawStatus::Referenced(_,c) => *c += 1,
            MetricsDrawStatus::LoadReq(_,c) => *c += 1,
        }
    }
}
pub struct DX11Metrics {
    pub last_reset: SystemTime,
    /// Number of times `hook_VSSetConstantBuffers` was called
    pub vs_set_const_buffers_calls: u32,
    /// Number of times `hook_VSSetConstantBuffers` rehooked at least one function
    pub vs_set_const_buffers_hooks: u32,
    /// List of prim,vert combos that triggered a mod action from a recent draw call.
    pub drawn_recently: FnvHashMap<(u32,u32),MetricsDrawStatus>, // (prim,vert) => (mtype,count)
    pub rehook_time_nanos: u64,
    pub rehook_calls: u32,
}

impl DX11Metrics {
    pub fn new() -> Self {
        DX11Metrics {
            last_reset: SystemTime::now(),
            vs_set_const_buffers_calls: 0,
            vs_set_const_buffers_hooks: 0,
            drawn_recently: FnvHashMap::default(),
            rehook_time_nanos: 0,
            rehook_calls: 0,
        }
    }
    pub fn reset(&mut self) {
        self.last_reset = SystemTime::now();
        self.vs_set_const_buffers_calls = 0;
        self.vs_set_const_buffers_hooks = 0;
        self.drawn_recently.clear();
        self.rehook_time_nanos = 0;
        self.rehook_calls = 0;
    }
    /// Return number of milisecs since last reset
    pub fn ms_since_reset(&self) -> u64 {
        let now = SystemTime::now();
        match now.duration_since(self.last_reset) {
            Ok(d) => {
                let secs = d.as_secs();
                let milisecs = d.subsec_millis();
                return secs * 1000 + milisecs as u64;
            },
            _ => {
                return 0
            }
        }
    }
}

pub struct HookD3D11State {
    pub hooks: HookDirect3D11,
    /// In DX11 the device pointer is stored as part of this state because we generally
    /// do most of the work in device context functions, which don't get the device pointer.
    pub devptr: DevicePointer,
    pub metrics: DX11Metrics,
    /// Contains current render state for the device
    pub rs: DX11RenderState,
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