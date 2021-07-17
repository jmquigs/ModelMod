use std;
use winapi;

use winapi::shared::minwindef::{FARPROC, HMODULE, UINT};
use winapi::shared::windef::{HWND};
use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryW};
use winapi::um::winuser::{GetAncestor, GetForegroundWindow, GetParent};

use std::ffi::OsString;

use shared_dx9::error::*;

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

pub fn appwnd_is_foreground(app_wnd: HWND) -> bool {
    const GA_ROOTOWNER: UINT = 3;

    unsafe {
        if app_wnd == std::ptr::null_mut() {
            return false;
        }
        let focus_wnd = GetForegroundWindow();
        let mut is_focused = focus_wnd == app_wnd;
        if !is_focused {
            // check parent
            let par = GetParent(app_wnd);
            is_focused = par == focus_wnd;
        }
        if !is_focused {
            // check root owner
            let own = GetAncestor(app_wnd, GA_ROOTOWNER);
            is_focused = own == focus_wnd;
        }
        is_focused
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
        use winapi::ctypes::c_void;
        use winapi::shared::minwindef::DWORD;
        use winapi::shared::winerror::ERROR_SUCCESS;
        use winapi::um::winreg::*;

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

pub fn get_managed_dll_path(mm_root: &str) -> Result<String> {
    use std::path::PathBuf;

    let subdir_paths = ["", "Bin", "Release", "Debug"];
    // save the full path list so that we can reference it in case of err
    let full_paths: Vec<PathBuf> = subdir_paths
        .iter()
        .map(|spath| {
            let mut path = PathBuf::from(mm_root);
            path.push(spath);
            path.push("MMManaged.dll");
            path
        })
        .collect();
    full_paths
        .iter()
        .filter(|p| p.as_path().exists())
        .take(1)
        .next()
        .ok_or(HookError::UnableToLocatedManagedDLL(format!(
            "Searched: {:?}",
            full_paths
        )))
        .and_then(|found| {
            found
                .as_path()
                .to_str()
                .ok_or(HookError::UnableToLocatedManagedDLL(format!(
                    "could not convert located path to string: {:?}",
                    found
                )))
                .and_then(|spath| Ok(String::from(spath)))
        })
}
pub fn from_wide_str(ws: &[u16]) -> Result<String> {
    use std::os::windows::prelude::*;
    // use winapi::shared::minwindef::DWORD;
    // use winapi::um::libloaderapi::*;

    // HACK: should use the widestring crate
    // find the "null terminator". I don't have a great understanding of wide strings but I think
    // its valid for some to be null terminated, but maybe not all.
    // the MM managed code will fill in the whole array with null, and it looks like from_wide
    // just happily treats those as part of the string - i.e finding the length is our problem.
    // this would probably break paths that actually have certain unicode chars in them, but oh well.
    let mut null_pos = None;
    for (i,c) in ws.iter().enumerate() {
        if *c == 0 {
            null_pos = Some(i);
            break;
        }
    }
    let ws = match null_pos {
        Some(pos) => &ws[0..pos],
        None => ws
    };

    let len = ws.len();
    let s = unsafe { std::slice::from_raw_parts(ws.as_ptr(), len as usize) };
    let s = OsString::from_wide(&s).into_string()?;
    Ok(s)
}
pub fn to_wide_str(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

pub fn get_module_name() -> Result<String> {
    use std::os::windows::prelude::*;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::libloaderapi::*;

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


pub use winapi::shared::d3d9::{IDirect3DBaseTexture9,
    IDirect3DVertexDeclaration9,IDirect3DIndexBuffer9,IDirect3DPixelShader9,
    IDirect3DVertexShader9};

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
            Ok((ref active, ref _path)) if *active == false => {
                assert!(false, format!("mm should be active"))
            }
            Ok((ref active, ref path)) if *active == true && path.is_none() => {
                assert!(false, format!("if active, path must be set"))
            }
            Ok(_) => {}
        }
    }

    #[test]
    pub fn test_get_managed_dll_path() {
        if let Err(e) = get_managed_dll_path("C:\\Dev\\modelmod.new") {
            // TODO unhardcode
            assert!(false, format!("file should exist: {:?}", e))
        }
        if let Ok(f) = get_managed_dll_path("C:\\Dev\\modelmod.foo") {
            // TODO unhardcode
            assert!(false, format!("file should not exist: {:?}", f))
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
