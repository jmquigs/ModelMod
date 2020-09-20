#![allow(non_snake_case)]
use std::os::raw::c_char;
use winapi::um::winnt::WCHAR;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;

pub enum ModType {
    None = 0,
    CPUAdditive,
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
pub struct ModData {
    pub numbers: ModNumbers,
    pub texPath0: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath1: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath2: [WCHAR; MAX_TEX_PATH_LEN],
    pub texPath3: [WCHAR; MAX_TEX_PATH_LEN],
    pub modName: [WCHAR; MAX_MOD_NAME_LEN],
    pub parentModName: [WCHAR; MAX_MOD_NAME_LEN],
    pub _pixelShaderPath: [WCHAR; MAX_TEX_PATH_LEN], // not used
}

#[repr(C)]
pub struct SnapshotData {
    pub sd_size: u32,
    pub prim_type: i32,
    pub base_vertex_index: i32,
    pub min_vertex_index: u32,
    pub num_vertices: u32,
    pub start_index: u32,
    pub prim_count: u32,

    /// Vertex buffer pointer
    pub vert_decl: *mut IDirect3DVertexDeclaration9,
    /// Index buffer pointer
    pub index_buffer: *mut IDirect3DIndexBuffer9,
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
    device: *mut IDirect3DDevice9,
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
}

#[derive(Copy, Clone)]
pub struct InteropState {
    pub callbacks: ManagedCallbacks,
    pub conf_data: ConfData,
    pub loading_mods: bool,
    pub done_loading_mods: bool,
}