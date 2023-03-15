use std::{fmt::{Display, Formatter, Error}, ffi::CStr};

use fnv::FnvHashMap;
use winapi::um::{d3d11::{ID3D11InputLayout, D3D11_INPUT_ELEMENT_DESC, D3D11_PRIMITIVE_TOPOLOGY}, d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED};


pub struct VertexFormat {
    pub layout: Vec<D3D11_INPUT_ELEMENT_DESC>,
    pub size: u32,
}

impl Display for VertexFormat {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "VertexFormat {{ layout: [")?;
        for i in 0..self.layout.len() {
            let bytename = unsafe { CStr::from_ptr(self.layout[i].SemanticName) }.to_str();

            write!(f, "{:?}",  bytename)?;
            if i < self.layout.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "], size: {} }}", self.size)
    }
}

pub struct DX11RenderState {
    /// Current vertex buffer properties, vector of (buf index,byte width,stride).
    pub vb_state: Vec<(u32,u32,u32)>,
    pub input_layouts_by_ptr: FnvHashMap<usize, VertexFormat>,
    pub current_input_layout: *mut ID3D11InputLayout,
    pub prim_topology: D3D11_PRIMITIVE_TOPOLOGY,
}

impl DX11RenderState {
    pub fn new() -> Self {
        Self {
            vb_state: Vec::new(),
            input_layouts_by_ptr: FnvHashMap::with_capacity_and_hasher(1600, Default::default()),
            current_input_layout: std::ptr::null_mut(),
            prim_topology: D3D_PRIMITIVE_TOPOLOGY_UNDEFINED,
        }
    }
}