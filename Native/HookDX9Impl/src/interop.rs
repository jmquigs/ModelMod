use std::os::raw::c_char;
use winapi::um::winnt::WCHAR;

use hook_render;
use std;
use util;
use d3dx;
use shared_dx9::util::write_log_file;

#[derive(Copy, Clone)]
pub struct InteropState {
    pub callbacks: ManagedCallbacks,
    pub conf_data: ConfData,
    pub loading_mods: bool,
    pub done_loading_mods: bool,
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

pub enum ModType {
    None = 0,
    CPUAdditive,
    CPUReplacement,
    GPUReplacement,
    GPUPertubation,
    Deletion,
}

// #define MaxModTextures 4
// #define MaxModTexPathLen 8192 // Must match SizeConst attribute in managed code
// typedef WCHAR ModPath[MaxModTexPathLen];

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
    pub vert_decl: *mut hook_render::IDirect3DVertexDeclaration9,
    /// Index buffer pointer
    pub index_buffer: *mut hook_render::IDirect3DIndexBuffer9,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SnapshotResult {
    pub directory: [WCHAR; MAX_TEX_PATH_LEN],
    pub snap_file_prefix: [WCHAR; MAX_TEX_PATH_LEN],

    pub directory_len: i32,
    pub snap_file_prefix_len: i32,
}

pub struct NativeModData {
    pub mod_data: ModData,
    pub vb_data: *mut c_char,
    pub ib_data: *mut c_char,
    pub decl_data: *mut c_char,
    pub vb: *mut hook_render::IDirect3DVertexBuffer9,
    pub ib: *mut hook_render::IDirect3DIndexBuffer9,
    pub decl: *mut hook_render::IDirect3DVertexDeclaration9,
    pub textures: [hook_render::LPDIRECT3DTEXTURE9; 4],
    pub is_parent: bool,
    pub parent_mod_name: String,
    pub last_frame_render: u64, // only set for parent mods
    pub name: String,
    //IDirect3DPixelShader9* pixelShader;
}

impl NativeModData {
    pub fn mod_key(vert_count: u32, prim_count: u32) -> u32 {
        //https://en.wikipedia.org/wiki/Pairing_function#Cantor_pairing_function
        ((vert_count + prim_count) * (vert_count + prim_count + 1) / 2) + prim_count
    }
    pub fn recently_rendered(&self, curr_frame_num:u64) -> bool {
        if self.last_frame_render > curr_frame_num {
            // we rendered in the future, so I guess that is recent?
            return true;
        }
        curr_frame_num - self.last_frame_render <= 10 // last 150ms or so ought to be fine
    }
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
    device: *mut hook_render::IDirect3DDevice9,
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

lazy_static! {
    pub static ref LOG_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

unsafe fn loggit(prefix: &str, category: *const c_char, message: *const c_char) -> () {
    use std::ffi::CStr;

    let _lock = LOG_MUTEX.lock();

    // convert the c_strs to rust strs; if it works, we get a &str.  If it doesn't,
    // we get an error. format error to make a String, store that in a mutable to prevent drop,
    // and return a ref to the String for display.  amusingly the error contains the
    // debug representation of the string that couldn't be converted.  ^_^
    // TODO: when I am smarter, do this better or make it into a utility function.
    let mut cerr = String::new();
    let category = CStr::from_ptr(category).to_str().unwrap_or_else(|e| {
        cerr = format!("{:?} [conversion error: {}]", CStr::from_ptr(category), e);
        &cerr
    });
    let mut merr = String::new();
    let message = CStr::from_ptr(message).to_str().unwrap_or_else(|e| {
        merr = format!("{:?} [conversion error: {}]", CStr::from_ptr(message), e);
        &merr
    });

    if prefix == "" {
        write_log_file(&format!("[{}]: {}", category, message));
    } else {
        write_log_file(&format!("[{}:{}]: {}", prefix, category, message));
    };
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "stdcall" fn LogInfo(category: *const c_char, message: *const c_char) -> () {
    loggit("", category, message);
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "stdcall" fn LogWarn(category: *const c_char, message: *const c_char) -> () {
    loggit("WARN", category, message);
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "stdcall" fn LogError(category: *const c_char, message: *const c_char) -> () {
    loggit("ERROR", category, message);
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "stdcall" fn SaveTexture(index: i32, filepath: *const u16) -> bool {
    match d3dx::save_texture(index, filepath) {
        Ok(_) => true,
        Err(e) => {
            write_log_file(&format!("failed to save texture: {:?}", e));
            false
        }
    }
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "stdcall" fn OnInitialized(
    callbacks: *mut ManagedCallbacks,
    global_state_pointer: u64,
) -> i32 {
    use std::ffi::CStr;
    use std::ffi::CString;

    let on_init_error_code = 666;

    // reinit global state pointer.  technically we only really need to do this for the
    // tests, where we can have multiple copies of globals (see rt.sh for details).
    write_log_file(&format!(
        "OnInitialized called with global state address: {}",
        global_state_pointer
    ));
    let local_gs_addr = hook_render::get_global_state_ptr() as u64;
    if global_state_pointer != local_gs_addr {
        write_log_file(&format!(
            "WARNING: OnInitialized's global state address {:x} differs from input param {:x}",
            local_gs_addr, global_state_pointer
        ));
    }

    let global_hookstate: *mut hook_render::HookState =
        global_state_pointer as *mut hook_render::HookState;

    if global_hookstate == std::ptr::null_mut() {
        write_log_file("error: global state pointer is null");
        return 666;
    }
    if callbacks == std::ptr::null_mut() {
        write_log_file("error: no callbacks specified");
        return 666;
    }

    let mmpath = match util::get_mm_conf_info() {
        Ok((true, Some(mmpath))) => mmpath,
        Ok((a, b)) => {
            write_log_file(&format!("Unexpected conf return: {:?} {:?}", a, b));
            return on_init_error_code;
        }
        Err(e) => {
            write_log_file(&format!("Unexpected conf error value: {:?}", e));
            return on_init_error_code;
        }
    };

    // get module path (exe that has loaded this dll).
    let exemodule = match util::get_module_name() {
        Err(e) => {
            write_log_file(&format!(
                "Unexpected error getting module handle name: {:?}",
                e
            ));
            return on_init_error_code;
        }
        Ok(s) => s,
    };

    let mut mmpath = util::to_wide_str(&mmpath);
    let mut exemodule = util::to_wide_str(&exemodule);
    let cd = ((*callbacks).SetPaths)(mmpath.as_mut_ptr(), exemodule.as_mut_ptr());
    if cd == std::ptr::null_mut() {
        write_log_file(&format!(
            "error calling setpaths, returned conf data is null"
        ));
        return on_init_error_code;
    }

    let is = InteropState {
        callbacks: (*callbacks),
        conf_data: (*cd),
        loading_mods: false,
        done_loading_mods: false,
    };

    (*global_hookstate).interop_state = Some(is);

    0
}
