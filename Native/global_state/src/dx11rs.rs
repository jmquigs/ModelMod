use fnv::FnvHashMap;
use winapi::um::d3d11::D3D11_INPUT_CLASSIFICATION;
use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::shared::minwindef::UINT;

#[derive(Debug)]
pub struct InputLayoutElem {
    pub name: String,
    pub index: UINT,
    pub format: DXGI_FORMAT,
    pub offset: UINT,
    pub slot: UINT,
    pub slot_class: D3D11_INPUT_CLASSIFICATION,
}

#[derive(Debug)]
pub struct VertexFormat {
    pub layout: Vec<InputLayoutElem>,
    pub size: u32,
}

pub struct DX11RenderState {
    /// Current vertex buffer properties, vector of (buf index,byte width,stride).
    pub vb_state: Vec<(u32,u32,u32)>,
    pub input_layouts_by_ptr: Option<FnvHashMap<u64, VertexFormat>>,
    pub current_input_layout: u64,
}