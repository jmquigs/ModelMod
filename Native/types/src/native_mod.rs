use crate::{interop::ModData};
pub use crate::d3ddata::ModD3DData;

pub enum ModD3DState {
    Unloaded,
    /// The mod data is partially available.  Used for DX11 before where we need a place to
    /// store the input layout prior to obtaining the rest of the data.
    Partial(ModD3DData),
    Loaded(ModD3DData)
}

impl ModD3DState {
    /// Change the state from partial to loaded.  If the current state is not partial, this is a no-op.
    pub fn set_loaded(&mut self) {
        use crate::native_mod::ModD3DState::Unloaded;
        use crate::native_mod::ModD3DState::Loaded;

        if let ModD3DState::Partial(_d3d_data) = self {
            let prev = std::mem::replace(self, Unloaded);
            if let ModD3DState::Partial(d3d_data) = prev {
                *self = Loaded(d3d_data);
            }
        }
    }

    pub fn is_loaded(&self) -> bool {
        use crate::native_mod::ModD3DState::Loaded;
        match self {
            Loaded(_) => true,
            _ => false,
        }
    }
}

pub struct NativeModData {
    pub midx: i32,
    pub mod_data: ModData,
    pub d3d_data: ModD3DState,
    pub is_parent: bool,
    pub parent_mod_names: Vec<String>,
    pub last_frame_render: u64,
    pub name: String,
}

pub const MAX_RECENT_RENDER_FRAMES:u64 = 500;

impl NativeModData {
    pub fn new() -> Self {
        Self {
            midx: -1,
            mod_data: ModData::new(),
            d3d_data: ModD3DState::Unloaded,
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
        curr_frame_num - self.last_frame_render <= MAX_RECENT_RENDER_FRAMES
    }
    /// Utility function to split a potentially or'ed list of parents into individual strings
    pub fn split_parent_string(pstr:&str) -> Vec<String> {
        pstr.trim().split(" or ").map(|p| p.trim()).filter(|p| !p.is_empty()).map(|p| p.to_owned()).collect()
    }
}

