use std::ffi::c_void;
/*
Contains combinations of types from both DX9 and 11, most notably `HookDeviceState` which
carries the device specific state for one or the other (but not both) at runtime.
 */
use std::ptr::null_mut;
use std::time::SystemTime;
use fnv::FnvHashMap;
use winapi::shared::d3d9::LPDIRECT3DTEXTURE9;
use winapi::shared::windef::HWND;
use winapi::shared::d3d9::IDirect3DDevice9;
use winapi::um::d3d11::ID3D11Device;
use winapi::um::d3d11::ID3D11Resource;
use winapi::um::d3d11::ID3D11ShaderResourceView;
use winapi::um::unknwnbase::IUnknown;
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
                secs * 1000 + milisecs as u64
            },
            _ => {
                0
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
    pub app_hwnds: Vec<HWND>,
    pub last_timebased_update: SystemTime,
    pub last_data_expire: SystemTime,
    pub last_data_expire_type_flip: bool,
    pub app_foreground: bool,
}

impl HookD3D11State {
    pub fn from(hooks:HookDirect3D11, devptr:*mut ID3D11Device ) -> Self {
        HookD3D11State {
            hooks,
            devptr: DevicePointer::D3D11(devptr),
            metrics: DX11Metrics::new(),
            rs: DX11RenderState::new(),
            app_hwnds: Vec::new(),
            last_timebased_update: SystemTime::now(),
            last_data_expire: SystemTime::now(),
            last_data_expire_type_flip: false,
            app_foreground: false,
        }
    }
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
    /// Run a function with the device pointer if the pointer is non null and a d3d11 device.
    /// If either of these is false, does nothing.
    pub fn with_d3d11<F,R>(&self, f:F) -> Option<R>
        where F: FnOnce(*mut ID3D11Device) -> R
    {
        match self {
            DevicePointer::D3D11(d3d11) if !d3d11.is_null() => Some(f(*d3d11)),
            _ => None,
        }
    }
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
    /// Returns the pointer value as a usize.
    pub fn as_usize(&self) -> usize {
        match self {
            DevicePointer::D3D9(d3d9) => *d3d9 as usize,
            DevicePointer::D3D11(d3d11) => *d3d11 as usize,
        }
    }
    /// Returns the pointer value as a *mut c_void.
    pub fn as_c_void(&mut self) -> *mut c_void {
        match self {
            DevicePointer::D3D9(d3d9) => *d3d9 as *mut c_void,
            DevicePointer::D3D11(d3d11) => *d3d11 as *mut c_void,
        }
    }
    /// If `new_ptr` is not null and differs from the current pointer in self,
    /// change the self pointer to `new_ptr` and return true.  Otherwise return false.
    pub fn maybe_update<T>(&mut self, new_ptr: *mut T) -> bool {
        if new_ptr.is_null() {
            return false;
        }
        match self {
            DevicePointer::D3D9(d3d9) => {
                if *d3d9 != new_ptr as *mut IDirect3DDevice9 {
                    *d3d9 = new_ptr as *mut IDirect3DDevice9;
                    true
                } else {
                    false
                }
            },
            DevicePointer::D3D11(d3d11) => {
                if *d3d11 != new_ptr as *mut ID3D11Device {
                    *d3d11 = new_ptr as *mut ID3D11Device;
                    true
                } else {
                    false
                }
            },
        }
    }
}

#[derive(Debug)]
pub enum D3D11Tex {
    Tex(*mut ID3D11Resource),
    TexSrv(*mut ID3D11Resource, *mut ID3D11ShaderResourceView),
}

#[derive(Debug)]
pub enum TexPtr {
    D3D9(LPDIRECT3DTEXTURE9),
    D3D11(D3D11Tex),
}

impl TexPtr {
    pub fn is_null(&self) -> bool {
        match self {
            TexPtr::D3D9(tex) => tex.is_null(),
            TexPtr::D3D11(D3D11Tex::Tex(tex)) => tex.is_null(),
            TexPtr::D3D11(D3D11Tex::TexSrv(tex, srv)) => tex.is_null() || srv.is_null(),
        }
    }

    /// Returns the pointer value as a usize.  0 if null.  If it is the TexSrv enum for DX11, returns the srv pointer.
    /// For other cases returns the texture pointer.
    pub fn as_usize(&self) -> usize {
        if self.is_null() {
            return 0;
        }
        match self {
            TexPtr::D3D9(tex) => *tex as usize,
            TexPtr::D3D11(D3D11Tex::Tex(tex)) => *tex as usize,
            // for this case need to pick one so use the srv
            TexPtr::D3D11(D3D11Tex::TexSrv(_tex, srv)) => *srv as usize,
        }
    }

    pub unsafe fn release(self) {
        match self {
            TexPtr::D3D9(tex) => {
                if !tex.is_null() {
                    (*(tex as *mut IUnknown)).Release();
                }
            },
            TexPtr::D3D11(D3D11Tex::Tex(tex)) => {
                if !tex.is_null() {
                    (*(tex as *mut IUnknown)).Release();
                }
            },
            TexPtr::D3D11(D3D11Tex::TexSrv(tex, srv)) => {
                if !tex.is_null() {
                    (*(tex as *mut IUnknown)).Release();
                }
                if !srv.is_null() {
                    (*(srv as *mut IUnknown)).Release();
                }
            },
        }
    }
}
