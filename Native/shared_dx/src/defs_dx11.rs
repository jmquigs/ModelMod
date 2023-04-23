use winapi::ctypes::c_void;
use winapi::shared::basetsd::SIZE_T;
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{UINT, INT, ULONG};

use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout, D3D11_INPUT_ELEMENT_DESC,
    ID3D11Device, D3D11_PRIMITIVE_TOPOLOGY, ID3D11ShaderResourceView, D3D11_BUFFER_DESC,
    D3D11_SUBRESOURCE_DATA, ID3D11Resource, D3D11_TEXTURE2D_DESC, ID3D11Texture2D};
use winapi::um::d3d11::ID3D11DeviceContext;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winnt::HRESULT;

use crate::impl_release_drop;

pub type IUnknownReleaseFn = unsafe extern "system" fn(THIS: *mut IUnknown) -> ULONG;
pub type QueryInterfaceFn = unsafe extern "system" fn (
    THIS: *mut IUnknown,
    riid: REFIID,
    ppvObject: *mut *mut c_void,
) -> HRESULT;

pub type CreateInputLayoutFn = unsafe extern "system" fn(
    THIS: *mut ID3D11Device,
    pInputElementDescs: *const D3D11_INPUT_ELEMENT_DESC,
    NumElements: UINT,
    pShaderBytecodeWithInputSignature: *const c_void,
    BytecodeLength: SIZE_T,
    ppInputLayout: *mut *mut ID3D11InputLayout,
) -> HRESULT;

pub type CreateBufferFn = unsafe extern "system" fn(
    THIS: *mut ID3D11Device,
    pDesc: *const D3D11_BUFFER_DESC,
    pInitialData: *const D3D11_SUBRESOURCE_DATA,
    ppBuffer: *mut *mut ID3D11Buffer,
) -> HRESULT;

pub type IASetVertexBuffersFn = unsafe extern "system" fn(
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumBuffers: UINT,
    ppVertexBuffers: *const *mut ID3D11Buffer,
    pStrides: *const UINT,
    pOffsets: *const UINT,
) -> ();

pub type VSSetConstantBuffersFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumBuffers: UINT,
    ppConstantBuffers: *const *mut ID3D11Buffer,
) -> ();
pub type IASetInputLayoutFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    pInputLayout: *mut ID3D11InputLayout,
) -> ();
pub type IASetPrimitiveTopologyFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    Topology: D3D11_PRIMITIVE_TOPOLOGY,
) -> ();
pub type PSSetShaderResourcesFn = unsafe extern "system" fn (
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumViews: UINT,
    ppShaderResourceViews: *const *mut ID3D11ShaderResourceView,
) -> ();
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

impl_release_drop!(ID3D11ShaderResourceView);
impl_release_drop!(ID3D11Buffer);
impl_release_drop!(ID3D11Resource);
impl_release_drop!(ID3D11DeviceContext);
impl_release_drop!(ID3D11Texture2D);