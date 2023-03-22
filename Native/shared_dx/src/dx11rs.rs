use std::{fmt::{Display, Formatter, Error}, ffi::CStr};

use fnv::FnvHashMap;
use winapi::um::{d3d11::{ID3D11InputLayout, D3D11_INPUT_ELEMENT_DESC, D3D11_PRIMITIVE_TOPOLOGY}, d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED};


#[derive(Clone)]
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
    /// Count of layouts stored in `device_input_layouts_by_ptr`.
    pub num_input_layouts: std::sync::atomic::AtomicUsize,
    /// Input layouts that were created on the device.  This should not be accessed without a
    /// lock (obtained with `dev_state_d3d11_write()`) because the device can have multiple threads.
    /// The context will periodically copy this to `context_input_layouts_by_ptr`, which it uses
    /// exclusively for its operations.  It does this so that it doesn't need to lock just to read these
    /// layouts, which would kill performance.
    pub device_input_layouts_by_ptr: FnvHashMap<usize, VertexFormat>,
    /// List of layouts available to the context, copied from `device_input_layouts_by_ptr`
    /// periodically.
    pub context_input_layouts_by_ptr: FnvHashMap<usize, VertexFormat>,
    /// The last input layout that was set on the context via IASetInputLayout.
    pub current_input_layout: *mut ID3D11InputLayout,
    /// The last primitive topology that was set on the context via IASetPrimitiveTopology.
    pub prim_topology: D3D11_PRIMITIVE_TOPOLOGY,
}

impl DX11RenderState {
    pub fn new() -> Self {
        Self {
            vb_state: Vec::new(),
            num_input_layouts: std::sync::atomic::AtomicUsize::new(0),
            device_input_layouts_by_ptr: FnvHashMap::with_capacity_and_hasher(1600, Default::default()),
            context_input_layouts_by_ptr: FnvHashMap::with_capacity_and_hasher(1600, Default::default()),
            current_input_layout: std::ptr::null_mut(),
            prim_topology: D3D_PRIMITIVE_TOPOLOGY_UNDEFINED,
        }
    }
}