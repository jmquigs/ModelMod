use std;
use winapi;

use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryW};
use winapi::shared::minwindef::{FARPROC, HMODULE};

use std::ffi::OsString;

#[derive(Debug, Clone)]
pub enum HookError {
    ProtectFailed,
    LoadLibFailed(String),
    GetProcAddressFailed(String),
    CLRInitFailed(String),
    NulError(std::ffi::NulError),
    GlobalStateCopyFailed,
    Direct3D9InstanceNotFound,
    CreateDeviceFailed(i32),
    ConfReadFailed(String),
    FailedToConvertString(OsString),
    ModuleNameError(String),
    D3D9HookFailed,
    D3D9DeviceHookFailed,
    GlobalLockError,
}

impl std::convert::From<std::ffi::NulError> for HookError {
    fn from(error: std::ffi::NulError) -> Self {
        HookError::NulError(error)
    }
}

impl std::convert::From<std::ffi::OsString> for HookError {
    fn from(error: std::ffi::OsString) -> Self {
        HookError::FailedToConvertString(error)
    }
}

pub type Result<T> = std::result::Result<T, HookError>;

pub fn write_log_file(format: &str) -> () {
    use std::io::Write;
    use std::fs::OpenOptions;

    let tid = std::thread::current().id();

    let w = || -> std::io::Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open("D:\\Temp\\rd3dlog.txt")?; // TODO: duh, unhardcode
        writeln!(f, "{:?}: {}\r", tid, format)?;
        Ok(())
    };

    w().unwrap_or_else(|e| eprintln!("oops can't write log file: {}", e));
}

pub unsafe fn protect_memory(
    target: *mut winapi::ctypes::c_void,
    size: usize,
    protection: u32,
) -> Result<u32> {
    let process = winapi::um::processthreadsapi::GetCurrentProcess();
    let mut old_protection = winapi::um::winnt::PAGE_READWRITE;
    if winapi::um::memoryapi::VirtualProtectEx(
        process,
        target as *mut winapi::ctypes::c_void,
        size,
        protection,
        (&mut old_protection) as *mut u32,
    ) > 0
    {
        Ok(old_protection)
    } else {
        Err(HookError::ProtectFailed)
    }
}

pub unsafe fn unprotect_memory(target: *mut winapi::ctypes::c_void, size: usize) -> Result<u32> {
    protect_memory(target, size, winapi::um::winnt::PAGE_READWRITE)
}

pub fn load_lib(name: &str) -> Result<HMODULE> {
    let wide: Vec<u16> = to_wide_str(name);
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle == std::ptr::null_mut() {
        Err(HookError::LoadLibFailed(name.to_owned()))
    } else {
        Ok(handle)
    }
}

pub fn unload_lib(h: HMODULE) -> Result<()> {
    if unsafe { FreeLibrary(h) } == 0 {
        Err(HookError::LoadLibFailed(format!(
            "Unload of the library {:?} failed",
            h
        )))
    } else {
        Ok(())
    }
}

pub fn get_proc_address(h: HMODULE, name: &str) -> Result<FARPROC> {
    use std::ffi::CString;

    if h == std::ptr::null_mut() {
        return Err(HookError::GetProcAddressFailed("null handle".to_owned()));
    }
    let csname = CString::new(name)?;
    let addr = unsafe { GetProcAddress(h, csname.as_ptr()) };
    if addr == std::ptr::null_mut() {
        Err(HookError::GetProcAddressFailed(format!(
            "{} not found in module",
            name
        )))
    } else {
        Ok(addr)
    }
}

#[cfg(test)]
fn get_mm_reg_key() -> &'static str {
    "Software\\ModelModTEST"
}
#[cfg(not(test))]
fn get_mm_reg_key() -> &'static str {
    "Software\\ModelMod"
}
pub fn get_mm_conf_info() -> Result<(bool, Option<String>)> {
    unsafe {
        let reg_root = get_mm_reg_key();
        // find the MM install directory, this must be set in the registry by the launcher.
        // the launcher will also set whether MM is active.
        use winapi::um::winreg::*;
        use winapi::shared::minwindef::DWORD;
        use winapi::shared::winerror::ERROR_SUCCESS;
        use winapi::ctypes::c_void;

        use std::os::windows::prelude::*;

        // first check if it is active
        {
            let sk = to_wide_str(reg_root);
            let kv = to_wide_str("Active");
            let mut active: DWORD = 0;
            let p_active: *mut c_void = std::mem::transmute(&mut active);
            let mut out_active_sz: DWORD = std::mem::size_of::<DWORD>() as DWORD;
            let res = RegGetValueW(
                HKEY_CURRENT_USER,
                sk.as_ptr(),
                kv.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                p_active,
                &mut out_active_sz,
            );
            if res as DWORD != ERROR_SUCCESS {
                return Err(HookError::ConfReadFailed(format!("Error reading Active registry key: {}.  You must start ModelMod using its launcher.", res)));
            }
            if active != 1 {
                return Ok((false, None));
            }
        }

        // its active, so get path and make sure it exists
        {
            let sk = to_wide_str(reg_root);
            let kv = to_wide_str("MMRoot");
            let mut max_path: DWORD = 65535;
            // path could have wide chars, use u16
            let mut out_buf: Vec<u16> = Vec::with_capacity(max_path as usize);

            // max path input is in bytes
            max_path = max_path * 2;
            let res = RegGetValueW(
                HKEY_CURRENT_USER,
                sk.as_ptr(),
                kv.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                out_buf.as_mut_ptr() as *mut c_void,
                &mut max_path,
            );
            if res as DWORD != ERROR_SUCCESS {
                return Err(HookError::ConfReadFailed(format!(
                    "Error reading MMRoot registry key: {}",
                    res
                )));
            }
            //println!("bytes read from registry {}", max_path);
            // convert bytes read to chars and remove null terminator
            let nchars = (max_path / 2) - 1;
            let wslice = std::slice::from_raw_parts(out_buf.as_mut_ptr(), nchars as usize);
            let wpath = OsString::from_wide(wslice).into_string()?;

            // check if path exists

            use std::path::Path;
            if !Path::new(&wpath).exists() {
                return Err(HookError::ConfReadFailed(format!(
                    "ModelMod path read from registry, {}, does not exist",
                    wpath
                )));
            }

            return Ok((true, Some(wpath)));
        }
    }
}

pub fn to_wide_str(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

pub fn get_module_name() -> Result<String> {
    use winapi::um::libloaderapi::*;
    use std::ffi::OsString;
    use std::os::windows::prelude::*;
    use winapi::shared::minwindef::DWORD;

    unsafe {
        let ssize = 65535;
        let mut mpath: Vec<u16> = Vec::with_capacity(ssize);

        let handle = GetModuleHandleW(std::ptr::null_mut());
        let r = GetModuleFileNameW(handle, mpath.as_mut_ptr(), ssize as DWORD);
        if r == 0 {
            return Err(HookError::ModuleNameError(format!(
                "failed to get module file name: {}",
                r
            )));
        } else {
            let s = std::slice::from_raw_parts(mpath.as_mut_ptr(), r as usize);
            let s = OsString::from_wide(&s).into_string()?;
            Ok(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_get_mm_conf_info() {
        // TODO: actually add the requisite values to the registry instead of
        // just assuming they are there.
        let res = get_mm_conf_info();
        match res {
            Err(e) => assert!(false, format!("conf test failed: {:?}", e)),
            Ok((ref active, ref path)) if *active == false => {
                assert!(false, format!("mm should be active"))
            }
            Ok((ref active, ref path)) if *active == true && path.is_none() => {
                assert!(false, format!("if active, path must be set"))
            }
            Ok(_) => {}
        }
    }

    #[test]
    pub fn test_load_lib() {
        let _r = load_lib("unlikely_ducksarecool.dll")
            .map(|h| assert!(false, "Expected Err but got {:?}", h));

        let _r = load_lib("d3d9.dll")
            .map(|h| {
                let _r = get_proc_address(h, "Direct3DCreate9")
                    .map_err(|err| assert!(false, "Expected Ok but got {:?}", err));

                let _r = get_proc_address(h, "NOTTHEREDirect3DCreate9")
                    .map(|res| assert!(false, "Expected Err but got {:?}", res));

                unload_lib(h).map_err(|err| assert!(false, "Expected Ok but got {:?}", err))
            })
            .map_err(|err| assert!(false, "Expected Ok but got {:?}", err));
    }
}
