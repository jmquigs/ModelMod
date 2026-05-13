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

/// Packed bitmask of (semantic, semantic_index) pairs declared by a vertex
/// layout, restricted to the semantics ModelMod knows how to fill.
///
/// Two distinct layouts that declare the same set of supported semantics
/// produce equal masks. The hot-path refill check is `(new & !old) != 0`.
pub type SemanticMask = u128;

/// Subset of D3D semantic names ModelMod fills. Anything not listed here
/// (custom engine semantics, etc.) is silently dropped from the mask, which
/// matches the existing fill behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Semantic {
    Position     = 0,
    Normal       = 1,
    TexCoord     = 2,
    Binormal     = 3,
    Bitangent    = 4,
    Color        = 5,
    Tangent      = 6,
    BlendIndices = 7,
    BlendWeight  = 8,
}

/// Number of distinct semantic indices per semantic that fit in the mask.
/// 9 semantics * 14 indices = 126 bits, fits in u128 with 2 spare. Indices
/// at or above this value get clamped to the top slot (extremely rare in
/// practice; ModelMod would not be filling them anyway).
const SEM_INDEX_SLOTS: u32 = 14;

impl Semantic {
    /// D3D treats semantic names as case-insensitive; match that here.
    fn from_name_bytes(name: &[u8]) -> Option<Self> {
        if      name.eq_ignore_ascii_case(b"POSITION")     { Some(Semantic::Position) }
        else if name.eq_ignore_ascii_case(b"NORMAL")       { Some(Semantic::Normal) }
        else if name.eq_ignore_ascii_case(b"TEXCOORD")     { Some(Semantic::TexCoord) }
        else if name.eq_ignore_ascii_case(b"BINORMAL")     { Some(Semantic::Binormal) }
        else if name.eq_ignore_ascii_case(b"BITANGENT")    { Some(Semantic::Bitangent) }
        else if name.eq_ignore_ascii_case(b"COLOR")        { Some(Semantic::Color) }
        else if name.eq_ignore_ascii_case(b"TANGENT")      { Some(Semantic::Tangent) }
        else if name.eq_ignore_ascii_case(b"BLENDINDICES") { Some(Semantic::BlendIndices) }
        else if name.eq_ignore_ascii_case(b"BLENDWEIGHT")  { Some(Semantic::BlendWeight) }
        else { None }
    }

    #[inline]
    fn mask_bit(self, index: u32) -> SemanticMask {
        let idx = index.min(SEM_INDEX_SLOTS - 1);
        1u128 << ((self as u32) * SEM_INDEX_SLOTS + idx)
    }
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

    /// Compute a bitmask of the (semantic, semantic_index) pairs declared by
    /// this layout, restricted to semantics ModelMod fills. Cheap enough to
    /// recompute on each modded draw (CStr scan over typically <=16 elements).
    pub fn semantic_mask(&self) -> SemanticMask {
        let mut mask: SemanticMask = 0;
        for elem in &self.layout {
            if elem.SemanticName.is_null() {
                continue;
            }
            let name_bytes = unsafe { CStr::from_ptr(elem.SemanticName) }.to_bytes();
            if let Some(sem) = Semantic::from_name_bytes(name_bytes) {
                mask |= sem.mask_bit(elem.SemanticIndex);
            }
        }
        mask
    }

    /// True if `new` declares any (semantic, index) pair not present in `old`.
    #[inline]
    pub fn has_extra_semantics(old: SemanticMask, new: SemanticMask) -> bool {
        (new & !old) != 0
    }
}

impl Display for VertexFormat {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "VertexFormat {{ layout: [")?;
        for i in 0..self.layout.len() {
            let bytename = unsafe { CStr::from_ptr(self.layout[i].SemanticName) }.to_str();

            write!(f, "{:?}/{}",  bytename, self.layout[i].SemanticIndex)?;
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