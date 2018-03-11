use winapi::um::libloaderapi::{LoadLibraryW, GetProcAddress};
use winapi::shared::guiddef::{CLSID, REFCLSID, REFIID};
use winapi::um::winnt::{HRESULT};
use winapi::ctypes::c_void;
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winnt::{HANDLE, LPCWSTR, LPWSTR,LUID, VOID};
use winapi::shared::minwindef::{BOOL, BYTE, DWORD, FLOAT, INT, UINT};
use winapi::um::objidlbase::{IEnumString, IEnumUnknown, IStream, IStreamVtbl};

use std;
use util::{write_log_file, load_lib, get_proc_address};
use util::{HookError, Result};

DEFINE_GUID!{CLSID_CLR_META_HOST, 0x9280188d, 0xe8e, 0x4867, 0xb3, 0xc, 0x7f, 0xa8, 0x38, 0x84, 0xe8, 0xde}
DEFINE_GUID!{IID_ICLR_META_HOST, 0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16}
DEFINE_GUID!{IID_ICLR_RUNTIME_INFO, 0xBD39D1D2, 0xBA2F, 0x486a, 0x89, 0xB0, 0xB4, 0xB0, 0xCB, 0x46, 0x68, 0x91}
    
RIDL!(#[uuid(0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16)]
interface ICLRMetaHost(ICLRMetaHostVtbl): IUnknown(IUnknownVtbl) {
        fn GetRuntime(pwzVersion:LPCWSTR, riid:REFIID, ppRuntime:*mut *mut c_void,) -> HRESULT,       
        fn GetVersionFromFile(pwzFilePath: LPCWSTR, pwzBuffer: LPWSTR, pcchBuffer: *mut DWORD,) -> HRESULT,
        fn EnumerateInstalledRuntimes(ppEnumerator: *mut *mut IEnumUnknown,) -> HRESULT,       
        fn EnumerateLoadedRuntimes(hndProcess:HANDLE, ppEnumerator: *mut *mut IEnumUnknown,) -> HRESULT,
        fn RequestRuntimeLoadedNotification(pCallbackFunction:*mut c_void /*RuntimeLoadedCallbackFnPtr*/,) -> HRESULT,        
        fn QueryLegacyV2RuntimeBinding( riid:REFIID, ppUnk: *mut *mut c_void,) -> HRESULT,
        fn ExitProcess(iExitCode:u32,) -> HRESULT, }
);
    
type CLRCreateInstanceFn = unsafe extern "stdcall" 
    fn(clsid:REFCLSID, riid:REFIID, ppInterface: *mut *mut ICLRMetaHost) -> HRESULT;    

pub fn init_clr() -> Result<()> {
    let h = load_lib("mscoree.dll")?;
    let clr_create_instance = get_proc_address(h, "CLRCreateInstance")?;

    unsafe {
        let create:CLRCreateInstanceFn = std::mem::transmute(clr_create_instance);
        let mut metahost: *mut ICLRMetaHost = std::ptr::null_mut();
        let metahost: *mut *mut ICLRMetaHost = &mut metahost;
        let hr = (create)(&CLSID_CLR_META_HOST, &IID_ICLR_META_HOST, metahost);
        println!("create:result: {:?}", hr);
        if hr != 0 {
            return Err(HookError::CLRInitFailed("failed to create meta host".to_owned()));
        }
        if metahost == std::ptr::null_mut() || (*metahost) == std::ptr::null_mut() {
            return Err(HookError::CLRInitFailed("meta host instance is null".to_owned()));
        }
        let metahost = *metahost;

        // skip the enumeration loop and just try creating v4.0 directly
        // TODO: but must enumerate since this specific version likely not found everywhere.
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = OsStr::new("v4.0.30319").encode_wide().chain(once(0)).collect();
        let mut runtime: *mut c_void = std::ptr::null_mut();
        let runtime: *mut *mut c_void = &mut runtime;
        let hr = (*metahost).GetRuntime(wide.as_ptr(), &IID_ICLR_RUNTIME_INFO, runtime);
        if hr != 0 {
            return Err(HookError::CLRInitFailed("failed to create runtime".to_owned()));
        }
        if runtime == std::ptr::null_mut() || (*runtime) == std::ptr::null_mut() {
            return Err(HookError::CLRInitFailed("runtime instance is null".to_owned()));
        }

        // TODO: release things
        write_log_file(format!("clr sortof initialized"));

    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_init_clr() {
        let _r = init_clr()
        .map_err(|err| {
            assert!(false, "Expected Ok but got {:?}", err)
         });
    }
}

