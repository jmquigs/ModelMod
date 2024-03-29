
use winapi::shared::d3d9::*;
use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout, ID3D11Texture2D, ID3D11Resource, ID3D11ShaderResourceView};

pub struct ModD3DData9 {
    pub vb: *mut IDirect3DVertexBuffer9,
    pub decl: *mut IDirect3DVertexDeclaration9,
    pub textures: [LPDIRECT3DTEXTURE9; 4],
}

impl ModD3DData9 {
    pub fn new() -> Self {
        use std::ptr::null_mut;

        Self {
            vb: null_mut(),
            decl: null_mut(),
            textures: [null_mut(); 4],
        }
    }

    pub unsafe fn release(&mut self) {
        if !self.vb.is_null() {
            (*self.vb).Release();
            self.vb = std::ptr::null_mut();
        }
        if !self.decl.is_null() {
            (*self.decl).Release();
            self.decl = std::ptr::null_mut();
        }
        for tex in self.textures.iter_mut() {
            if !tex.is_null() {
                let btex = *tex as *mut IDirect3DBaseTexture9;
                (*btex).Release();
                *tex = std::ptr::null_mut();
            }
        }
    }
}

pub struct ModD3DData11 {
    pub vb: *mut ID3D11Buffer,
    pub vlayout: *mut ID3D11InputLayout,
    pub textures: [*mut ID3D11Texture2D; 4],
    pub has_textures: bool,
    pub srvs: [*mut ID3D11ShaderResourceView; 4],
    pub vert_size:u32,
    pub vert_count:u32,
}

impl ModD3DData11 {
    pub fn new() -> Self {
        use std::ptr::null_mut;

        Self {
            vb: null_mut(),
            vlayout: null_mut(),
            textures: [null_mut(); 4],
            has_textures: false,
            srvs: [null_mut(); 4],
            vert_size: 0,
            vert_count: 0,
        }
    }
    /// Create a new ModD3DData11 with the given layout.  AddRef is not called on the layout.
    pub fn with_layout(layout: *mut ID3D11InputLayout) -> Self {
        use std::ptr::null_mut;

        Self {
            vb: null_mut(),
            vlayout: layout,
            textures: [null_mut(); 4],
            has_textures: false,
            srvs: [null_mut(); 4],
            vert_size: 0,
            vert_count: 0,
        }
    }

    pub fn release(&mut self) {
        unsafe {
            if !self.vb.is_null() {
                (*self.vb).Release();
                self.vb = std::ptr::null_mut();
            }
            if !self.vlayout.is_null() {
                (*self.vlayout).Release();
                self.vlayout = std::ptr::null_mut();
            }
            for srv in self.srvs.iter_mut() {
                if !srv.is_null() {
                    let bsrv = *srv as *mut ID3D11Resource;
                    (*bsrv).Release();
                    *srv = std::ptr::null_mut();
                }
            }
            for tex in self.textures.iter_mut() {
                if !tex.is_null() {
                    let btex = *tex as *mut ID3D11Resource;
                    (*btex).Release();
                    *tex = std::ptr::null_mut();
                }
            }
        }
    }
}

/// Container for D3D resources of a mod.
pub enum ModD3DData {
    D3D9(ModD3DData9),
    D3D11(ModD3DData11),
}

impl ModD3DData {
    /// Release the resource owned by this mod.  Safe to call if they are null.
    /// Sets own fields to null after release, so they can't be released more than once
    /// by this function.
    pub unsafe fn release(&mut self) {
        match self {
            ModD3DData::D3D9(d) => d.release(),
            ModD3DData::D3D11(d) => d.release(),
        }
    }
}
