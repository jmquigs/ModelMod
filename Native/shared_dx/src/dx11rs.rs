use std::{fmt::{Display, Formatter, Error}, ffi::CStr, time::SystemTime};

use fnv::FnvHashMap;
use winapi::um::{d3d11::{ID3D11InputLayout, D3D11_INPUT_ELEMENT_DESC, D3D11_PRIMITIVE_TOPOLOGY}, d3dcommon::D3D_PRIMITIVE_TOPOLOGY_UNDEFINED};


/// Container for a vertex format.  Contains a list of elements used by the format and its size in bytes.
/// The vertex elements contain raw pointers which are const char* from the C-world.
/// Prior to creating a `VertexFormat`, these strings are copied and then the pointers updated to point
/// at `device_semantic_string_table`
/// in `DX11RenderState`.  Because of this, this struct does not implement Copy or Clone as I don't
/// want random copies of it getting strewn about.
///
/// `shallow_copy` can be used on the format to
/// make a copy, but since this aliases the pointer it should be used very sparingly.
pub struct VertexFormat {
    pub layout: Vec<D3D11_INPUT_ELEMENT_DESC>,
    pub size: u32,
}

impl VertexFormat {
    /// Create a shallow copy of the vertex format.  This will copy the layout vector, but the
    /// pointers in the vector elements will still point to the same strings as the original.
    pub fn shallow_copy(&self) -> Self {
        VertexFormat {
            layout: self.layout.clone(),
            size: self.size,
        }
    }
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
    /// Number of layouts in `device_input_layouts_by_ptr`
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
    /// Contains the semantic string pointers used by the `VertexFormats` in
    /// `device_input_layouts_by_ptr` and `context_input_layouts_by_ptr`.
    /// Clearing this will invalidate and leave dangling all the pointers
    /// those structures.  As well as any clones that exist elsewhere.
    /// So probably you shouldn't clear it, unless you can clear those as well or this entire
    /// structure and you know there aren't any clones.
    pub device_semantic_string_table: FnvHashMap<String, Vec<u8>>,
    /// When snapshotting this stores all index buffer data, because we can't read it on the fly.
    pub device_index_buffer_data: FnvHashMap<usize, Vec<u8>>,
    /// Controls when index data is removed
    pub device_index_buffer_createtime: Vec<(usize,SystemTime)>,
    pub device_index_buffer_totalsize_nextlog: (usize,usize),
    /// When snapshotting this stores all vertex buffer data, because we can't read it on the fly.
    pub device_vertex_buffer_data: FnvHashMap<usize, Vec<u8>>,
    /// Controls when vertex data is removed
    pub device_vertex_buffer_createtime: Vec<(usize,SystemTime)>,
    pub device_vertex_buffer_totalsize_nextlog: (usize,usize),
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
            device_semantic_string_table: FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            device_index_buffer_data: FnvHashMap::with_capacity_and_hasher(1600, Default::default()),
            device_index_buffer_createtime: Vec::new(),
            device_index_buffer_totalsize_nextlog: (0,0),
            device_vertex_buffer_data: FnvHashMap::with_capacity_and_hasher(1600, Default::default()),
            device_vertex_buffer_createtime: Vec::new(),
            device_vertex_buffer_totalsize_nextlog: (0,0),
        }
    }

    pub fn get_current_vertex_format(&self) -> Option<&VertexFormat>  {
        if self.current_input_layout.is_null() {
            return None;
        }
        let ptr = self.current_input_layout as usize;
        self.context_input_layouts_by_ptr.get(&ptr)
    }
}