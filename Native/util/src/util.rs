


use aho_corasick::AhoCorasick;
use shared_dx::defs_dx9::DWORD;
use shared_dx::util::{write_log_file, set_log_file_path};
use winapi::shared::minwindef::{FARPROC, HMODULE, UINT};
use winapi::shared::windef::{HWND};
use winapi::shared::winerror::ERROR_FILE_NOT_FOUND;
use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryW};
use winapi::um::winuser::{GetAncestor, GetForegroundWindow, GetParent};

use std::cell::RefCell;
use std::ffi::OsString;
use std::sync::MutexGuard;

use shared_dx::error::*;

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

pub unsafe fn reg_query_string(path:&str, key:&str) -> Result<String> {
    use winapi::ctypes::c_void;
    use winapi::shared::winerror::ERROR_SUCCESS;
    use winapi::um::winreg::*;

    use std::os::windows::prelude::*;
    let sk = to_wide_str(path);
    let kv = to_wide_str(key);
    let mut max_path: DWORD = 65535;
    // path could have wide chars, use u16
    let mut out_buf: Vec<u16> = Vec::with_capacity(max_path as usize);

    // max path input is in bytes
    max_path *= 2;
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
            "Error reading {}\\{} registry key as string: {}",
            path, key, res
        )));
    }
    //println!("bytes read from registry {}", max_path);
    // convert bytes read to chars and remove null terminator
    let nchars = (max_path / 2) - 1;
    let wslice = std::slice::from_raw_parts(out_buf.as_mut_ptr(), nchars as usize);
    let wpath = OsString::from_wide(wslice).into_string()?;
    Ok(wpath)
}

pub unsafe fn reg_query_root_dword(key:&str) -> Result<DWORD> {
    let reg_root = get_mm_reg_key();
    // find the MM install directory, this must be set in the registry by the launcher.
    // the launcher will also set whether MM is active.
    let res = reg_query_dword(reg_root, key)?;
    Ok(res)
}

pub unsafe fn reg_query_dword(path:&str, key:&str) -> Result<DWORD> {
    use winapi::ctypes::c_void;
    use winapi::shared::winerror::ERROR_SUCCESS;
    use winapi::um::winreg::*;

    let sk = to_wide_str(path);
    let kv = to_wide_str(key);
    let mut out_val: DWORD = 0;
    let p_out_val: *mut c_void = std::mem::transmute(&mut out_val);
    let mut out_val_dw: DWORD = std::mem::size_of::<DWORD>() as DWORD;
    let res = RegGetValueW(
        HKEY_CURRENT_USER,
        sk.as_ptr(),
        kv.as_ptr(),
        RRF_RT_REG_DWORD,
        std::ptr::null_mut(),
        p_out_val,
        &mut out_val_dw,
    );
    match res as DWORD {
        ERROR_SUCCESS => {}
        ERROR_FILE_NOT_FOUND => {
            return Err(HookError::NoRegistryKey(format!(
                "{}\\{} not found", path, key
            )));
        }
        _ => {
            return Err(HookError::ConfReadFailed(format!(
                "Error reading {}\\{} registry key as dword: {}",
                path, key, res
            )));
        }
    }

    Ok(out_val)
}

pub fn get_mm_conf_info() -> Result<(bool, Option<String>)> {
    unsafe {
        let reg_root = get_mm_reg_key();
        // find the MM install directory, this must be set in the registry by the launcher.
        // the launcher will also set whether MM is active.
        let wpath = reg_query_string(reg_root, "MMRoot")?;


        // first check if it is active
        // {
        //     let sk = to_wide_str(reg_root);
        //     let kv = to_wide_str("Active");
        //     let mut active: DWORD = 0;
        //     let p_active: *mut c_void = std::mem::transmute(&mut active);
        //     let mut out_active_sz: DWORD = std::mem::size_of::<DWORD>() as DWORD;
        //     let res = RegGetValueW(
        //         HKEY_CURRENT_USER,
        //         sk.as_ptr(),
        //         kv.as_ptr(),
        //         RRF_RT_REG_DWORD,
        //         std::ptr::null_mut(),
        //         p_active,
        //         &mut out_active_sz,
        //     );
        //     if res as DWORD != ERROR_SUCCESS {
        //         return Err(HookError::ConfReadFailed(format!("Error reading Active registry key: {}.  You must start ModelMod using its launcher.", res)));
        //     }
        //     if active != 1 {
        //         return Ok((false, None));
        //     }
        // }

        // its active, so get path and make sure it exists
        {


            // check if path exists

            use std::path::Path;
            if !Path::new(&wpath).exists() {
                return Err(HookError::ConfReadFailed(format!(
                    "ModelMod path read from registry, {}, does not exist",
                    wpath
                )));
            }

            Ok((true, Some(wpath)))
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
                ))).map(String::from)
        })
}

/// Get a string from wide slice using exact length of slice
pub fn from_wide_fixed(ws: &[u16]) -> Result<String> {
    use std::os::windows::prelude::*;

    let len = ws.len();
    let s = unsafe { std::slice::from_raw_parts(ws.as_ptr(), len) };
    let s = OsString::from_wide(s).into_string()?;
    Ok(s)
}

/// Get a string from wide slice.  This version will look for a "null" character in the slice and
/// stop at that character, excluding it and everything after it.  If no null character found
/// it takes the whole slice.
pub fn from_wide_str(ws: &[u16]) -> Result<String> {
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

    from_wide_fixed(ws)
}

/// Convert string to wide array and append null
pub fn to_wide_str(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

pub fn get_module_name() -> Result<String> {
    use std::os::windows::prelude::*;
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
            let s = OsString::from_wide(s).into_string()?;
            Ok(s)
        }
    }
}

pub fn get_module_name_base() -> Result<String> {
    get_module_name()
        .and_then(|mod_name| {
            use std::path::PathBuf;

            let stem = {
                let pb = PathBuf::from(&mod_name);
                let s = pb
                    .file_stem()
                    .ok_or(HookError::ConfReadFailed("no stem".to_owned()))?;
                let s = s
                    .to_str()
                    .ok_or(HookError::ConfReadFailed("cant't make stem".to_owned()))?;
                (*s).to_owned()
            };

            Ok(stem)
        })
}

pub fn mm_verify_load() -> Option<String> {
    match get_mm_conf_info() {
        Ok((true, Some(dir))) => return Some(dir),
        Ok((false, _)) => {
            write_log_file(&format!("ModelMod not initializing because it is not active (did you start it with the ModelMod launcher?)"));
            return None;
        }
        Ok((true, None)) => {
            write_log_file(&format!("ModelMod not initializing because install dir not found (did you start it with the ModelMod launcher?)"));
            return None;
        }
        Err(e) => {
            write_log_file(&format!(
                "ModelMod not initializing due to conf error: {:?}",
                e
            ));
            return None;
        }
    };
}

pub fn format_time(time: &std::time::SystemTime) -> String {
    use chrono::prelude::*;
    let dt = DateTime::<Local>::from(*time);
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn aho_corasick_scan<'b, B, I, P>(patterns: I, numpat:usize, haystack: &'b B) -> Vec<Vec<(usize,usize)>>
    where
    B: ?Sized + AsRef<[u8]>,
    I: IntoIterator<Item = P>,
    P: AsRef<[u8]> {
    let mut outputs:Vec<Vec<(usize,usize)>> =
        (0..numpat).into_iter().map(|_u| vec![]).collect();

    let ac = AhoCorasick::new(patterns);
    ac.find_iter(&haystack).for_each(|mat| {
        outputs[mat.pattern()].push( (mat.start(), mat.end()) );
    });
    outputs
}

thread_local! {
    static LOG_WAS_INIT: RefCell<bool>  = RefCell::new(false);
}

pub fn log_initted_on_this_thread() -> bool {
    LOG_WAS_INIT.with(|was_init| {
        *was_init.borrow()
    })
}

pub fn set_log_initted_on_this_thread() {
    LOG_WAS_INIT.with(|was_init| {
        *was_init.borrow_mut() = true;
    });
}

/// Useful for tests to avoid log file getting put in wrong place.
#[allow(dead_code)]
pub fn init_log_no_root(file_name:&str) -> Result<()> {
    set_log_file_path(&"", &file_name)?;

    LOG_WAS_INIT.with(|was_init| {
        *was_init.borrow_mut() = true;
    });
    Ok(())
}

#[allow(dead_code)]
pub fn prep_log_file<'a>(_lock: &MutexGuard<()>, filename:&'a str) -> std::io::Result<&'a str> {
    prep_log_file_nolock(filename,true)
}

#[allow(dead_code)]
pub fn prep_log_file_nolock<'a>(filename:&'a str, remove:bool) -> std::io::Result<&'a str> {
    if remove && std::path::Path::new(filename).exists() {
        std::fs::remove_file(filename)?;
    }
    init_log_no_root(filename)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))?;
    Ok(filename)
}

pub use winapi::shared::d3d9::{IDirect3DBaseTexture9,
    IDirect3DVertexDeclaration9,IDirect3DIndexBuffer9,IDirect3DPixelShader9,
    IDirect3DVertexShader9};

#[cfg(test)]
mod tests {
    use super::*;



    #[test]
    pub fn test_nasty_string_utils() {
        // to_wide_str will append a null terminator
        let mut disgusting = super::to_wide_str("GROSS💩");
        // from_wide_fixed will take everything in the slice including the null
        assert_eq!("GROSS💩\u{0}", super::from_wide_fixed(&disgusting).unwrap());

        // from_wide_str will exclude the null and everything after it.
        let crap:Vec<u16> = vec![80, 81];
        disgusting.extend_from_slice(&crap);
        assert_eq!("GROSS💩", super::from_wide_str(&disgusting).unwrap());
        // buf if there is no null, it takes everything
        let crap:Vec<u16> = vec![71, 82, 79, 83, 83, 0xD83D, 0xDCA9];
        assert_eq!("GROSS💩", super::from_wide_str(&crap).unwrap());
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    pub fn test_get_mm_conf_info() {
        // TODO: actually add the requisite values to the registry instead of
        // just assuming they are there.
        // test the reg funcs
        unsafe {
            let active = super::reg_query_dword(get_mm_reg_key(), "Active").expect("doh no active key");
            assert_eq!(active, 1);
            let docroot = super::reg_query_string(get_mm_reg_key(), "DocRoot").expect("doh no active key");
            assert_eq!(docroot, "M:\\ModelMod\\TestData");
        }
        let res = get_mm_conf_info();
        match res {
            Err(e) => assert!(false, "conf test failed: {:?}", e),
            Ok((ref active, ref _path)) if !(*active) => {
                assert!(false, "mm should be active")
            }
            Ok((ref active, ref path)) if *active && path.is_none() => {
                assert!(false, "if active, path must be set")
            }
            Ok(_) => {}
        }
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    pub fn test_get_managed_dll_path() {
        if let Err(e) = get_managed_dll_path("M:\\modelmod") {
            // TODO unhardcode
            assert!(false, "file should exist: {:?}", e)
        }
        if let Ok(f) = get_managed_dll_path("C:\\Dev\\modelmod.foo") {
            // TODO unhardcode
            assert!(false, "file should not exist: {:?}", f)
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
