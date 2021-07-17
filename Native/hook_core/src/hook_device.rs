use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use std;
use std::ptr::null_mut;
use shared_dx9::types::*;
use shared_dx9::util::*;
use shared_dx9::error::*;
use input;
use util;
use util::*;
use global_state::{GLOBAL_STATE, GLOBAL_STATE_LOCK};

use device_state::DEVICE_STATE;
use crate::hook_render::{hook_present, hook_draw_indexed_primitive, hook_release};

/*
Would be nice to move this into a separate crate, but it needs to know about the device functions
that we want to hook and override.  So its probably stuck here.
*/

unsafe fn hook_device(
    device: *mut IDirect3DDevice9,
    _guard: &std::sync::MutexGuard<()>,
) -> Result<HookDirect3D9Device> {
    //write_log_file(&format!("gs hook_direct3d9device is some: {}", GLOBAL_STATE.hook_direct3d9device.is_some()));
    write_log_file(&format!("hooking new device: {:x}", device as u64));
    // Oddity: each device seems to have its own vtbl.  So need to hook each one of them.
    // but the direct3d9 instance seems to share a vtbl between different instances.  So need to only
    // hook those once.  I'm not sure why this is.
    let vtbl: *mut IDirect3DDevice9Vtbl = std::mem::transmute((*device).lpVtbl);
    write_log_file(&format!("device vtbl: {:x}", vtbl as u64));
    let vsize = std::mem::size_of::<IDirect3DDevice9Vtbl>();

    let real_draw_indexed_primitive = (*vtbl).DrawIndexedPrimitive;
    // check for already hook devices (useful in late-hook case)
    if real_draw_indexed_primitive as u64 == hook_draw_indexed_primitive as u64 {
        write_log_file(&format!("error: device already appears to be hooked, skipping"));
        return Err(HookError::D3D9DeviceHookFailed);
    }
    //let real_begin_scene = (*vtbl).BeginScene;
    let real_release = (*vtbl).parent.Release;
    let real_present = (*vtbl).Present;

    // remember these functions but don't hook them yet
    let real_set_texture = (*vtbl).SetTexture;

    let real_set_vertex_sc_f = (*vtbl).SetVertexShaderConstantF;
    let real_set_vertex_sc_i = (*vtbl).SetVertexShaderConstantI;
    let real_set_vertex_sc_b = (*vtbl).SetVertexShaderConstantB;

    let real_set_pixel_sc_f = (*vtbl).SetPixelShaderConstantF;
    let real_set_pixel_sc_i = (*vtbl).SetPixelShaderConstantI;
    let real_set_pixel_sc_b = (*vtbl).SetPixelShaderConstantB;

    let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

    // This was used to debug an issue with reshade where something
    // was unhooking the pointers after I hooked it.  possibly securom in
    // mass effect 2.
    // write_log_file(&format!("DrawIndexedPrimitive real: {:x}, hook: {:x}",
    //     real_draw_indexed_primitive as u64,
    //     hook_draw_indexed_primitive as u64,
    // ));
    // write_log_file(&format!("Present real: {:x}, hook: {:x}",
    //     real_present as u64,
    //     hook_present as u64,
    // ));
    (*vtbl).DrawIndexedPrimitive = hook_draw_indexed_primitive;
    //(*vtbl).BeginScene = hook_begin_scene;
    (*vtbl).Present = hook_present;
    (*vtbl).parent.Release = hook_release;

    protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

    // Inc ref count on the device
    (*device).AddRef();

    // shader constants init
    if constant_tracking::is_enabled() {
        GLOBAL_STATE.vertex_constants = Some(constant_tracking::ConstantGroup::new());
        GLOBAL_STATE.pixel_constants = Some(constant_tracking::ConstantGroup::new());

        // (*vtbl).SetVertexShaderConstantF = dev_constant_tracking::hook_set_vertex_sc_f;
        // (*vtbl).SetVertexShaderConstantI = dev_constant_tracking::hook_set_vertex_sc_i;
        // (*vtbl).SetVertexShaderConstantB = dev_constant_tracking::hook_set_vertex_sc_b;

        // (*vtbl).SetPixelShaderConstantF = dev_constant_tracking::hook_set_pixel_sc_f;
        // (*vtbl).SetPixelShaderConstantI = dev_constant_tracking::hook_set_pixel_sc_i;
        // (*vtbl).SetPixelShaderConstantB = dev_constant_tracking::hook_set_pixel_sc_b;
    }
    write_log_file(&format!("constant tracking enabled: {}", constant_tracking::is_enabled()));

    Ok(HookDirect3D9Device::new(
        real_draw_indexed_primitive,
        //real_begin_scene,
        real_present,
        real_release,
        real_set_texture,
        real_set_vertex_sc_f,
        real_set_vertex_sc_i,
        real_set_vertex_sc_b,
        real_set_pixel_sc_f,
        real_set_pixel_sc_i,
        real_set_pixel_sc_b,
    ))
}

#[inline]
unsafe fn create_and_hook_device(
    THIS: *mut IDirect3D9,
    Adapter: UINT,
    DeviceType: D3DDEVTYPE,
    hFocusWindow: HWND,
    BehaviorFlags: DWORD,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
    ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
) -> Result<()> {
    let lock = GLOBAL_STATE_LOCK
        .lock()
        .map_err(|_err| HookError::GlobalLockError)?;

    if DEVICE_STATE == null_mut() {
        return Err(HookError::BadStateError("no device state pointer??".to_owned()));
    }
    (*DEVICE_STATE)
        .hook_direct3d9
        .as_mut()
        .ok_or(HookError::Direct3D9InstanceNotFound)
        .and_then(|hd3d9| {
            write_log_file(&format!("calling real create device"));
            if BehaviorFlags & D3DCREATE_MULTITHREADED == D3DCREATE_MULTITHREADED {
                write_log_file(&format!(
                    "Notice: device being created with D3DCREATE_MULTITHREADED"
                ));
            }
            let result = (hd3d9.real_create_device)(
                THIS,
                Adapter,
                DeviceType,
                hFocusWindow,
                BehaviorFlags,
                pPresentationParameters,
                ppReturnedDeviceInterface,
            );
            if result != S_OK {
                write_log_file(&format!("create device FAILED: {}", result));
                return Err(HookError::CreateDeviceFailed(result));
            }
            (*DEVICE_STATE).d3d_window = hFocusWindow;
            hook_device(*ppReturnedDeviceInterface, &lock)
        })
        .and_then(|hook_d3d9device| {
            (*DEVICE_STATE).hook_direct3d9device = Some(hook_d3d9device);
            write_log_file(&format!(
                "hooked device on thread {:?}",
                std::thread::current().id()
            ));
            Ok(())
        })
        .or_else(|err| {
            if ppReturnedDeviceInterface != null_mut() && *ppReturnedDeviceInterface != null_mut() {
                (*(*ppReturnedDeviceInterface)).Release();
            }
            Err(err)
        })
}

pub unsafe extern "system" fn hook_create_device(
    THIS: *mut IDirect3D9,
    Adapter: UINT,
    DeviceType: D3DDEVTYPE,
    hFocusWindow: HWND,
    BehaviorFlags: DWORD,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
    ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
) -> HRESULT {
    let res = create_and_hook_device(
        THIS,
        Adapter,
        DeviceType,
        hFocusWindow,
        BehaviorFlags,
        pPresentationParameters,
        ppReturnedDeviceInterface,
    );

    // TODO: need to do this on late-hook path, not here
    // create input, but don't fail everything if we can't (may be able to still use read-only mode)
    input::Input::new()
        .map(|inp| {
            GLOBAL_STATE.input = Some(inp);
        })
        .unwrap_or_else(|e| {
            write_log_file(&format!(
                "failed to create input; only playback from existing mods will be possible: {:?}",
                e
            ))
        });

    match res {
        Err(e) => {
            write_log_file(&format!("error creating/hooking device: {:?}", e));
            E_FAIL
        }
        Ok(_) => S_OK,
    }
}

// perf event typedefs from:
// https://github.com/Microsoft/DXUT/blob/942a9f4e30abf6d5d0c1b3529c17cd6b574743f9/Core/DXUTmisc.cpp
#[allow(unused)]
#[no_mangle]
// typedef INT         (WINAPI * LPD3DPERF_BEGINEVENT)(DWORD, LPCWSTR);
pub extern "system" fn D3DPERF_BeginEvent(a: DWORD, b: LPCWSTR) -> i32 {
    0
}
#[allow(unused)]
#[no_mangle]
// typedef INT         (WINAPI * LPD3DPERF_ENDEVENT)(void);
pub extern "system" fn D3DPERF_EndEvent() -> i32 {
    0
}
#[allow(unused)]
#[no_mangle]
// typedef VOID        (WINAPI * LPD3DPERF_SETMARKER)(DWORD, LPCWSTR);
pub extern "system" fn D3DPERF_SetMarker(a: DWORD, b: LPCWSTR) -> () {}
#[allow(unused)]
#[no_mangle]
// typedef VOID        (WINAPI * LPD3DPERF_SETREGION)(DWORD, LPCWSTR);
pub extern "system" fn D3DPERF_SetRegion(a: DWORD, b: LPCWSTR) -> () {}
#[allow(unused)]
#[no_mangle]
// typedef BOOL        (WINAPI * LPD3DPERF_QUERYREPEATFRAME)(void);
pub extern "system" fn D3DPERF_QueryRepeatFrame() -> BOOL {
    FALSE
}
#[allow(unused)]
#[no_mangle]
// typedef VOID        (WINAPI * LPD3DPERF_SETOPTIONS)( DWORD dwOptions );
pub extern "system" fn D3DPERF_SetOptions(ops: DWORD) -> () {}
#[allow(unused)]
#[no_mangle]
// typedef DWORD (WINAPI * LPD3DPERF_GETSTATUS)();
pub extern "system" fn D3DPERF_GetStatus() -> DWORD {
    0
}

type Direct3DCreate9Fn = unsafe extern "system" fn(sdk_ver: u32) -> *mut IDirect3D9;

#[allow(unused)]
#[no_mangle]
pub extern "system" fn Direct3DCreate9(SDKVersion: u32) -> *mut u64 {
    match create_d3d9(SDKVersion) {
        Ok(ptr) => ptr as *mut u64,
        Err(x) => {
            write_log_file(&format!("create_d3d failed: {:?}", x));
            std::ptr::null_mut()
        }
    }
}

fn init_device_state_once() {
    unsafe {
        if DEVICE_STATE == null_mut() {
            DEVICE_STATE = Box::into_raw(Box::new(DeviceState {
                hook_direct3d9: None,
                hook_direct3d9device: None,
                d3d_window: null_mut(),
                d3d_resource_count: 0,
            }));
        }
    };
}

fn mm_verify_load() -> Option<String> {
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

fn init_log(mm_root:&str) {
    // try to create log file using module name and root dir.  if it fails then just
    // let logging go to the temp dir file.
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

            let file_name = format!("ModelMod.{}.log", stem);

            let mut tdir = mm_root.to_owned();
            tdir.push_str("\\Logs\\");
            let mut tname = tdir.to_owned();
            tname.push_str(&file_name);

            use std::fs::OpenOptions;
            use std::io::Write;
            // controls whether log file is cleared on each run
            let clear_log_file = true;
            let mut f = OpenOptions::new()
                .create(clear_log_file)
                .write(true)
                .truncate(clear_log_file)
                .open(&tname)?;
            writeln!(f, "ModelMod initialized\r")?;

            // if that succeeded then we can set the file name now
            set_log_file_path(&tdir, &file_name)?;

            eprintln!("Log File: {}", tname);

            Ok(())
        })
        .map_err(|e| {
            write_log_file(&format!("error setting custom log file name: {:?}", e));
        })
        .unwrap_or(());
}

#[allow(unused)]
#[no_mangle]
/// Experimental api for hooking a device that was created externally,
/// for example, inside reshade.  This is incomplete, and requires a
/// version of reshade that supports addons as well as an addon specific
/// to modelmod to load it (see ReshadeAddon in the root of this volume)
pub fn late_hook_device(deviceptr: u64) -> i32 {
    init_device_state_once();
    let mm_root = match mm_verify_load() {
        Some(dir) => dir,
        None => {
            return 1;
        }
    };
    init_log(&mm_root);
    unsafe {
        GLOBAL_STATE.mm_root = Some(mm_root);
    }

    if deviceptr == 0 {
        return 2;
    }

    unsafe {
        #[cfg(target_arch = "x86")]
        let praw:u32 = deviceptr as u32;
        #[cfg(target_arch = "x86_64")]
        let praw:u64 = deviceptr;

        let device:LPDIRECT3DDEVICE9 = std::mem::transmute(praw);

        let hookit = || -> Result<()> {
            let lock = GLOBAL_STATE_LOCK
            .lock()
            .map_err(|_err| HookError::GlobalLockError)?;

            // TODO should not hook more than once! (need to remember it somehow, compare fn
            // pointers in the vtable?)
            let hook_d3d9device = hook_device(device, &lock)?;

            //(*DEVICE_STATE).d3d_window = hFocusWindow; // TODO: need to get this in late hook API
            (*DEVICE_STATE).hook_direct3d9device = Some(hook_d3d9device);
            write_log_file(&format!(
                "hooked device on thread {:?}",
                std::thread::current().id()
            ));

            Ok(())
        };

        hookit();
    }

    0
}

pub fn create_d3d9(sdk_ver: u32) -> Result<*mut IDirect3D9> {
    init_device_state_once();

    let handle = util::load_lib("c:\\windows\\system32\\d3d9.dll")?; // Todo: use GetSystemDirectory
    let addr = util::get_proc_address(handle, "Direct3DCreate9")?;

    let make_it = || unsafe {
        let create: Direct3DCreate9Fn = std::mem::transmute(addr);

        let direct3d9 = (create)(sdk_ver);
        let direct3d9 = direct3d9 as *mut IDirect3D9;
        direct3d9
    };

    unsafe {
        let mm_root = match mm_verify_load() {
            Some(dir) => dir,
            None => {
                return Ok(make_it())
            }
        };

        init_log(&mm_root);

        let direct3d9 = make_it();
        write_log_file(&format!("created d3d: {:x}", direct3d9 as u64));

        // let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);
        // write_log_file(&format!("vtbl: {:x}", vtbl as u64));

        // don't hook more than once
        let _lock = GLOBAL_STATE_LOCK
            .lock()
            .map_err(|_err| HookError::D3D9HookFailed)?;

        if (*DEVICE_STATE).hook_direct3d9.is_some() {
            return Ok(direct3d9);
        }

        GLOBAL_STATE.mm_root = Some(mm_root);

        // get pointer to original vtable
        let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);

        // save pointer to real function
        let real_create_device = (*vtbl).CreateDevice;
        // write_log_file(&format!(
        //     "hooking real create device, hookfn: {:?}, realfn: {:?} ",
        //     hook_create_device as u64, real_create_device as u64
        // ));

        // unprotect memory and slam the vtable
        let vsize = std::mem::size_of::<IDirect3D9Vtbl>();
        let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;

        (*vtbl).CreateDevice = hook_create_device;

        util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

        // create hookstate
        let hd3d9 = HookDirect3D9 {
            real_create_device: real_create_device,
        };

        (*DEVICE_STATE).hook_direct3d9 = Some(hd3d9);

        Ok(direct3d9)
    }
}
