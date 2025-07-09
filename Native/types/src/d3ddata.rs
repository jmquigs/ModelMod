
use shared_dx::util;
use winapi::shared::d3d9::*;
use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout, ID3D11Texture2D, ID3D11Resource, ID3D11ShaderResourceView};

pub struct ModD3DData9 {
    pub vb: *mut IDirect3DVertexBuffer9,
    pub decl: *mut IDirect3DVertexDeclaration9,
    pub textures: [LPDIRECT3DTEXTURE9; 4],
}

impl Clone for ModD3DData9 {
    fn clone(&self) -> Self {
        unsafe {
            if !self.vb.is_null() {
                (*self.vb).AddRef();
            }
            if !self.decl.is_null() {
                (*self.decl).AddRef();
            }
            for tex in self.textures.iter() {
                if !tex.is_null() {
                    let btex = *tex as *mut IDirect3DBaseTexture9;
                    (*btex).AddRef();
                }
            }
        }

        Self {
            vb: self.vb,
            decl: self.decl,
            // Clones the textures array with each element having AddRef called if non-null
            textures: self.textures,
        }
    }
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

impl Drop for ModD3DData9 {
    fn drop(&mut self) {
        unsafe { self.release(); }
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

impl Clone for ModD3DData11 {
    fn clone(&self) -> Self {
        unsafe {
            if !self.vb.is_null() {
                (*self.vb).AddRef();
            }
            if !self.vlayout.is_null() {
                (*self.vlayout).AddRef();
            }
            for tex in self.textures.iter() {
                if !tex.is_null() {
                    let btex = *tex as *mut ID3D11Resource;
                    (*btex).AddRef();
                }
            }
            for srv in self.srvs.iter() {
                if !srv.is_null() {
                    let bsrv = *srv as *mut ID3D11Resource;
                    (*bsrv).AddRef();
                }
            }
        }

        Self {
            vb: self.vb,
            vlayout: self.vlayout,
            textures: self.textures,
            has_textures: self.has_textures,
            srvs: self.srvs,
            vert_size: self.vert_size,
            vert_count: self.vert_count,
        }
    }
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
                let _rc = (*self.vb).Release();
                //if rc == 0 { util::write_log_file("releasing vb on d3d11 data");}
                self.vb = std::ptr::null_mut();
            }
            if !self.vlayout.is_null() {
                let _rc = (*self.vlayout).Release();
                //if rc == 0 { util::write_log_file("releasing vlayout on d3d11 data");}
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

impl Drop for ModD3DData11 {
    fn drop(&mut self) {
        self.release();
    }
}

/// Container for D3D resources of a mod.
#[derive(Clone)]
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
