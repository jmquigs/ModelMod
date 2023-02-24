use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::wingdi::RGNDATA;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use crate::impl_release_drop;

pub type D3DXSaveTextureToFileWFn = unsafe extern "system" fn(
    path: LPCWSTR,
    fileformat: i32,
    src_texture: *mut IDirect3DBaseTexture9,
    src_palette: *mut c_void,
) -> HRESULT;

RIDL!(#[uuid(0x8ba5fb08, 0x5195, 0x40e2, 0xac, 0x58, 0xd, 0x98, 0x9c, 0x3a, 0x1, 0x2)]
interface ID3DXBuffer(ID3DXBufferVtbl): IUnknown(IUnknownVtbl) {
    fn GetBufferPointer() -> LPVOID,
    fn GetBufferSize() -> DWORD,
});

impl_release_drop!(ID3DXBuffer);

pub type D3DXDisassembleShaderFn = unsafe extern "system" fn(
    pShader: *const DWORD,
    EnableColorCode: BOOL,
    pComments: *mut c_void,
    ppDisassembly: *mut *mut ID3DXBuffer,
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
    Vector4fCount: UINT,
) -> HRESULT;

pub type SetVertexShaderConstantBFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT,
) -> HRESULT;

pub type SetVertexShaderConstantIFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT;

pub type SetPixelShaderConstantFFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT,
) -> HRESULT;

pub type SetPixelShaderConstantBFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT,
) -> HRESULT;

pub type SetPixelShaderConstantIFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT;

impl_release_drop!(IDirect3DBaseTexture9);
impl_release_drop!(IDirect3DVertexDeclaration9);
impl_release_drop!(IDirect3DIndexBuffer9);
impl_release_drop!(IDirect3DPixelShader9);
impl_release_drop!(IDirect3DVertexShader9);