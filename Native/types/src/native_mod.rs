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
    pub parent_mod_names: Vec<String>,
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
            parent_mod_names: vec![],
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
    /// Utility function to split a potentially or'ed list of parents into individual strings
    pub fn split_parent_string(pstr:&str) -> Vec<String> {
        pstr.trim().split(" or ").map(|p| p.trim()).filter(|p| !p.is_empty()).map(|p| p.to_owned()).collect()
    }
}

