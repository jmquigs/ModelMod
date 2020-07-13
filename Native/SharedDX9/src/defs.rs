use winapi::um::unknwnbase::IUnknown;
use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::wingdi::RGNDATA;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

pub type D3DXSaveTextureToFileWFn = unsafe extern "system" fn(
    path: LPCWSTR,
    fileformat: i32,
    src_texture: *mut IDirect3DBaseTexture9,
    src_palette: *mut c_void,
) -> HRESULT;

pub type CreateDeviceFn = unsafe extern "system" fn(
    THIS: *mut IDirect3D9,
    Adapter: UINT,
    DeviceType: D3DDEVTYPE,
    hFocusWindow: HWND,
    BehaviorFlags: DWORD,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
    ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
) -> HRESULT;
pub type DrawIndexedPrimitiveFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    arg1: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT;
pub type BeginSceneFn = unsafe extern "system" fn(THIS: *mut IDirect3DDevice9) -> HRESULT;
pub type IUnknownReleaseFn = unsafe extern "system" fn(THIS: *mut IUnknown) -> ULONG;
pub type PresentFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT;
pub type SetTextureFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,
) -> HRESULT;

// shader constants
pub type SetVertexShaderConstantFFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT;