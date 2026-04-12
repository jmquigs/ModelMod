use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::HWND;
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use std;
use std::ptr::null_mut;
use std::time::Instant;
use shared_dx::types::*;
use shared_dx::types_dx9::*;
use shared_dx::util::*;
use shared_dx::error::*;
use input;
use util;
use util::*;
use global_state::{GLOBAL_STATE, GLOBAL_STATE_LOCK};

use device_state::{DEVICE_STATE, dev_state};
use crate::hook_render::{hook_present, hook_draw_indexed_primitive, hook_release};
use crate::hook_render_d3d11::HOOK_DRAW_PERIODIC_CALLS;
use crate::hook_device_d3d11::query_and_set_runconf_in_globalstate;

/*
Would be nice to move this into a separate crate, but it needs to know about the device functions
that we want to hook and override.  So its probably stuck here.
*/

unsafe fn hook_d3d9_device(
    device: *mut IDirect3DDevice9,
    _guard: &std::sync::MutexGuard<()>,
) -> Result<HookDirect3D9Device> {
    //write_log_file(&format!("gs hook_direct3d9device is some: {}", GLOBAL_STATE.hook_direct3d9device.is_some()));
    write_log_file(&format!("hooking new device: {:x}", device as usize));
    // Oddity: each device seems to have its own vtbl.  So need to hook each one of them.
    // but the direct3d9 instance seems to share a vtbl between different instances.  So need to only
    // hook those once.  I'm not sure why this is.
    let vtbl: *mut IDirect3DDevice9Vtbl = std::mem::transmute((*device).lpVtbl);
    write_log_file(&format!("device vtbl: {:x}", vtbl as usize));
    let vsize = std::mem::size_of::<IDirect3DDevice9Vtbl>();

    let real_draw_indexed_primitive = (*vtbl).DrawIndexedPrimitive;
    // check for already hook devices (useful in late-hook case)
    if real_draw_indexed_primitive as usize == hook_draw_indexed_primitive as usize {
        write_log_file(&format!("error: device already appears to be hooked, skipping"));
        return Err(HookError::D3D9DeviceHookFailed);
    }
    //let real_begin_scene = (*vtbl).BeginScene;
    let real_release = (*vtbl).parent.Release;
    let real_present = (*vtbl).Present;

    // remember these functions but don't hook them yet
    let real_set_texture = (*vtbl).SetTexture;
    let real_create_texture = (*vtbl).CreateTexture;
    let real_update_texture = (*vtbl).UpdateTexture;

    let real_set_vertex_sc_f = (*vtbl).SetVertexShaderConstantF;
    let real_set_vertex_sc_i = (*vtbl).SetVertexShaderConstantI;
    let real_set_vertex_sc_b = (*vtbl).SetVertexShaderConstantB;

    let real_set_pixel_sc_f = (*vtbl).SetPixelShaderConstantF;
    let real_set_pixel_sc_i = (*vtbl).SetPixelShaderConstantI;
    let real_set_pixel_sc_b = (*vtbl).SetPixelShaderConstantB;

    let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

    // This was used to debug an issue with reshade where something
    // was unhooking the pointers after I hooked it. this issue exists
    // with multiple games when run under reshade, so it must be something
    // to do with how reshade manages that (possibly interference from
    // minhook or imgui)
    // write_log_file(&format!("DrawIndexedPrimitive real: {:x}, hook: {:x}",
    //     real_draw_indexed_primitive as usize,
    //     hook_draw_indexed_primitive as usize,
    // ));
    // write_log_file(&format!("Present real: {:x}, hook: {:x}",
    //     real_present as usize,
    //     hook_present as usize,
    // ));
    (*vtbl).DrawIndexedPrimitive = hook_draw_indexed_primitive;
    //(*vtbl).BeginScene = hook_begin_scene;
    (*vtbl).Present = hook_present;
    (*vtbl).parent.Release = hook_release;
    // Always hook CreateTexture; the hook checks force_tex_cpu_read at runtime
    // to decide whether to change DEFAULT pool to MANAGED for snapshotting.
    // JMQNOTE: This d3d9-specific change sets up a hook method that converts to using the MANAGED pool 
    // instead of DEFAULT for textures if the registry key enables it. 
    // Games that use MANAGED (every dx9 game I have tried until now)
    // don't need this.
    // For games that do use DEFAULT, this might be sufficient to capture textures,
    // which can't be captured from default. It wasn't for 2026g1, which micromanages textures
    // by putting them in SYSTEMMEM first and then copying to the device via UpdateTexture. 
    // The way to tell if this pool-flip 
    // method is sufficient is enable the reg key and load the game - 
    // if everything shows up black, this method is not sufficient.
    (*vtbl).CreateTexture = hook_create_texture;
    // Always hook UpdateTexture to track source->destination mappings so that
    // during snapshotting we can reach back to the lockable source texture.
    (*vtbl).UpdateTexture = hook_update_texture;

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
    write_log_file(&format!("periodic update freq: {} draw calls", HOOK_DRAW_PERIODIC_CALLS));

    Ok(HookDirect3D9Device::new(
        real_draw_indexed_primitive,
        //real_begin_scene,
        real_present,
        real_release,
        real_set_texture,
        real_create_texture,
        real_update_texture,
        real_set_vertex_sc_f,
        real_set_vertex_sc_i,
        real_set_vertex_sc_b,
        real_set_pixel_sc_f,
        real_set_pixel_sc_i,
        real_set_pixel_sc_b,
    ))
}

/// Hook for IDirect3DDevice9::CreateTexture.
/// When force_tex_cpu_read is enabled, changes D3DPOOL_DEFAULT to D3DPOOL_MANAGED
/// for regular textures (not render targets, depth stencils, or dynamic textures).
/// This allows D3DXSaveTextureToFileW to lock and read the texture data during
/// snapshotting. Falls back to original pool if creation with MANAGED fails.
unsafe extern "system" fn hook_create_texture(
    THIS: *mut IDirect3DDevice9,
    Width: UINT,
    Height: UINT,
    Levels: UINT,
    Usage: DWORD,
    Format: D3DFORMAT,
    Pool: D3DPOOL,
    ppTexture: *mut *mut IDirect3DTexture9,
    pSharedHandle: *mut winapi::um::winnt::HANDLE,
) -> HRESULT {
    let real_fn = match (dev_state()).hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
            dev.real_create_texture
        },
        _ => {
            write_log_file("hook_CreateTexture: no device state, returning E_FAIL");
            return E_FAIL;
        }
    };

    let mut actual_pool = Pool;
    let mut changed = false;

    if GLOBAL_STATE.run_conf.force_tex_cpu_read && Pool == D3DPOOL_DEFAULT {
        // Only change pool for textures without special usage flags that require DEFAULT pool
        let special_usage = D3DUSAGE_RENDERTARGET | D3DUSAGE_DEPTHSTENCIL | D3DUSAGE_DYNAMIC;
        if (Usage & special_usage) == 0 {
            actual_pool = D3DPOOL_MANAGED;
            changed = true;
        }
    }

    let res = (real_fn)(THIS, Width, Height, Levels, Usage, Format, actual_pool, ppTexture, pSharedHandle);

    if res != 0 && changed {
        // Retry with original pool if MANAGED creation failed
        write_log_file(&format!(
            "hook_CreateTexture: MANAGED pool failed (hr {:x}) for {}x{} fmt {}, retrying with DEFAULT",
            res, Width, Height, Format
        ));
        let res = (real_fn)(THIS, Width, Height, Levels, Usage, Format, Pool, ppTexture, pSharedHandle);
        if res == 0 {
            write_log_file("hook_CreateTexture: retry with original pool succeeded");
        } else {
            write_log_file(&format!("hook_CreateTexture: retry also failed: {:x}", res));
        }
        return res;
    }

    res
}

/// Hook for IDirect3DDevice9::UpdateTexture.
/// Records the (destination -> source) mapping in GLOBAL_STATE so that
/// snapshotting code can locate a lockable source texture when asked to
/// save a DEFAULT-pool destination texture. The most recent mapping for a
/// given destination wins.
///
/// Because the game may Release the source before we get a chance to
/// snapshot it, we AddRef the source the first time we see it and record
/// it in `dx9_update_texture_tracked_srcs` + `dx9_update_texture_deque`.
/// Subsequent UpdateTexture calls with a source we already track skip the
/// AddRef (so we only ever own a single ref per unique source, which lets
/// us later detect when the game has dropped its own refs by observing a
/// zero refcount on Release). The `dx9_update_texture_gc` pass periodically
/// releases these refs.
unsafe extern "system" fn hook_update_texture(
    THIS: *mut IDirect3DDevice9,
    pSourceTexture: *mut IDirect3DBaseTexture9,
    pDestinationTexture: *mut IDirect3DBaseTexture9,
) -> HRESULT {
    let real_fn = match (dev_state()).hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
            dev.real_update_texture
        },
        _ => {
            write_log_file("hook_UpdateTexture: no device state, returning E_FAIL");
            return E_FAIL;
        }
    };

    if !pSourceTexture.is_null() && !pDestinationTexture.is_null() {
        if GLOBAL_STATE.dx9_update_texture_map.is_none() {
            GLOBAL_STATE.dx9_update_texture_map = Some(fnv::FnvHashMap::default());
        }
        if GLOBAL_STATE.dx9_update_texture_tracked_srcs.is_none() {
            GLOBAL_STATE.dx9_update_texture_tracked_srcs = Some(fnv::FnvHashSet::default());
        }
        if GLOBAL_STATE.dx9_update_texture_deque.is_none() {
            GLOBAL_STATE.dx9_update_texture_deque = Some(std::collections::VecDeque::new());
        }

        let dest_key = pDestinationTexture as usize;
        let src_key = pSourceTexture as usize;

        if let Some(map) = GLOBAL_STATE.dx9_update_texture_map.as_mut() {
            map.insert(dest_key, src_key);
        }

        // Only AddRef the first time we see this source pointer; subsequent
        // references are deduped by the tracked-srcs set.
        let is_new = GLOBAL_STATE.dx9_update_texture_tracked_srcs.as_mut()
            .map(|set| set.insert(src_key))
            .unwrap_or(false);
        if is_new {
            (*pSourceTexture).AddRef();
            if let Some(deque) = GLOBAL_STATE.dx9_update_texture_deque.as_mut() {
                deque.push_back((src_key, std::time::SystemTime::now()));
            }
        }
    }

    (real_fn)(THIS, pSourceTexture, pDestinationTexture)
}

/// Garbage-collect the DX9 UpdateTexture source-tracking state.
///
/// Intended to be called periodically (e.g. every ~30 seconds from
/// `hook_present`). Pops up to `MAX_PER_PASS` entries from the front of
/// `dx9_update_texture_deque` (the oldest tracked sources). For each entry
/// older than `AGE_THRESHOLD`:
///
/// * Release the ref we AddRef'd in `hook_update_texture`.
/// * If the refcount is still > 0, the texture is still alive — probably
///   the game is still holding its own ref — so we re-AddRef and push the
///   entry back on the deque with a refreshed timestamp. This means we'll
///   wait another AGE_THRESHOLD before re-checking it.
/// * If the refcount reached zero, we were the last owner; the texture is
///   now destroyed. Remove it from the tracked-srcs set and purge any map
///   entries whose value points at this (now-dangling) source.
///
/// If the entry at the front is not yet old enough, stop early (the deque
/// is ordered oldest-first, so nothing behind is due either).
pub unsafe fn dx9_update_texture_gc() {
    use std::time::Duration;
    const AGE_THRESHOLD: Duration = Duration::from_mins(5);
    const MAX_PER_PASS: usize = 1000;
    const MAX_TIME_PER_PASS_MICROS:u128 = 10000;

    let start = Instant::now();

    let deque_len = GLOBAL_STATE.dx9_update_texture_deque.as_ref()
        .map(|d| d.len())
        .unwrap_or(0);
    if deque_len == 0 {
        return;
    }

    let count = deque_len.min(MAX_PER_PASS);
    let mut released_srcs: Vec<usize> = Vec::new();
    let mut processed: usize = 0;
    let mut refreshed: usize = 0;

    for i in 0..count {
        if i % 50 == 0 {
            // check to see if we ran out of time
            if Instant::now().duration_since(start).as_micros() > MAX_TIME_PER_PASS_MICROS {
                break;
            }
        }
        // Peek front; if not old enough, stop the whole pass.
        let front_due = GLOBAL_STATE.dx9_update_texture_deque.as_ref()
            .and_then(|d| d.front().copied())
            .map(|(_, t)| t.elapsed().map(|e| e >= AGE_THRESHOLD).unwrap_or(false))
            .unwrap_or(false);
        if !front_due {
            break;
        }

        let popped = GLOBAL_STATE.dx9_update_texture_deque.as_mut()
            .and_then(|d| d.pop_front());
        let (src_key, _) = match popped {
            Some(e) => e,
            None => break,
        };
        processed += 1;

        let src_ptr = src_key as *mut IDirect3DBaseTexture9;
        let remaining = (*src_ptr).Release();
        if remaining > 0 {
            // Still alive: re-take our ref and push back with a fresh stamp.
            (*src_ptr).AddRef();
            if let Some(deque) = GLOBAL_STATE.dx9_update_texture_deque.as_mut() {
                deque.push_back((src_key, std::time::SystemTime::now()));
            }
            refreshed += 1;
        } else {
            // Refcount hit zero — the texture is gone. Drop our tracking.
            if let Some(set) = GLOBAL_STATE.dx9_update_texture_tracked_srcs.as_mut() {
                set.remove(&src_key);
            }
            released_srcs.push(src_key);
        }
    }

    if !released_srcs.is_empty() {
        if let Some(map) = GLOBAL_STATE.dx9_update_texture_map.as_mut() {
            map.retain(|_, v| !released_srcs.contains(v));
        }
    }

    if processed > 0 {
        write_log_file(&format!(
            "dx9_update_texture_gc: processed {} (refreshed {}, released {}), deque now {}; elapsed: {}micros",
            processed,
            refreshed,
            released_srcs.len(),
            GLOBAL_STATE.dx9_update_texture_deque.as_ref().map(|d| d.len()).unwrap_or(0),
            Instant::now().duration_since(start).as_micros()
        ));
    }
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
    {
        let trylock = GLOBAL_STATE_LOCK.try_lock();
        match trylock {
            Ok(_) => {
                //write_log_file("create_and_hook_device: lock is free (normal)");
            },
            Err(_) => {
                write_log_file("create_and_hook_device: error: lock is already held, will deadlock");
            }
        }
    }

    let lock = GLOBAL_STATE_LOCK
        .lock()
        .map_err(|_err| HookError::GlobalLockError)?;

    if DEVICE_STATE == null_mut() {
        return Err(HookError::BadStateError("no device state pointer??".to_owned()));
    }

    // Query run configuration (e.g. force_tex_cpu_read) so DX9 hooks can use it
    query_and_set_runconf_in_globalstate(true);

    (*DEVICE_STATE)
        .hook
        .as_mut()
        .ok_or(HookError::Direct3D9InstanceNotFound)
        .and_then(|hook| {
            match hook {
                HookDeviceState::D3D9(ds) if ds.d3d9.is_some() => Ok(ds),
                _ => Err(HookError::D3D9HookFailed)
            }
        })
        .and_then(|hd3d9| {
            write_log_file(&format!("calling real create device"));
            if BehaviorFlags & D3DCREATE_MULTITHREADED == D3DCREATE_MULTITHREADED {
                write_log_file(&format!(
                    "Notice: device being created with D3DCREATE_MULTITHREADED"
                ));
            }
            // option is_some() checked earlier
            let result = (hd3d9.d3d9.as_ref().unwrap().real_create_device)(
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
            hook_d3d9_device(*ppReturnedDeviceInterface, &lock)
        })
        .and_then(|hook_d3d9device| {
            match (*DEVICE_STATE).hook {
                Some(HookDeviceState::D3D9(ref mut d3d9)) => d3d9.device = Some(hook_d3d9device),
                _ => ()
            };
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

    // TODO: should do this on late-hook path, not here
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

/// Allocate the device state object (pointer).  The "once"
/// suggests we only expect this to be called once per app,
/// but if called more than once as some apps tend to do,
/// we may reallocate device state, unless a hook has already been
/// set.  After the hook has been set we can't really reallocate
/// because the hook structures are not copyable and so nulling them
/// out, as we would need to do in a new allocation, would lose
/// the addresses of any "real" functions such as create device.
pub fn init_device_state_once() -> bool {
    unsafe {
        // its possible to get in here more than once in same process
        // (if it creates multiple devices).  leak the previous
        // pointer to avoided crashes; if the game is creating devices
        // in a tight loop we've got bigger problems than a memory leak.
        // note: in a single threaded env nothing else should be
        // using the state right now so we could free it.
        let was_init = DEVICE_STATE != null_mut();
        let has_hook = DEVICE_STATE != null_mut() && (*DEVICE_STATE).hook.is_some();

        // allow it be created if it doesn't exist yet or if there is no hook yet
        if !was_init || !has_hook {
            DEVICE_STATE = Box::into_raw(Box::new(DeviceState {
                hook: None,
                d3d_window: null_mut(),
                d3d_resource_count: 0,
            }));

            write_log_file(&format!("initted new device state instance: {}; was initted: {}", DEVICE_STATE as usize, was_init));
        }
        // but if there is a hook already don't replace it since we might lose the real hook fn addresses if we do that
        else if has_hook {
            write_log_file(&format!("not creating new device state because it already has a hook"));
        }

        was_init
    }
}

pub fn init_log(mm_root:&str) {
    if log_initted_on_this_thread() {
        write_log_file("log already initialized on this thread");
        return;
    }
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

            if !clear_log_file {
                writeln!(f, "ModelMod log file reinitialized (clear_log_file is false)")?;
            }
            writeln!(f, "ModelMod initialized, built with rustc: {} {}, git hash: {}, build date: {}, mm root: {}",
                super::RUSTCVER, super::RUSTCDATE, super::GIT_HASH, super::BUILD_TS, mm_root)?;
            writeln!(f, "Detected root directory (from registry, set by MMLaunch): {}", mm_root)?;

            // if that succeeded then we can set the file name now
            set_log_file_path(&tdir, &file_name)?;

            eprintln!("Log File: {}", tname);

            set_log_initted_on_this_thread();

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
    // Disabled because I saw it on a very sleepy profile and it
    // shouldn't be called at all right now.
    return 0;

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

        let device:LPDIRECT3DDEVICE9 = std::ptr::with_exposed_provenance_mut::<global_state::IDirect3DDevice9>(praw as usize);

        let hookit = || -> Result<()> {
            let lock = GLOBAL_STATE_LOCK
            .lock()
            .map_err(|_err| HookError::GlobalLockError)?;

            let hook_d3d9device = hook_d3d9_device(device, &lock)?;

            //(*DEVICE_STATE).d3d_window = hFocusWindow; // TODO: need to get this in late hook API
            (*DEVICE_STATE).hook = Some(HookDeviceState::D3D9(HookD3D9State {
                d3d9: None,
                device: Some(hook_d3d9device)
            }));
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

pub fn load_d3d_lib(name:&str) -> Result<*mut HINSTANCE__> {
    unsafe {
        let bsize:u32 = 65535;
        let mut syswide: Vec<u16> = Vec::with_capacity(bsize as usize);
        let res = winapi::um::sysinfoapi::GetSystemDirectoryW(syswide.as_mut_ptr(), bsize);
        if res == 0 {
            write_log_file(&format!("Failed to get system directory, can't load {}", name));
            return Err(HookError::D3D9HookFailed);
        }
        syswide.set_len(res as usize);
        let mut sd = util::from_wide_fixed(&syswide)?;
        sd.push_str("\\");
        sd.push_str(name);

        let handle = util::load_lib(&sd)?;
        Ok(handle)
    }
}

pub fn create_d3d9(sdk_ver: u32) -> Result<*mut IDirect3D9> {
    init_device_state_once();

    // load d3d9 lib.  do this before trying to load managed lib, because if we can't load d3d9
    // there is no point in loading the managed stuff.  however this means that if this fails,
    // the logging will go to the %temp%\ModelMod.log file.
    // Note: _handle is never unloaded, IDK if there is a reason a game would ever do that
    let (_handle,addr) = {
        let handle = load_d3d_lib("d3d9.dll")?;
        let addr = util::get_proc_address(handle, "Direct3DCreate9")?;
        (handle,addr)
    };

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
        write_log_file(&format!("created d3d: {:x}", direct3d9 as usize));

        // let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);
        // write_log_file(&format!("vtbl: {:x}", vtbl as usize));

        // don't hook more than once
        let _lock = GLOBAL_STATE_LOCK
            .lock()
            .map_err(|_err| HookError::D3D9HookFailed)?;

        match (*DEVICE_STATE).hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: ref what, device: _ })) => {
                let _ = what;
                return Ok(direct3d9);
            },
            _ => {}
        };

        GLOBAL_STATE.mm_root = Some(mm_root);

        // get pointer to original vtable
        let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);

        // save pointer to real function
        let real_create_device = (*vtbl).CreateDevice;
        // hax check
        let chook = hook_create_device as u64;
        let creal = real_create_device as u64;
        if chook == creal {
            write_log_file(&format!("error: oops, the supposedly real create device function appears to be the hook function already; bailing out to avoid infinite recursion"));
            return Ok(direct3d9);
        }
        // write_log_file(&format!(
        //     "hooking real create device, hookfn: {:?}, realfn: {:?} ",
        //     hook_create_device as usize, real_create_device as usize
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

        (*DEVICE_STATE).hook =
            Some(HookDeviceState::D3D9(HookD3D9State {
                d3d9: Some(hd3d9),
                device: None
            }));

        write_log_file(&format!("device state set with hook on ds instance: {}", DEVICE_STATE as u64));
        Ok(direct3d9)
    }
}
