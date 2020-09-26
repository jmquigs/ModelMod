use std::os::raw::c_char;
pub use winapi::shared::d3d9::*;
//pub use winapi::shared::d3d9types::*;
use crate::interop::ModData;

pub struct NativeModData {
    pub mod_data: ModData,
    pub vb_data: *mut c_char,
    pub ib_data: *mut c_char,
    pub decl_data: *mut c_char,
    pub vb: *mut IDirect3DVertexBuffer9,
    pub ib: *mut IDirect3DIndexBuffer9,
    pub decl: *mut IDirect3DVertexDeclaration9,
    pub textures: [LPDIRECT3DTEXTURE9; 4],
    pub is_parent: bool,
    pub parent_mod_name: String,
    pub last_frame_render: u64, // only set for parent mods
    pub name: String,
    //IDirect3DPixelShader9* pixelShader;
}

impl NativeModData {
    pub fn new() -> Self {
        use std::ptr::null_mut;
        
        Self {
            mod_data: ModData::new(),
            vb_data: null_mut(),
            ib_data: null_mut(),
            decl_data: null_mut(),
            vb: null_mut(),
            ib: null_mut(),
            decl: null_mut(),
            textures: [null_mut(); 4],
            is_parent: false,
            parent_mod_name: "".to_owned(),
            last_frame_render: 0,
            name: "".to_owned(),
        }
    }
    pub fn mod_key(vert_count: u32, prim_count: u32) -> u32 {
        //https://en.wikipedia.org/wiki/Pairing_function#Cantor_pairing_function
        ((vert_count + prim_count) * (vert_count + prim_count + 1) / 2) + prim_count
    }
    pub fn recently_rendered(&self, curr_frame_num:u64) -> bool {
        if self.last_frame_render > curr_frame_num {
            // we rendered in the future, so I guess that is recent?
            return true;
        }
        curr_frame_num - self.last_frame_render <= 10 // last 150ms or so ought to be fine
    }
}

