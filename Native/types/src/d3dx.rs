#![allow(non_snake_case)]
use winapi::{shared::{d3d9::*, dxgiformat::DXGI_FORMAT}, um::d3d11::{ID3D11Device, ID3D11Resource, ID3D11DeviceContext}, ctypes::c_void};
//use winapi::shared::d3d9types::*;
use shared_dx::defs_dx9::*;

pub type D3DXCreateTextureFromFileWFn = unsafe extern "system" fn(
    pDevice: LPDIRECT3DDEVICE9,
    pSrcFile: LPCWSTR,
    ppTexture: *mut LPDIRECT3DTEXTURE9,
) -> HRESULT;

pub struct D3DX9Fn {
    pub D3DXSaveTextureToFileW: D3DXSaveTextureToFileWFn,
    pub D3DXCreateTextureFromFileW: D3DXCreateTextureFromFileWFn,
    pub D3DXDisassembleShader: D3DXDisassembleShaderFn,
}

pub type D3DX11CreateTextureFromFileWFn = unsafe extern "system" fn(
    pDevice: *mut ID3D11Device,
    pSrcFile: LPCWSTR,
    pLoadInfo: *const c_void,
    pPump: *const c_void,
    ppTexture: *mut *mut ID3D11Resource,
    pHResult: *mut HRESULT,
) -> HRESULT;

pub type D3DX11SaveTextureToFileWFn = unsafe extern "system" fn(
    pContext: *mut ID3D11DeviceContext,
    pSrcResource: *mut ID3D11Resource,
    DestFormat: DXGI_FORMAT,
    pDestFile: LPCWSTR,
) -> HRESULT;

pub struct D3DX11Fn {
    pub D3DX11SaveTextureToFileW: D3DX11SaveTextureToFileWFn,
    pub D3DX11CreateTextureFromFileW: D3DX11CreateTextureFromFileWFn,
    //pub D3DX11DisassembleShader: D3DX11DisassembleShaderFn,
}

pub enum D3DXFn {
    DX9(D3DX9Fn),
    DX11(D3DX11Fn),
}