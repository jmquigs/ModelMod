use winapi::um::winnt::WCHAR;
use std::os::raw::c_char;

use hookd3d9;
use std;
use util;
use util::write_log_file;

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
}

pub struct NativeModData {
    pub mod_data: ModData,
    pub vb_data: *mut c_char,
    pub ib_data: *mut c_char,
    pub decl_data: *mut c_char,
    pub vb: *mut hookd3d9::IDirect3DVertexBuffer9,
    pub ib: *mut hookd3d9::IDirect3DIndexBuffer9,
    pub decl: *mut hookd3d9::IDirect3DVertexDeclaration9,
    // TODO:
    //IDirect3DBaseTexture9* texture[MaxModTextures];
    //IDirect3DPixelShader9* pixelShader;
}

impl NativeModData {
    pub fn mod_key(vert_count: u32, prim_count: u32) -> u32 {
        //https://en.wikipedia.org/wiki/Pairing_function#Cantor_pairing_function
        ((vert_count + prim_count) * (vert_count + prim_count + 1) / 2) + prim_count
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
    device: *mut u64,   /*IDirect3DDevice9*/
    snapdata: *mut u64, /*SnapshotData*/
);
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
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "C" fn OnInitialized(callbacks: *mut ManagedCallbacks, cookie: u64) -> i32 {
    use std::ffi::CString;
    use std::ffi::CStr;

    // can't use GLOBAL_STATE_LOCK here (this thread originates in the CLR, so does not have
    // access to that global, or any rust global).  So we have to manufacture a pointer
    // to global state from the cookie and set the interop state directly.
    write_log_file(&format!("OnInitialized called with cookie: {}", cookie));
    let local_gs_addr = hookd3d9::get_global_state_ptr() as u64;
    if cookie != local_gs_addr {
        write_log_file(&format!(
            "WARNING: OnInitialized's global state address {:x} differs from cookie {:x}",
            local_gs_addr, cookie
        ));
    }

    let global_hookstate: *mut hookd3d9::HookState = cookie as *mut hookd3d9::HookState;

    if callbacks == std::ptr::null_mut() {
        write_log_file("error: no callbacks specified");
        return 666;
    }
    // TODO: unhardcode
    let mut mmpath = util::to_wide_str("D:\\Dev\\ModelMod\\xx.dll");
    let mut exemodule = util::to_wide_str("D:\\Guild Wars 2\\gw2.exe");
    let cd = ((*callbacks).SetPaths)(mmpath.as_mut_ptr(), exemodule.as_mut_ptr());
    if cd == std::ptr::null_mut() {
        write_log_file(&format!(
            "error calling setpaths, returned conf data is null"
        ));
        return 666;
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
