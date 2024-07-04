#![allow(non_snake_case)]
use std::ffi::c_void;
use std::os::raw::c_char;
use winapi::um::winnt::WCHAR;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;

pub enum ModType {
    None = 0,
    GPUAdditive,
    CPUReplacement,
    GPUReplacement,
    GPUPertubation,
    Deletion,
}

const MAX_TEX_PATH_LEN: usize = 8192;
const MAX_MOD_NAME_LEN: usize = 1024;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ModNumbers {
    pub mod_type: i32,
    pub prim_type: i32,
    pub vert_count: i32,
    pub prim_count: i32,
    pub index_count: i32,
    pub ref_vert_count: i32,
    pub ref_prim_count: i32,
    pub decl_size_bytes: i32,
    pub vert_size_bytes: i32,
    pub index_elem_size_bytes: i32,
}
#[repr(C)]
#[derive(Copy, Clone)]
/// Contains information and data associated with a mod, but _not_ any D3D resources.
/// This structure is passed to/from managed code so must have a defined layout, and can
/// only contain types that can be marshalled over the interop boundary.
pub struct ModData {
    pub numbers: ModNumbers,
    pub update_tangent_space: i32,
    pub texPath0: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath1: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath2: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath3: [WCHAR; MAX_TEX_PATH_LEN],
    pub modName: [WCHAR; MAX_MOD_NAME_LEN],
    pub parentModName: [WCHAR; MAX_MOD_NAME_LEN],
    pub _pixelShaderPath: [WCHAR; MAX_TEX_PATH_LEN], // not used
}

impl ModData {
    pub fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[cfg(target_pointer_width = "32")]
const DX9_PAD_SIZE:usize = 11;
#[cfg(target_pointer_width = "64")]
const DX9_PAD_SIZE:usize = 13;

#[repr(C, packed(4))]
#[derive(Copy,Clone,Debug)]
pub struct D3D9SnapshotRendData {
    /// Vertex declaration pointer.
    pub vert_decl: *mut IDirect3DVertexDeclaration9,
    /// Index buffer pointer
    pub index_buffer: *mut IDirect3DIndexBuffer9,
    /// Increases size of this struct to match D3D11 size.
    /// See comment for `SnapshotRendData` for why this is necessary.
    pub _padx: [u32; DX9_PAD_SIZE],
}
impl D3D9SnapshotRendData {
    pub fn new() -> Self {
        Self {
            vert_decl: std::ptr::null_mut(),
            index_buffer: std::ptr::null_mut(),
            _padx: [0xDEADBEEF; DX9_PAD_SIZE],
        }
    }
    pub fn from(vert_decl: *mut IDirect3DVertexDeclaration9, index_buffer: *mut IDirect3DIndexBuffer9) -> Self {
        Self {
            vert_decl,
            index_buffer,
            _padx: [0xDEADBEEF; DX9_PAD_SIZE],
        }
    }
}

#[repr(C, packed(4))]
#[derive(Copy,Clone)]
pub struct D3D11SnapshotRendData {
    // put pointers first to keep them aligned, especially for ib and vb
    pub layout_elems: *const D3D11_INPUT_ELEMENT_DESC,
    pub ib_data: *const u8,
    pub vb_data: *const u8,
    pub act_tex_indices: *const u32,
    pub layout_size_bytes: u64,
    pub ib_size_bytes: u64,
    pub vb_size_bytes: u64,
    pub ib_index_size_bytes: u32,
    pub vb_vert_size_bytes: u32,
    pub num_act_tex_indices: u32,
}
impl D3D11SnapshotRendData {
    pub fn new() -> Self {
        Self {
            layout_elems: std::ptr::null(),
            layout_size_bytes: 0,
            ib_data: std::ptr::null(),
            ib_size_bytes: 0,
            ib_index_size_bytes: 0,
            vb_data: std::ptr::null(),
            vb_size_bytes: 0,
            vb_vert_size_bytes: 0,
            act_tex_indices: std::ptr::null(),
            num_act_tex_indices: 0,
        }
    }
}

/// See comment for `SnapshotRendData` for why this is necessary.
macro_rules! check_size {
    ($name:ident) => {
        if std::mem::size_of::<D3D9SnapshotRendData>() == std::mem::size_of::<D3D11SnapshotRendData>() {
            1
        } else {
            1/0 // Make compile fail because D3D9SnapshotRendData and D3D11SnapshotRendData have different size
        }
    }
}
const _HACK_SIZE_CHECK: i32 = check_size!(__unused);


/// Union type to represent D3D version-specific data.  Note, due to a quirk with
/// how the .Net marshals these, these structs must be _the same size_.  Failure to do this
/// will cause the .net marshal to read a short amount of bytes for the smaller struct,
/// most likely resulting in a crash.
/// Note that in Rust, `packed` must be used to lower the bytes from the
/// default, whereas `align` is use to raise it.
#[repr(C, packed(4))]
pub union SnapshotRendData {
    pub d3d9: D3D9SnapshotRendData,
    pub d3d11: D3D11SnapshotRendData,
}

#[repr(C, packed(4))]
pub struct SnapshotData {
    pub sd_size: u32,
    pub was_reset: bool, 
    pub clear_sd_on_reset: bool, 
    pub prim_type: i32,
    pub base_vertex_index: i32,
    pub min_vertex_index: u32,
    pub num_vertices: u32,
    pub start_index: u32,
    pub prim_count: u32,

    pub rend_data: SnapshotRendData,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SnapshotResult {
    pub directory: [WCHAR; MAX_TEX_PATH_LEN],
    pub snap_file_prefix: [WCHAR; MAX_TEX_PATH_LEN],

    pub directory_len: i32,
    pub snap_file_prefix_len: i32,
}


type SetPathsCB =
    unsafe extern "stdcall" fn(dllpath: *mut WCHAR, exemodule: *mut WCHAR) -> *mut ConfData;
type LoadModDBCB = unsafe extern "stdcall" fn() -> i32;
type GetModCountCB = unsafe extern "stdcall" fn() -> i32;
type GetModDataCB = unsafe extern "stdcall" fn(modIndex: i32) -> *mut ModData;
type FillModDataCB = unsafe extern "stdcall" fn(
    modIndex: i32,
    declData: *mut u8,
    declSize: i32,
    vbData: *mut u8,
    vbSize: i32,
    ibData: *mut u8,
    ibSize: i32,
) -> i32;
type TakeSnapshotCB = unsafe extern "stdcall" fn(
    device: *mut c_void,  // *mut IDirect3DDevice9 or *mut IDirect3D11Device
    snapdata: *mut SnapshotData,
) -> i32;
type GetSnapshotResultCB = unsafe extern "stdcall" fn() -> *mut SnapshotResult;

type GetLoadingStateCB = unsafe extern "stdcall" fn() -> i32;


#[repr(C)]
#[derive(Copy, Clone)]
pub struct ManagedCallbacks {
    pub SetPaths: SetPathsCB,
    pub LoadModDB: LoadModDBCB,
    pub GetModCount: GetModCountCB,
    pub GetModData: GetModDataCB,
    pub FillModData: FillModDataCB,
    pub TakeSnapshot: TakeSnapshotCB,
    pub GetLoadingState: GetLoadingStateCB,
    pub GetSnapshotResult: GetSnapshotResultCB,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ConfData {
    // Note: marshalling to bool requires [<MarshalAs(UnmanagedType.I1)>] on the field in managed code; otherwise it will try to marshall it as a 4 byte BOOL,
    // which has a detrimental effect on subsequent string fields!
    pub RunModeFull: bool,
    pub LoadModsOnStart: bool,
    pub InputProfile: [c_char; 512],
    pub MinimumFPS: i32,
    pub ProfileKey: [c_char; 512],
}

#[derive(Copy, Clone)]
pub struct InteropState {
    pub callbacks: ManagedCallbacks,
    pub conf_data: ConfData,
    pub loading_mods: bool,
    pub done_loading_mods: bool,
}