pub use winapi::shared::minwindef::{UINT, INT};
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};

use winapi::um::d3d11::ID3D11Buffer;

 pub type DrawIndexedFn = unsafe extern "system" fn (
    IndexCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
) -> ();
pub type DrawFn = unsafe extern "system" fn (
    VertexCount: UINT,
    StartVertexLocation: UINT,
) -> ();
pub type DrawIndexedInstancedFn = unsafe extern "system" fn (
    IndexCountPerInstance: UINT,
    InstanceCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
    StartInstanceLocation: UINT,
) -> ();
pub type DrawInstancedFn = unsafe extern "system" fn (
    VertexCountPerInstance: UINT,
    InstanceCount: UINT,
    StartVertexLocation: UINT,
    StartInstanceLocation: UINT,
) -> ();
pub type DrawAutoFn = unsafe extern "system" fn () -> ();
pub type DrawIndexedInstancedIndirectFn = unsafe extern "system" fn (
    pBufferForArgs: *mut ID3D11Buffer,
    AlignedByteOffsetForArgs: UINT,
) -> ();
pub type DrawInstancedIndirectFn = unsafe extern "system" fn (
    pBufferForArgs: *mut ID3D11Buffer,
    AlignedByteOffsetForArgs: UINT,
) -> ();
