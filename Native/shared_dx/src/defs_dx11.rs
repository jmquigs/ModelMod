use winapi::shared::minwindef::{UINT, INT, ULONG};
//use winapi::shared::windef::{HWND, RECT};
//use winapi::shared::winerror::{E_FAIL, S_OK};

use winapi::um::d3d11::ID3D11Buffer;
use winapi::um::d3d11::ID3D11DeviceContext;
use winapi::um::unknwnbase::IUnknown;

pub type IUnknownReleaseFn = unsafe extern "system" fn(THIS: *mut IUnknown) -> ULONG;

 pub type DrawIndexedFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    IndexCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
) -> ();
pub type DrawFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    VertexCount: UINT,
    StartVertexLocation: UINT,
) -> ();
pub type DrawIndexedInstancedFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    IndexCountPerInstance: UINT,
    InstanceCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
    StartInstanceLocation: UINT,
) -> ();
pub type DrawInstancedFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    VertexCountPerInstance: UINT,
    InstanceCount: UINT,
    StartVertexLocation: UINT,
    StartInstanceLocation: UINT,
) -> ();
pub type DrawAutoFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
) -> ();
pub type DrawIndexedInstancedIndirectFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    pBufferForArgs: *mut ID3D11Buffer,
    AlignedByteOffsetForArgs: UINT,
) -> ();
pub type DrawInstancedIndirectFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    pBufferForArgs: *mut ID3D11Buffer,
    AlignedByteOffsetForArgs: UINT,
) -> ();
