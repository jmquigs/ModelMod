use winapi::shared::guiddef::{REFCLSID, REFIID};
use winapi::um::winnt::{HRESULT};
use winapi::ctypes::c_void;
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winnt::{HANDLE, LPCWSTR, LPWSTR, LPCSTR};
use winapi::shared::minwindef::{BOOL, FALSE, DWORD, UINT, HMODULE, LPVOID};
use winapi::shared::ntdef::LONG;
use winapi::um::objidlbase::{IEnumUnknown};

use std;
use std::ptr::null_mut;
use util::{write_log_file, load_lib, get_proc_address};
use util::{HookError, Result};
use util;
use hookd3d9;

DEFINE_GUID!{CLSID_CLR_META_HOST, 0x9280188d, 0xe8e, 0x4867, 0xb3, 0xc, 0x7f, 0xa8, 0x38, 0x84, 0xe8, 0xde}
DEFINE_GUID!{IID_ICLR_META_HOST, 0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16}
DEFINE_GUID!{IID_ICLR_RUNTIME_INFO, 0xBD39D1D2, 0xBA2F, 0x486a, 0x89, 0xB0, 0xB4, 0xB0, 0xCB, 0x46, 0x68, 0x91}

DEFINE_GUID!{CLSID_CLR_RUNTIME_HOST, 0x90F1A06E, 0x7712, 0x4762, 0x86, 0xB5, 0x7A, 0x5E, 0xBA, 0x6B, 0xDB, 0x02}
DEFINE_GUID!{IID_ICLR_RUNTIME_HOST, 0x90F1A06C, 0x7712, 0x4762, 0x86, 0xB5, 0x7A, 0x5E, 0xBA, 0x6B, 0xDB, 0x02}

RIDL!(#[uuid(0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16)]
interface ICLRMetaHost(ICLRMetaHostVtbl): IUnknown(IUnknownVtbl) {
    fn GetRuntime(pwzVersion:LPCWSTR, riid:REFIID, ppRuntime:*mut *mut c_void,) -> HRESULT,       
    fn GetVersionFromFile(pwzFilePath: LPCWSTR, pwzBuffer: LPWSTR, pcchBuffer: *mut DWORD,) -> HRESULT,
    fn EnumerateInstalledRuntimes(ppEnumerator: *mut *mut IEnumUnknown,) -> HRESULT,       
    fn EnumerateLoadedRuntimes(hndProcess:HANDLE, ppEnumerator: *mut *mut IEnumUnknown,) -> HRESULT,
    fn RequestRuntimeLoadedNotification(pCallbackFunction:*mut c_void /*RuntimeLoadedCallbackFnPtr*/,) -> HRESULT,        
    fn QueryLegacyV2RuntimeBinding( riid:REFIID, ppUnk: *mut *mut c_void,) -> HRESULT,
    fn ExitProcess(iExitCode:u32,) -> HRESULT, 
});

RIDL!(#[uuid(0xBD39D1D2, 0xBA2F, 0x486a, 0x89, 0xB0, 0xB4, 0xB0, 0xCB, 0x46, 0x68, 0x91)]
interface ICLRRuntimeInfo(ICLRRuntimeInfoVtbl): IUnknown(IUnknownVtbl) {
    fn GetVersionString(pwzBuffer: LPWSTR, pcchBuffer: *mut DWORD,) -> HRESULT,
    fn GetRuntimeDirectory( pwzBuffer: LPWSTR, pcchBuffer: *mut DWORD,) -> HRESULT,        
    fn IsLoaded(hndProcess:HANDLE, pbLoaded: *mut BOOL,) -> HRESULT,        
    fn LoadErrorString(iResourceID:UINT, pwzBuffer:LPWSTR, pcchBuffer: *mut DWORD, iLocaleID: LONG,)
        -> HRESULT,
    fn LoadLibrary(pwzDllName:LPCWSTR, phndModule:*mut HMODULE,) -> HRESULT,
    fn GetProcAddress(pszProcName: LPCSTR, ppProc: *mut LPVOID,) -> HRESULT,
    fn GetInterface(rclsid:REFCLSID, riid:REFIID, ppUnk:*mut LPVOID,) -> HRESULT,       
    fn IsLoadable(pbLoadable: *mut BOOL,) -> HRESULT,        
    fn SetDefaultStartupFlags(dwStartupFlags: DWORD, pwzHostConfigFile: LPCWSTR,) -> HRESULT,
    fn GetDefaultStartupFlags(pdwStartupFlags:*mut DWORD, pwzHostConfigFile:LPWSTR, pcchHostConfigFile:*mut DWORD,)
        -> HRESULT,
    fn BindAsLegacyV2Runtime() -> HRESULT,        
    fn IsStarted(pbStarted:*mut BOOL, pdwStartupFlags: *mut DWORD,) -> HRESULT,
}); 

RIDL!(#[uuid(0x90F1A06C, 0x7712, 0x4762, 0x86, 0xB5, 0x7A, 0x5E, 0xBA, 0x6B, 0xDB, 0x02)]
interface ICLRRuntimeHost(ICLRRuntimeHostVtbl): IUnknown(IUnknownVtbl) {
    fn Start() -> HRESULT,
    fn Stop() -> HRESULT,
    fn SetHostControl(pHostControl: *mut c_void /*IHostControl*/,) -> HRESULT,
    fn GetHostControl(pHostControl: *mut *mut c_void /*IHostControl*/,) -> HRESULT,              
    fn UnloadAppDomain(dwAppDomainId:DWORD, fWaitUntilDone: BOOL,) -> HRESULT,        
    fn ExecuteInAppDomain(dwAppDomainId:DWORD, 
        pCallback: *mut c_void /*FExecuteInAppDomainCallback*/, cookie: LPVOID,) -> HRESULT,
    fn GetCurrentAppDomainId(pdwAppDomainId: *mut DWORD,) -> HRESULT, 
    fn ExecuteApplication(pwzAppFullName: LPCWSTR, dwManifestPaths:DWORD, ppwzManifestPaths: *mut LPCWSTR,
        dwActivationData: DWORD, ppwzActivationData: *mut LPCWSTR, pReturnValue: *mut i32,) -> HRESULT,        
    fn ExecuteInDefaultAppDomain(pwzAssemblyPath:LPCWSTR, pwzTypeName:LPCWSTR, pwzMethodName:LPCWSTR,
        pwzArgument:LPCWSTR, pReturnValue: *mut DWORD,) -> HRESULT,
});
    
type CLRCreateInstanceFn = unsafe extern "stdcall" 
    fn(clsid:REFCLSID, riid:REFIID, ppInterface: *mut *mut ICLRMetaHost) -> HRESULT;    

pub fn init_clr() -> Result<()> {
    let h = load_lib("mscoree.dll")?;
    let clr_create_instance = get_proc_address(h, "CLRCreateInstance")?;

    unsafe {
        let metahost:*mut ICLRMetaHost = {
            let create:CLRCreateInstanceFn = std::mem::transmute(clr_create_instance);
            let mut metahost: *mut ICLRMetaHost = null_mut();
            let metahost: *mut *mut ICLRMetaHost = &mut metahost;
            let hr = (create)(&CLSID_CLR_META_HOST, &IID_ICLR_META_HOST, metahost);
            if hr != 0 {
                return Err(HookError::CLRInitFailed("failed to create meta host".to_owned()));
            }
            if metahost == null_mut() || (*metahost) == null_mut() {
                return Err(HookError::CLRInitFailed("meta host instance is null".to_owned()));
            }
            *metahost
        };

        // skip the enumeration loop and just try creating v4.0 directly
        // TODO: but must enumerate since this specific version likely not found everywhere.
        let runtime_info:*mut ICLRRuntimeInfo = {
            let wide = util::to_wide_str("v4.0.30319");
            let mut p_runtime:*mut c_void = null_mut();
            let pp_runtime: *mut *mut c_void = &mut p_runtime;
            let hr = (*metahost).GetRuntime(wide.as_ptr(), &IID_ICLR_RUNTIME_INFO, pp_runtime);
            if hr != 0 {
                return Err(HookError::CLRInitFailed("failed to create runtime".to_owned()));
            }
            if pp_runtime == null_mut() || (*pp_runtime) == null_mut() {
                return Err(HookError::CLRInitFailed("runtime instance is null".to_owned()));
            }            
            std::mem::transmute(*pp_runtime)
        };

        let mut loadable:BOOL = FALSE;
        let hr = (*runtime_info).IsLoadable(&mut loadable);
        if hr != 0 {
            return Err(HookError::CLRInitFailed("failed to check loadability".to_owned()));
        }
        if loadable == FALSE {
            return Err(HookError::CLRInitFailed("runtime is not loadable".to_owned()));
        }

        let runtime_host:*mut ICLRRuntimeHost = {
            let mut p_rhost:*mut c_void = null_mut();
            let pp_rhost: *mut *mut c_void = &mut p_rhost;
            let hr = (*runtime_info).GetInterface(&CLSID_CLR_RUNTIME_HOST, &IID_ICLR_RUNTIME_HOST, pp_rhost);

            if hr != 0 {
                return Err(HookError::CLRInitFailed("failed to query runtime host".to_owned()));
            }
            if pp_rhost == null_mut() || (*pp_rhost) == null_mut() {
                return Err(HookError::CLRInitFailed("runtime host instance is null".to_owned()));
            }
            std::mem::transmute(*pp_rhost)
        };

        // TODO: maybe use custom host control to support reloading

        let hr = (*runtime_host).Start();
        if hr != 0 {
            return Err(HookError::CLRInitFailed(format!("failed to start clr, HRESULT: {}", hr)));
        }        

        let path = "D:\\Dev\\ModelMod\\Release\\MMManaged.dll"; // TODO: unhardcode
        let app = util::to_wide_str(path);

        let typename = util::to_wide_str("ModelMod.Main");
        let method = util::to_wide_str("Main");

        let cookie = hookd3d9::get_global_state_ptr();
        let argument = util::to_wide_str(&format!("{}", cookie as u64));
        let mut ret:u32 = 0xFFFFFFFF;
        let hr = (*runtime_host).ExecuteInDefaultAppDomain(app.as_ptr(),
            typename.as_ptr(),
            method.as_ptr(),
            argument.as_ptr(),
            &mut ret);
        if hr != 0 {
            return Err(HookError::CLRInitFailed(format!("failed to start clr, HRESULT: {:x}", hr)));
        }

        // TODO: release things?
        write_log_file(&format!("clr sortof initialized"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_init_clr() {
        // TODO: fix this to use a generic test assembly
        // init_clr()
        // .map_err(|err| {
        //     assert!(false, "Expected Ok but got {:?}", err)
        // })
        // .map(|r| {
        //     hookd3d9::hook_begin_scene()

        // });
    }
}

