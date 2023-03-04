use fnv::FnvHashMap;
use winapi::um::d3d11::{ID3D11InputLayout, D3D11_INPUT_ELEMENT_DESC};


pub struct VertexFormat {
    pub layout: Vec<D3D11_INPUT_ELEMENT_DESC>,
    pub size: u32,
}

pub struct DX11RenderState {
    /// Current vertex buffer properties, vector of (buf index,byte width,stride).
    pub vb_state: Vec<(u32,u32,u32)>,
    pub input_layouts_by_ptr: Option<FnvHashMap<u64, VertexFormat>>,
    pub current_input_layout: *mut ID3D11InputLayout,
}