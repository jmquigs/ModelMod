#![allow(non_snake_case)]

#![feature(test)]
extern crate test;

#[macro_use]
extern crate lazy_static;

#[cfg(windows)] extern crate winapi;

use winapi::um::libloaderapi::{LoadLibraryW, GetProcAddress};

mod hookd3d9;
mod util;

use util::write_log_file;
use util::Result;

type Direct3DCreate9Fn = unsafe extern "system" fn(sdk_ver: u32) -> *mut hookd3d9::IDirect3D9;

#[allow(unused)]
#[no_mangle]
pub extern "system" fn Direct3DCreate9(
     SDKVersion: u32,
) -> *mut u64 {
    match create_d3d(SDKVersion) {
        Ok(ptr) => ptr as *mut u64,
        Err(x) => {
            write_log_file(format!("create_d3d failed: {:?}", x));
            std::ptr::null_mut()
        }
    }
}

fn create_d3d(sdk_ver:u32) -> Result<*mut hookd3d9::IDirect3D9> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::CString;
    //use std::os::raw::c_void;

    let msg = "c:\\windows\\system32\\d3d9.dll";  // TODO: use get system directory
    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    unsafe { 
        let handle = LoadLibraryW(wide.as_ptr()) ;

        let fname = CString::new("Direct3DCreate9").unwrap();
        let addr = GetProcAddress(handle, fname.as_ptr());

        let addr = addr as *const ();

        //addr as *mut c_void as (extern fn(c: u32) -> u64)
        //let fn: extern fn(sdkver: u32) -> u64 = std::ptr::null_mut();
        let create:Direct3DCreate9Fn = std::mem::transmute(addr);

        let direct3d9 = (create)(sdk_ver);
        let direct3d9 = direct3d9 as *mut hookd3d9::IDirect3D9;
        write_log_file(format!("created d3d: {:x}", direct3d9 as u64));

        // get pointer to original vtable        
        let vtbl: *mut hookd3d9::IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);

        // todo: maybe will need to hook this
        // let iuvtbl = Box::new(IUnknownVtbl {
        //             AddRef: std::mem::transmute(std::ptr::null_mut()),
        //             QueryInterface: std::mem::transmute(std::ptr::null_mut()),
        //             Release: std::mem::transmute(std::ptr::null_mut())
        //         });
        // let iuvtbl = Box::into_raw(iuvtbl);

        // save pointer to real functions
        let real_create_device = (*vtbl).CreateDevice;

        // unprotect memory and slam the vtable
        // let process = winapi::um::processthreadsapi::GetCurrentProcess();
        // let protection = winapi::um::winnt::PAGE_READWRITE;
        // let mut old_protection = winapi::um::winnt::PAGE_READWRITE;
        let vsize = std::mem::size_of::<hookd3d9::IDirect3D9Vtbl>();

        let old_prot = util::unprotect_memory(vtbl as *mut winapi::ctypes::c_void, vsize)?;

        (*vtbl).CreateDevice = hookd3d9::hook_create_device;

        util::protect_memory(vtbl as *mut winapi::ctypes::c_void, vsize, old_prot)?;

        // create hookstate
        let hd3d9 = hookd3d9::HookDirect3D9 {
            real_create_device: real_create_device
        };
        hookd3d9::set_hook_direct3d9(hd3d9);

        Ok(direct3d9)
    } 
}

#[test]
fn can_create_d3d9() {   
    let d3d9 = create_d3d(32);
    if let Err(x) = d3d9 {
        assert!(false, format!("unable to create d39: {:?}", x));
    }
}