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

/// What a resource the game is writing turned out to be, as seen by the dynamic buffer hooks.
/// Cached by resource pointer in `DX11RenderState::device_buffer_meta` so that the very common
/// constant-buffer write costs a single hash lookup rather than a `GetType`/`GetDesc` pair.
///
/// Note the cache is keyed on a raw pointer and D3D reuses freed addresses, so a cached value is
/// a filter, not a source of truth: capture paths re-query the resource before copying anything
/// out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferMeta {
    /// A vertex or index buffer whose contents we want for snapshots, with its `ByteWidth`.
    Mesh { is_index: bool, byte_width: u32 },
    /// Anything else: a constant buffer, a texture, a zero-sized buffer, etc.
    Other,
}

/// How a tracked mesh buffer's stored bytes were last produced.  Diagnostic only: a snapshot that
/// comes out as garbage usually means our copy did not match what the draw actually read, and the
/// write path (and its D3D11_MAP type) is the first thing worth knowing.
///
/// Recorded only while the dynamic buffer feature is on, which is why `InitialData` appears here
/// even though it describes an ordinary static buffer: with the feature on, the creation-time copy
/// is tracked alongside the dynamic ones so the two can be told apart later.
#[cfg(feature = "snapshot-dynamic-buffers")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferWriteKind {
    /// Copied from `pInitialData` when the buffer was created.
    InitialData,
    /// Copied at `Unmap`; carries the `D3D11_MAP` type the game passed to `Map`.
    Map(u32),
    /// Whole-resource `UpdateSubresource`.
    UpdateSubresource,
    /// Boxed (partial) `UpdateSubresource` covering bytes `[left, right)`.
    UpdateSubresourceBox(u32, u32),
}

#[cfg(feature = "snapshot-dynamic-buffers")]
impl Display for BufferWriteKind {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            BufferWriteKind::InitialData => write!(f, "CreateBuffer initial data"),
            // D3D11_MAP values; spelled out here so the log doesn't need decoding.
            BufferWriteKind::Map(t) => {
                let name = match t {
                    1 => "READ",
                    2 => "WRITE",
                    3 => "READ_WRITE",
                    4 => "WRITE_DISCARD",
                    5 => "WRITE_NO_OVERWRITE",
                    _ => "?",
                };
                write!(f, "Map({})", name)
            },
            BufferWriteKind::UpdateSubresource => write!(f, "UpdateSubresource"),
            BufferWriteKind::UpdateSubresourceBox(l, r) =>
                write!(f, "UpdateSubresource[{}..{}]", l, r),
        }
    }
}

/// Provenance of one tracked buffer's stored bytes.  See `BUFFER_CAPTURE_SEQ` for what `seq` is
/// good for.
#[cfg(feature = "snapshot-dynamic-buffers")]
#[derive(Debug, Clone, Copy)]
pub struct BufferCaptureInfo {
    pub kind: BufferWriteKind,
    /// Value of `BUFFER_CAPTURE_SEQ` when this buffer was last captured.
    pub seq: u64,
    /// How many times this buffer has been captured.
    pub count: u64,
    /// Length in bytes of the stored copy after that capture.
    pub bytes: usize,
}

/// Monotonic counter incremented on every dynamic buffer capture.
///
/// Its value is meaningless on its own; the point is the differences.  Comparing the index and
/// vertex buffers' `seq` at snapshot time says whether the two copies came from the same batch of
/// writes or whether one of them is many captures stale, which is the difference between a
/// coherent mesh and a poly soup.
#[cfg(feature = "snapshot-dynamic-buffers")]
pub static BUFFER_CAPTURE_SEQ: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

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
    /// Holds copies taken at creation from `pInitialData` and, with the `snapshot-dynamic-buffers`
    /// feature, copies captured from dynamic buffers as the game writes them.  The two are mixed
    /// here on purpose; `buffer_capture_info` records which is which.
    pub device_index_buffer_data: FnvHashMap<usize, Vec<u8>>,
    /// Controls when index data is removed
    pub device_index_buffer_createtime: Vec<(usize,SystemTime)>,
    pub device_index_buffer_totalsize_nextlog: (usize,usize),
    /// When snapshotting this stores all vertex buffer data, because we can't read it on the fly.
    /// Mixes creation-time and dynamic copies exactly as `device_index_buffer_data` does.
    pub device_vertex_buffer_data: FnvHashMap<usize, Vec<u8>>,
    /// Controls when vertex data is removed
    pub device_vertex_buffer_createtime: Vec<(usize,SystemTime)>,
    pub device_vertex_buffer_totalsize_nextlog: (usize,usize),
    /// What each resource the dynamic buffer hooks have seen turned out to be, keyed by resource
    /// pointer.  This lets them identify whether a resource being written is a VB/IB we want to
    /// capture (and how big it is) without repeating the `GetType`/`GetDesc` calls on the hot
    /// path.  Needed because the game may create a buffer empty and fill it later, in which case
    /// `device_*_buffer_data` won't have an entry yet.
    ///
    /// Entries are written both by `hook_CreateBuffer` and lazily by the dynamic buffer hooks the
    /// first time they see an unknown resource.  The lazy path is what makes a runtime precopy
    /// enable work on buffers that already existed before the toggle.
    ///
    /// Only populated with the `snapshot-dynamic-buffers` feature; empty otherwise.
    pub device_buffer_meta: FnvHashMap<usize, BufferMeta>,
    /// Writes in progress on a dynamic buffer, keyed by resource pointer.  The CPU pointer the
    /// game was handed is only known when the write starts, but the data must be copied when it
    /// ends (before the pointer is invalidated).  Tuple is
    /// `(cpu_ptr, is_index_buffer, byte_width, map_type)`.
    ///
    /// Only populated with the `snapshot-dynamic-buffers` feature; empty otherwise.
    pub mapped_buffers: FnvHashMap<usize, (usize, bool, u32, u32)>,
    /// Diagnostic provenance for each captured mesh buffer, keyed by buffer pointer.  Written by
    /// the dynamic buffer capture paths, read and logged by the snapshot code.
    #[cfg(feature = "snapshot-dynamic-buffers")]
    pub buffer_capture_info: FnvHashMap<usize, BufferCaptureInfo>,
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
            // Allocate lazily (no capacity): these stay empty unless the snapshot-dynamic-buffers
            // feature is enabled and actively capturing, so this keeps the default build zero-cost.
            device_buffer_meta: FnvHashMap::default(),
            mapped_buffers: FnvHashMap::default(),
            #[cfg(feature = "snapshot-dynamic-buffers")]
            buffer_capture_info: FnvHashMap::default(),
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