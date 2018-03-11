use winapi::um::winnt::{WCHAR};
use std::os::raw::c_char;

use std;
use util;
use util::write_log_file;

#[repr(C)]
pub struct ConfData {
	// Note: marshalling to bool requires [<MarshalAs(UnmanagedType.I1)>] on the field in managed code; otherwise it will try to marshall it as a 4 byte BOOL,
	// which has a detrimental effect on subsequent string fields!
	RunModeFull:bool,
	LoadModsOnStart:bool,
	InputProfile: [c_char;512],
}

type SetPathsCb = unsafe extern "stdcall" fn(dllpath:*mut WCHAR, exemodule:*mut WCHAR) -> *mut ConfData;

#[repr(C)]
pub struct ManagedCallbacks {
    SetPaths: SetPathsCb,
    LoadModDB: *mut u64,
    GetModCount: *mut u64,
    GetModData: *mut u64,
    FillModData: *mut u64,
    TakeSnapshot: *mut u64,
    GetLoadingState: *mut u64,
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "C" fn OnInitialized(callbacks: *mut ManagedCallbacks) -> i32 {
    use std::ffi::CString;
    use std::ffi::CStr;

    write_log_file("OnInitialized called");
    // TODO: unhardcode
    let mut mmpath = util::to_wide_str("D:\\Dev\\ModelMod\\xx.dll");
    let mut exemodule = util::to_wide_str("D:\\Guild Wars 2\\gw2.exe");
    let cd = ((*callbacks).SetPaths)(mmpath.as_mut_ptr(), exemodule.as_mut_ptr());
    if cd != std::ptr::null_mut() {
        write_log_file(&format!("run mode full: {}", (*cd).RunModeFull));
        write_log_file(&format!("load mods on start: {}", (*cd).LoadModsOnStart));
        let ip = CStr::from_ptr((*cd).InputProfile.as_mut_ptr());
        write_log_file(&format!("input profile: {:?}", ip));
        0
    } else {
        666
    }    
}