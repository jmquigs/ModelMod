use std;
use winapi;

use winapi::shared::minwindef::HINSTANCE__;
use winapi::um::libloaderapi::{LoadLibraryW, GetProcAddress, FreeLibrary};
use winapi::shared::minwindef::{HMODULE,FARPROC};
use winapi::ctypes::c_void;

#[derive(Debug,Clone)]
pub enum HookError {
    ProtectFailed,
    LoadLibFailed(String),
    GetProcAddressFailed(String),
    CLRInitFailed(String),
    NulError(std::ffi::NulError),
    GlobalStateCopyFailed,
}

impl std::convert::From<std::ffi::NulError> for HookError {
    fn from(error: std::ffi::NulError) -> Self {
        HookError::NulError(error)
    }
}

pub type Result<T> = std::result::Result<T, HookError>;

pub fn write_log_file(format:String) -> () {
    use std::io::Write;
    use std::fs::OpenOptions;

    let w = || -> std::io::Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open("D:\\Temp\\rd3dlog.txt")?; // TODO: duh, unhardcode
        writeln!(f, "{}\r", format)?;
        Ok(())
    };

    w().unwrap_or_else(|e| eprintln!("oops can't write log file: {}", e));
}

pub unsafe fn protect_memory(target: *mut winapi::ctypes::c_void, size:usize, protection:u32) -> Result<u32> {
    let process = winapi::um::processthreadsapi::GetCurrentProcess();
    let mut old_protection = winapi::um::winnt::PAGE_READWRITE;    
    if winapi::um::memoryapi::VirtualProtectEx(process, 
            target as *mut winapi::ctypes::c_void, 
            size, 
            protection, 
            (&mut old_protection) as *mut u32) > 0 {
                Ok(old_protection)
    } else {
        Err(HookError::ProtectFailed)
    }    
}

pub unsafe fn unprotect_memory(target: *mut winapi::ctypes::c_void, size:usize) -> Result<u32> {
    protect_memory(target, size, winapi::um::winnt::PAGE_READWRITE)
}

pub fn load_lib(name:&str) -> Result<HMODULE> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(once(0)).collect();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle == std::ptr::null_mut() {
        Err(HookError::LoadLibFailed(name.to_owned()))
    } else {
        Ok(handle)
    }
}

pub fn unload_lib(h:HMODULE) -> Result<()> {
    if unsafe { FreeLibrary(h) } == 0 {
        Err(HookError::LoadLibFailed(format!("Unload of the library {:?} failed", h)))
    } else {
        Ok(())
    }
}

pub fn get_proc_address(h:HMODULE, name:&str) -> Result<FARPROC> {
    use std::ffi::CString;

    if h == std::ptr::null_mut() {
        return Err(HookError::GetProcAddressFailed("null handle".to_owned()));
    }
    let csname = CString::new(name)?;
    let addr = unsafe { GetProcAddress(h, csname.as_ptr()) };
    if addr == std::ptr::null_mut() {
        Err(HookError::GetProcAddressFailed(format!("{} not found in module", name)))
    } else {
        Ok(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_load_lib() {
        let _r = load_lib("unlikely_ducksarecool.dll")
        .map(|h| {
            assert!(false, "Expected Err but got {:?}", h)
        });

        let _r = load_lib("d3d9.dll")
        .map(|h| {
            let _r = get_proc_address(h, "Direct3DCreate9")
            .map_err(|err| {
                assert!(false, "Expected Ok but got {:?}", err)
            });

            let _r = get_proc_address(h, "NOTTHEREDirect3DCreate9")
            .map(|res| {
                assert!(false, "Expected Err but got {:?}", res)
            });

            unload_lib(h)
            .map_err(|err| {
                assert!(false, "Expected Ok but got {:?}", err)
            })
        })
        .map_err(|err| {
            assert!(false, "Expected Ok but got {:?}", err)
        });
    }
}