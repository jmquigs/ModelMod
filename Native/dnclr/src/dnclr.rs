#![allow(non_snake_case)]
use types::interop::{ModData, ModSnapProfile};
use winapi::ctypes::c_void;
use winapi::shared::guiddef::{REFCLSID, REFIID};
use winapi::shared::minwindef::{BOOL, DWORD, FALSE, HMODULE, LPVOID, UINT};
use winapi::shared::ntdef::LONG;
use winapi::um::objidlbase::IEnumUnknown;
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winnt::HRESULT;
use winapi::um::winnt::{HANDLE, LPCSTR, LPCWSTR, LPWSTR};

use std::ptr::null_mut;

use shared_dx::error::*;
use shared_dx::util::*;
use util::{get_proc_address, load_lib};

DEFINE_GUID!{CLSID_CLR_META_HOST,
0x9280188d, 0xe8e, 0x4867, 0xb3, 0xc, 0x7f, 0xa8, 0x38, 0x84, 0xe8, 0xde}
DEFINE_GUID!{IID_ICLR_META_HOST,
0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16}
DEFINE_GUID!{IID_ICLR_RUNTIME_INFO,
0xBD39D1D2, 0xBA2F, 0x486a, 0x89, 0xB0, 0xB4, 0xB0, 0xCB, 0x46, 0x68, 0x91}
DEFINE_GUID!{CLSID_CLR_RUNTIME_HOST,
0x90F1A06E, 0x7712, 0x4762, 0x86, 0xB5, 0x7A, 0x5E, 0xBA, 0x6B, 0xDB, 0x02}
DEFINE_GUID!{IID_ICLR_RUNTIME_HOST,
0x90F1A06C, 0x7712, 0x4762, 0x86, 0xB5, 0x7A, 0x5E, 0xBA, 0x6B, 0xDB, 0x02}

RIDL!(#[uuid(0xD332DB9E, 0xB9B3, 0x4125, 0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16)]
interface ICLRMetaHost(ICLRMetaHostVtbl): IUnknown(IUnknownVtbl) {
    fn GetRuntime(pwzVersion:LPCWSTR, riid:REFIID, ppRuntime:*mut *mut ICLRRuntimeInfo,) -> HRESULT,
    fn GetVersionFromFile(pwzFilePath: LPCWSTR, pwzBuffer: LPWSTR, pcchBuffer: *mut DWORD,)
        -> HRESULT,
    fn EnumerateInstalledRuntimes(ppEnumerator: *mut *mut IEnumUnknown,) -> HRESULT,
    fn EnumerateLoadedRuntimes(hndProcess:HANDLE, ppEnumerator: *mut *mut IEnumUnknown,)
        -> HRESULT,
    fn RequestRuntimeLoadedNotification(pCallbackFunction:*mut c_void
        /*RuntimeLoadedCallbackFnPtr*/,) -> HRESULT,
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
    fn GetDefaultStartupFlags(pdwStartupFlags:*mut DWORD, pwzHostConfigFile:LPWSTR,
        pcchHostConfigFile:*mut DWORD,) -> HRESULT,
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
    fn ExecuteApplication(pwzAppFullName: LPCWSTR, dwManifestPaths:DWORD,
        ppwzManifestPaths: *mut LPCWSTR, dwActivationData: DWORD, ppwzActivationData: *mut LPCWSTR,
        pReturnValue: *mut i32,) -> HRESULT,
    fn ExecuteInDefaultAppDomain(pwzAssemblyPath:LPCWSTR, pwzTypeName:LPCWSTR,
        pwzMethodName:LPCWSTR, pwzArgument:LPCWSTR, pReturnValue: *mut DWORD,) -> HRESULT,
});

type CLRCreateInstanceFn =
    unsafe extern "stdcall" fn(clsid: REFCLSID, riid: REFIID, ppInterface: *mut *mut ICLRMetaHost)
        -> HRESULT;

struct CLRGlobalState {
    runtime_host: *mut ICLRRuntimeHost
}

const NATIVE_CODE_VERSION:i32 = 4;

static mut CLR_GLOBAL_STATE: CLRGlobalState = CLRGlobalState {
    runtime_host: null_mut()
};

/// Use when running outside of any known d3d renderer (i.e in tests).
pub const RUN_CONTEXT_MMNATIVE:&str = "mm_native";
/// Use when running in a d3d9 renderer context.
pub const RUN_CONTEXT_D3D9:&str = "d3d9";
/// Use when running in a d3d11 renderer context.
pub const RUN_CONTEXT_D3D11:&str = "d3d11";

// Defaults in case context is not specified.  Note, d3d9 or 11 could be automatically detected,
// based on the filename of the dll or which was initialized, but this is not currently
// implemented.
#[cfg(test)]
pub fn get_run_context() -> &'static str {
    RUN_CONTEXT_MMNATIVE
}
#[cfg(not(test))]
pub fn get_run_context() -> &'static str {
    RUN_CONTEXT_D3D9
}

pub fn init_clr(mm_root: &Option<String>) -> Result<()> {
    let mm_root = mm_root
        .as_ref()
        .ok_or(HookError::UnableToLocatedManagedDLL(
            "No MM Root has been set".to_owned(),
        ))?;
    // fail early if this is missing
    let _managed_dll = util::get_managed_dll_path(mm_root)?;

    let h = load_lib("mscoree.dll")?;
    let clr_create_instance = get_proc_address(h, "CLRCreateInstance")?;

    unsafe {
        let metahost: *mut ICLRMetaHost = {
            let create: CLRCreateInstanceFn = std::mem::transmute(clr_create_instance);
            let mut metahost: *mut ICLRMetaHost = null_mut();
            let hr = (create)(&CLSID_CLR_META_HOST, &IID_ICLR_META_HOST, &mut metahost);
            if hr != 0 {
                return Err(HookError::CLRInitFailed(
                    "failed to create meta host".to_owned(),
                ));
            }
            if metahost == null_mut() {
                return Err(HookError::CLRInitFailed(
                    "meta host instance is null".to_owned(),
                ));
            }
            metahost
        };

        // skip the enumeration loop and just try creating v4.0 directly
        // TODO: but must enumerate since this specific version likely not found everywhere.
        let runtime_info = {
            let wide = util::to_wide_str("v4.0.30319");
            let mut p_runtime: *mut ICLRRuntimeInfo = null_mut();
            let hr = (*metahost).GetRuntime(wide.as_ptr(), &IID_ICLR_RUNTIME_INFO, &mut p_runtime);
            if hr != 0 {
                return Err(HookError::CLRInitFailed(
                    "failed to create runtime".to_owned(),
                ));
            }
            if p_runtime == null_mut() {
                return Err(HookError::CLRInitFailed(
                    "runtime instance is null".to_owned(),
                ));
            }
            p_runtime
        };

        let mut loadable: BOOL = FALSE;
        let hr = (*runtime_info).IsLoadable(&mut loadable);
        if hr != 0 {
            return Err(HookError::CLRInitFailed(
                "failed to check loadability".to_owned(),
            ));
        }
        if loadable == FALSE {
            return Err(HookError::CLRInitFailed(
                "runtime is not loadable".to_owned(),
            ));
        }

        let runtime_host: *mut ICLRRuntimeHost = {
            let mut p_rhost: *mut c_void = null_mut();
            let hr = (*runtime_info).GetInterface(
                &CLSID_CLR_RUNTIME_HOST,
                &IID_ICLR_RUNTIME_HOST,
                &mut p_rhost,
            );

            if hr != 0 {
                return Err(HookError::CLRInitFailed(
                    "failed to query runtime host".to_owned(),
                ));
            }
            if p_rhost == null_mut() {
                return Err(HookError::CLRInitFailed(
                    "runtime host instance is null".to_owned(),
                ));
            }
            std::mem::transmute(p_rhost)
        };

        // TODO: maybe use custom host control to support reloading

        let hr = (*runtime_host).Start();
        if hr != 0 {
            return Err(HookError::CLRInitFailed(format!(
                "failed to start clr, HRESULT: {}",
                hr
            )));
        }

        CLR_GLOBAL_STATE.runtime_host = runtime_host;
        Ok(())
    }
}

pub fn reload_managed_dll(mm_root: &Option<String>, run_context:Option<&'static str>) -> Result<()> {
    if unsafe { CLR_GLOBAL_STATE.runtime_host } == null_mut() {
        return Err(HookError::CLRInitFailed("runtime host pointer is null".to_owned()))?
    }
    let mm_root = mm_root
        .as_ref()
        .ok_or(HookError::UnableToLocatedManagedDLL(
            "No MM Root has been set".to_owned(),
        ))?;
    let managed_dll = util::get_managed_dll_path(mm_root)?;

    let run_context = run_context.unwrap_or_else(|| get_run_context());

    // copy the managed_dll to a temp name prior
    let attempts = 0..255;
    let pb = std::path::Path::new(&managed_dll);
    let pb = pb.parent().ok_or(HookError::CLRInitFailed("managed dll has no parent".to_owned()))?;

    let mut dll_copy:Option<String> = None;
    let mut _reload_idx = 0;
    for idx in attempts {
        if dll_copy.is_some() {
            break;
        }
        let mut pb = pb.to_path_buf();
        let name = format!("TempMMManaged{:03}.dll", idx);
        pb.push(name);
        let res = std::fs::copy(&managed_dll, &pb);
        if res.is_ok() {
            dll_copy = Some(pb.to_str().ok_or(
                HookError::CLRInitFailed("copied dll path error".to_owned()))?.to_owned());
            _reload_idx = idx;
            break;
        }
    }

    let managed_dll = dll_copy.ok_or(
        HookError::CLRInitFailed("copied dll path error 2".to_owned()))?;

        let app = util::to_wide_str(&managed_dll);
        let typename = util::to_wide_str("ModelMod.Main");
        let method = util::to_wide_str("Main");

        write_log_file(&format!("Loading managed dll {} into CLR", managed_dll));
        
        write_log_file(&format!(
            "using '{}' load context for CLR",
            run_context
        ));

        // can only pass one argument (a string), so delimit the arguments with pipe
        // note: intentially defeating the purpose of GSPointerRef here since we need to 
        // pass the pointer to managed code so that it can pass it back to us in 
        // OnInitialized
        let global_state_ptr = global_state::get_global_state_ptr();
        let ptr = global_state_ptr.gsp as usize;
        drop(global_state_ptr); // to avoid the gs tracking code logging when the managed code calls OnInitialized
        let argument = util::to_wide_str(&format!(
            "{}|{}|{}|mod_structsize={}|mod_snapprofile_structsize={}",
            ptr,
            run_context,
            NATIVE_CODE_VERSION,
            size_of::<ModData>(),
            size_of::<ModSnapProfile>(),
        ));
    unsafe {
        let mut ret: u32 = 0xFFFFFFFF;
        let hr = (*CLR_GLOBAL_STATE.runtime_host).ExecuteInDefaultAppDomain(
            app.as_ptr(),
            typename.as_ptr(),
            method.as_ptr(),
            argument.as_ptr(),
            &mut ret,
        );
        if hr != 0 {
            return Err(HookError::CLRInitFailed(format!(
                "failed to start clr, HRESULT: {:x}",
                hr
            )));
        }
        if ret != 0 {
            let msg =
                match ret {
                    48 => "Error: Managed code version mismatch.  Ensure that the d3d9.dll loaded by the game is the latest version.  You may need to copy the new version in from your ModelMod directory.  Click 'Start' in the ModelMod Launcher for more details.".to_string(),
                    _ => format!("Error: Managed code failed to initialize; return code: {}", ret)
                };
            return Err(HookError::CLRInitFailed(msg));
        }
    }

    // TODO: release things?
    write_log_file("clr initialized");

    Ok(())
}

// unsafe fn get_module_name() {
//     use winapi::um::libloaderapi::*;
//     use std::ffi::OsString;
//     use std::os::windows::prelude::*;

//     let ssize = 65535;
//     let mut mpath:Vec<u16> = Vec::with_capacity(ssize);

//     let handle = GetModuleHandleW(std::ptr::null_mut());
//     let r = GetModuleFileNameW(handle, mpath.as_mut_ptr(), ssize as DWORD);
//     if r == 0 {
//         println!("failed to get module file name");
//     } else {
//         let s = std::slice::from_raw_parts(mpath.as_mut_ptr(), r as usize);
//         let string = OsString::from_wide(&s);
//         println!("the handle is {:?}", &string);

//     }
// }

#[cfg(test)]
mod tests {
    //use super::*;

    #[test]
    pub fn test_init_clr() {
        //unsafe { get_module_name() };
        // TODO: fix this to use a generic test assembly
        // init_clr(&Some("C:\\Dev\\modelmod.new".to_owned()))
        // .map_err(|err| {
        //     assert!(false, "Expected Ok but got {:?}", err)
        // });
        // .map(|r| {
        //     hook_render::hook_begin_scene()

        // });
    }
}
