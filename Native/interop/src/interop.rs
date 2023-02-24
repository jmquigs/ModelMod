

use std::os::raw::c_char;
use std;
use util;
use d3dx;
use shared_dx::util::write_log_file;
use global_state::HookState;
use types::interop::*;

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
    let local_gs_addr = global_state::get_global_state_ptr() as u64;
    if global_state_pointer != local_gs_addr {
        write_log_file(&format!(
            "WARNING: OnInitialized's global state address {:x} differs from input param {:x}",
            local_gs_addr, global_state_pointer
        ));
    }

    let global_hookstate = global_state_pointer as *mut HookState;

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
