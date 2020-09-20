#![allow(non_snake_case)]
use winapi::shared::d3d9::*;
//use winapi::shared::d3d9types::*;
use shared_dx9::defs::*;

pub type D3DXCreateTextureFromFileWFn = unsafe extern "system" fn(
    pDevice: LPDIRECT3DDEVICE9,
    pSrcFile: LPCWSTR,
    ppTexture: *mut LPDIRECT3DTEXTURE9,
) -> HRESULT;

pub struct D3DXFn {
    pub D3DXSaveTextureToFileW: D3DXSaveTextureToFileWFn,
    pub D3DXCreateTextureFromFileW: D3DXCreateTextureFromFileWFn,
    pub D3DXDisassembleShader: D3DXDisassembleShaderFn,
}