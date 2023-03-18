use device_state::dev_state_d3d11_nolock;
use global_state::HookState;
use types::d3ddata::ModD3DData9;
use types::native_mod::ModD3DData;
use types::native_mod::NativeModData;
use winapi::um::unknwnbase::IUnknown;

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::wingdi::RGNDATA;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use dnclr::{init_clr, reload_managed_dll};

use util;
use mod_load;
use mod_load::AsyncLoadState;
use crate::input_commands;
use crate::mod_render;
use global_state::{GLOBAL_STATE, GLOBAL_STATE_LOCK};
use global_state::FrameMetrics;
use device_state::dev_state;
use hook_snapshot;
use types::native_mod;

use std;
use std::ptr::null_mut;
use std::time::SystemTime;

use shared_dx::util::*;
use shared_dx::error::*;
use shared_dx::types::*;

pub (crate) const CLR_OK:u64 = 1;
pub (crate) const CLR_FAIL:u64 = 666;
pub (crate) const MAX_STAGE: usize = 16;

fn get_current_texture() -> usize {
    unsafe {
        let idx = GLOBAL_STATE.curr_texture_index;
        GLOBAL_STATE
            .active_texture_list
            .as_ref()
            .map(|list| {
                if idx >= list.len() {
                    0
                } else {
                    list[idx]
                }
            })
            .unwrap_or(0)
    }
}

#[inline]
/// If selection mode is not active, this returns None.  If it is active, and the current texture
/// is selected, returns the texture that should be used to override the current (aka the
/// "selection texture") as well as the stage it should be set on.  For D3D9 this is the actual stage,
/// for D3D11 it is the index into the current pixel shader resource array.  If the current texture,
/// is not selected, returns None.
pub unsafe fn get_override_tex_if_selected<'a, T, F>(extract_ptr:F) -> Option<(*mut T, DWORD, bool)>
where F: FnOnce(&TexPtr) -> *mut T {
    if GLOBAL_STATE.making_selection {
        get_selected_texture_stage()
            .map(|stage| {
                GLOBAL_STATE.selection_texture.as_ref()
                .and_then(|seltext| {
                    Some((
                        extract_ptr(seltext),
                        stage,
                        true,
                    ))
                })
            })
            .flatten()
    } else {
        None
    }
}
fn get_selected_texture_stage() -> Option<DWORD> {
    unsafe {
        for i in 0..MAX_STAGE {
            if GLOBAL_STATE.selected_on_stage[i] {
                return Some(i as DWORD);
            }
        }
        None
    }
}

/// Controls how often `process_metrics` reports stats (regardless of how frequently it is called)
const METRICS_MIN_INTERVAL_SECS:f64 = 10.0;

/// Perform a metrics update if the number of dip calls exceeds `interval`.  If
/// an update is performed, the tracked primitive list will also be cleared.  If there
/// is no update it will be cleared too, unless the caller passes true for `preserve_prims`.
/// This allows `process_metrics` to be called from high frequency functions such as
/// d3d11 draw_indexed, and avoids clearing the list too soon in that case.
/// If global_state::METRICS_TRACK_PRIMS
/// is false there shouldn't be any primitives in the list anyway.
pub fn process_metrics(metrics:&mut FrameMetrics, preserve_prims:bool, interval:u32) {
    if metrics.dip_calls > interval {
        let mut report_dips_fps = true;

        let now = SystemTime::now();
        let elapsed = now.duration_since(metrics.last_call_log);
        let mut dip_stats_updated = false;
        match elapsed {
            Ok(d) => {
                let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                if secs >= METRICS_MIN_INTERVAL_SECS {
                    // process dx11 metrics (if any).  do this first because if we are using dx11 we
                    // want to note that since we might not log some stuff below (which is less useful
                    // in dx11)
                    unsafe {dev_state_d3d11_nolock()}.map(|state| {
                        // don't log these in dx11 because dips fps is not set and dips not usually useful
                        report_dips_fps = false;
                        let metrics = &state.metrics;
                        let ms_since_reset = metrics.ms_since_reset();
                        if metrics.vs_set_const_buffers_hooks > 0 && ms_since_reset > 0 {
                            let vshookrate = if metrics.vs_set_const_buffers_calls > 0 {
                                metrics.vs_set_const_buffers_hooks as f64 / metrics.vs_set_const_buffers_calls as f64
                            } else {
                                0.0
                            };
                            // also compute hooks per 100 ms
                            let vshookrate_per_sec = metrics.vs_set_const_buffers_hooks as f64 / ms_since_reset as f64 * 100.0;


                            write_log_file(&format!("dx11 metrics: {}ms; vsscb hooks: {:.2}%, {}={:.2}/100ms",
                                ms_since_reset, vshookrate * 100.0, metrics.vs_set_const_buffers_hooks, vshookrate_per_sec));
                            // write_log_file(&format!("  VSSetConstantBuffers calls: {}, hooks: {}",
                            //     metrics.vs_set_const_buffers_calls,
                            //     metrics.vs_set_const_buffers_hooks
                            // ));
                        }
                        if metrics.rehook_calls > 0 {
                            let rehook_ms = metrics.rehook_time_nanos / 1000 / 1000;
                            write_log_file(&format!("  rehook calls: {}, total ms: {}", metrics.rehook_calls, rehook_ms));
                        }
                        if metrics.drawn_recently.len() > 0 {
                            write_log_file("  drawn recently:");
                            for (pv, ds) in &metrics.drawn_recently {
                                write_log_file(&format!("    {:?}: {:?}", pv, ds));
                            }
                        }
                        state.metrics.reset();
                    });

                    let dipsec = metrics.dip_calls as f64 / secs;

                    dip_stats_updated = true;
                    if report_dips_fps {
                        write_log_file(&format!(
                            "{} dip calls in {:.*} secs ({:.*} dips/sec (fps: {:.*}))",
                            metrics.dip_calls, 2, secs, 2, dipsec, 2, metrics.last_fps
                        ));
                    }
                    unsafe {&mut GLOBAL_STATE}.active_texture_set.as_ref().map(|set| {
                        write_log_file(&format!(
                            "active texture set contains: {} textures",
                            set.len()
                        ))
                    });
                    metrics.last_call_log = now;
                    metrics.dip_calls = 0;
                }
            }
            Err(e) => write_log_file(&format!("Error getting elapsed duration: {:?}", e)),
        }

        // dump out the prim list every so often if we are tracking that.
        // note this only dumps out the primitives for the most recent frame.
        // also only write these out when we also just wrote a dip summary line
        // above.
        if global_state::METRICS_TRACK_PRIMS && dip_stats_updated {
            use global_state::RenderedPrimType::{PrimVertCount, PrimCountVertSizeAndVBs};
            let logname = shared_dx::util::get_log_file_path();
            if !logname.is_empty() && metrics.rendered_prims.len() > 0 {
                let p = std::path::Path::new(&logname);
                match p.parent() {
                    Some(par) => {
                        let mut pb = par.to_path_buf();
                        // TODO: should probably include exe name
                        pb.push("rendered_last_frame.txt");
                        let w = || -> std::io::Result<()> {
                            write_log_file(&format!("writing {} frame prim metrics to '{}'", &metrics.rendered_prims.len(), pb.as_path().display()));
                            let mut res_combined = String::new();
                            for tracked_prim in &metrics.rendered_prims {
                                match &tracked_prim {
                                    &PrimVertCount(prim,vert)=> {
                                        //writeln!(res_combined, "{},{}\r", prim, vert);
                                        // PERF: ugh, a lot of little allocations here...
                                        res_combined.push_str(&format!("{},{}\r", prim, vert));
                                    },
                                    &PrimCountVertSizeAndVBs(prim, vsize, vbvec) => {
                                        res_combined.push_str(&format!("{},{},{:?}\r\n", prim, vsize, vbvec));
                                    }
                                }
                            }

                            use std::fs::OpenOptions;
                            use std::io::Write;

                            let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&pb)?;
                            writeln!(f, "{}", res_combined)?;
                            Ok(())
                        };

                        w().unwrap_or_else(|e| write_log_file(&format!("metrics file write error: {}", e)));
                    },
                    None => {}
                }
            }
        }
        // always clear prims after an update, whether we wrote them or not (prevents them
        // from accumulating)
        unsafe {&mut GLOBAL_STATE}.metrics.rendered_prims.clear();


    } else {
        // not time for update, but clear the prim list unless caller said not to
        if !preserve_prims {
            unsafe {&mut GLOBAL_STATE}.metrics.rendered_prims.clear();
        }
    }
}

/// Should be called periodically to complete initialization of the .net common language
/// runtime.  In DX9, this is called by `do_per_frame_operations` once per frame.  No-ops if
/// CLR is already loaded.  Should not be cpu-intensive to call this unless the CLR does need to
/// be loaded, in which case its at least a few hundred ms, but it only happens once.
pub fn frame_init_clr(run_context:&'static str) -> Result<()> {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    if hookstate.clr.runtime_pointer.is_none() {
        let lock = GLOBAL_STATE_LOCK.lock();
        match lock {
            Ok(_ignored) => {
                if hookstate.clr.runtime_pointer.is_none() {
                    // store something in clr_pointer even if it create fails,
                    // so that we don't keep trying to create it.  clr_pointer is
                    // really just a bool right now, it remains to be
                    // seen whether storing anything related to clr in
                    // global state is actually useful.
                    write_log_file("creating CLR");
                    init_clr(&hookstate.mm_root)
                        .and_then(|_x| {
                            reload_managed_dll(&hookstate.mm_root, Some(run_context))
                        })
                        .and_then(|_x| {
                            hookstate.clr.runtime_pointer = Some(CLR_OK);
                            hookstate.clr.run_context = run_context.to_owned();
                            Ok(_x)
                        })
                        .map_err(|e| {
                            write_log_file(&format!("Error creating CLR: {:?}", e));
                            hookstate.clr.runtime_pointer = Some(CLR_FAIL);
                            e
                        })?;
                }
            }
            Err(e) => write_log_file(&format!("{:?} should never happen", e)),
        };
    }
    Ok(())
}

pub fn frame_load_mods(deviceptr: DevicePointer) {
    let interop_state = unsafe { &mut GLOBAL_STATE.interop_state };
    interop_state.as_mut().map(|is| {
        if !is.loading_mods && !is.done_loading_mods && is.conf_data.LoadModsOnStart {
            let loadstate = unsafe { (is.callbacks.GetLoadingState)() };
            if loadstate == AsyncLoadState::InProgress as i32 {
                is.loading_mods = true;
                is.done_loading_mods = false;
            } else if loadstate != AsyncLoadState::Pending as i32 {
                let r = unsafe { (is.callbacks.LoadModDB)() };
                if r == AsyncLoadState::Pending as i32 {
                    is.loading_mods = true;
                    is.done_loading_mods = false;
                }
                if r == AsyncLoadState::Complete as i32 {
                    is.loading_mods = false;
                    is.done_loading_mods = true;
                }
                write_log_file(&format!("mod db load returned: {}", r));
            }
        }

        if is.loading_mods
            && unsafe { (is.callbacks.GetLoadingState)() } == AsyncLoadState::Complete as i32
        {
            write_log_file("mod loading complete");
            is.loading_mods = false;
            is.done_loading_mods = true;

            match deviceptr {
                DevicePointer::D3D11(_)
                | DevicePointer::D3D9(_) =>
                    unsafe { mod_load::setup_mod_data(deviceptr, is.callbacks) },
            }
        }

        let has_pending_mods =
            unsafe {&GLOBAL_STATE}.load_on_next_frame
                .as_ref().map_or(false, |hs| hs.len() > 0);

        if has_pending_mods && is.done_loading_mods && !is.loading_mods {
            match deviceptr {
                DevicePointer::D3D11(_)
                | DevicePointer::D3D9(_) =>
                    unsafe { mod_load::load_deferred_mods(deviceptr, is.callbacks) },
            }
        }
    });
}
pub fn do_per_frame_operations(device: *mut IDirect3DDevice9) -> Result<()> {
    // write_log_file(&format!("performing per-scene ops on thread {:?}",
    //         std::thread::current().id()));

    frame_init_clr(dnclr::RUN_CONTEXT_D3D9)?;
    frame_load_mods(DevicePointer::D3D9(device));

    let metrics = &mut unsafe {&mut GLOBAL_STATE}.metrics;

    const METRICS_DIPS_INTERVAL:u32 = 1_000_000;
    process_metrics(metrics, false, METRICS_DIPS_INTERVAL);

    Ok(())
}

pub fn track_set_texture(tex_as_int:usize, tex_stage:u32, global_state:&mut HookState) {
    if !global_state.making_selection {
        return;
    }

    let has_it = global_state
        .active_texture_set
        .as_ref()
        .map(|set| set.contains(&tex_as_int))
        .unwrap_or(true);
    if !has_it {
        global_state.active_texture_set.as_mut().map(|set| {
            set.insert(tex_as_int);
        });
        global_state.active_texture_list.as_mut().map(|list| {
            list.push(tex_as_int);
        });
    }

    if tex_stage < MAX_STAGE as u32 {
        let curr = get_current_texture();
        if curr != 0 && tex_as_int == curr {
            global_state.selected_on_stage[tex_stage as usize] = true;
        } else if global_state.selected_on_stage[tex_stage as usize] {
            global_state.selected_on_stage[tex_stage as usize] = false;
        }
    } else {
        write_log_file(&format!("WARNING: texture stage {} is too high (max {})", tex_stage, MAX_STAGE));
    }
}

pub (crate) unsafe extern "system" fn hook_set_texture(
    THIS: *mut IDirect3DDevice9,
    Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,
) -> HRESULT {
    if GLOBAL_STATE.making_selection {
        track_set_texture(pTexture as usize, Stage, &mut GLOBAL_STATE);
    }

    match (dev_state()).hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
            (dev.real_set_texture)(THIS, Stage, pTexture)
        },
        _ => E_FAIL
    }
}

// TODO: hook this up to device release at the proper time
unsafe fn purge_device_resources(device: DevicePointer) {
    if device.is_null() {
        write_log_file("WARNING: ignoring insane attempt to purge devices on a null device");
        return;
    }
    mod_load::clear_loaded_mods(device);
    let seltext = GLOBAL_STATE.selection_texture.take();
    seltext.map(|t| t.release());

    GLOBAL_STATE
        .input
        .as_mut()
        .map(|input| input.clear_handlers());
    dev_state().d3d_resource_count = 0;
}

pub unsafe extern "system" fn hook_present(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT {
    //write_log_file("present");

    let call_real_present = || {
        match (dev_state()).hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
                (dev.real_present)(
                    THIS,
                    pSourceRect,
                    pDestRect,
                    hDestWindowOverride,
                    pDirtyRegion,
                )
            },
            _ => E_FAIL
        }
    };
    if GLOBAL_STATE.in_any_hook_fn() {
        return call_real_present();
    }

    if let Err(e) = do_per_frame_operations(THIS) {
        write_log_file(&format!(
            "unexpected error from do_per_scene_operations: {:?}",
            e
        ));
        return call_real_present()
    }

    let min_fps = GLOBAL_STATE
        .interop_state
        .map(|is| is.conf_data.MinimumFPS)
        .unwrap_or(0) as f64;

    let metrics = &mut GLOBAL_STATE.metrics;
    let present_ret = dev_state()
        .hook
        .as_mut()
        .map_or(S_OK, |_hdstate| {
            metrics.frames += 1;
            metrics.total_frames += 1;
            if metrics.frames % 90 == 0 {
                // enforce min fps
                // NOTE: when low, it just sets a boolean flag to disable mod rendering,
                // but we could also use virtual protect to temporarily swap out the hook functions
                // (except for present)
                let now = SystemTime::now();
                let elapsed = now.duration_since(metrics.last_fps_update);
                if let Ok(d) = elapsed {
                    let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                    let fps = metrics.frames as f64 / secs;
                    let smooth_fps = 0.3 * fps + 0.7 * metrics.last_fps;
                    metrics.last_fps = smooth_fps;
                    let min_off = min_fps * 1.1;
                    if smooth_fps < min_fps && !metrics.low_framerate {
                        metrics.low_framerate = true;
                    }
                    // prevent oscillation: don't reactivate until 10% above mininum
                    else if metrics.low_framerate && smooth_fps > (min_off * 1.1) {
                        metrics.low_framerate = false;
                    }
                    // write_log_file(&format!(
                    //     "{} frames in {} secs ({} instant, {} smooth) (low: {})",
                    //     hookdevice.frames, secs, fps, smooth_fps, hookdevice.low_framerate
                    // ));
                    metrics.last_fps_update = now;
                    metrics.frames = 0;
                }
            }
            call_real_present()
        });

    if GLOBAL_STATE.selection_texture.is_none() {
        input_commands::create_selection_texture_d3d9(THIS);
    }

    if util::appwnd_is_foreground(dev_state().d3d_window) {
        GLOBAL_STATE.input.as_mut().map(|inp| {
            if inp.get_press_fn_count() == 0 {
                input_commands::setup_input(DevicePointer::D3D9(THIS), inp)
                    .unwrap_or_else(|e| write_log_file(&format!("input setup error: {:?}", e)));
            }
            inp.process()
                .unwrap_or_else(|e| write_log_file(&format!("input error: {:?}", e)));
        });
    }

    if GLOBAL_STATE.is_snapping {
        // this may set is_snapping = false if the snapshot is done
        hook_snapshot::present_process();
    }

    present_ret
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
    // TODO: hack to work around Release on device while in DIP

    // I don't think release can "fail" normally but in rare/weird cases this
    // version might because we lack a context (and therefore don't have the address
    // of the real function) so this is what we return in those.
    let failret:ULONG = 0xFFFFFFFF;
    let oops_log_release_fail = || {
        write_log_file(&format!("OOPS hook_release returning {} due to bad state", failret));
    };

    if GLOBAL_STATE.in_hook_release {
        return match (dev_state()).hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
                (dev.real_release)(THIS)
            },
            _ => {
                oops_log_release_fail();
                failret
            }
        };
    }

    GLOBAL_STATE.in_hook_release = true;

    // dev_state() used to return an Option, but doesn't now,
    // so Some() it for compat with old combinator flow
    let r = Some(dev_state())
        .as_mut()
        .map_or(failret, |hookds| {
            let hookdevice = match hookds.hook {
                Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref mut dev) })) => {
                    dev
                },
                _ => { return failret; } // "should never happen"
            };
            hookdevice.ref_count = (hookdevice.real_release)(THIS);

            // if hookdevice.ref_count < 100 {
            //     write_log_file(&format!(
            //         "device {:x} refcount now {}",
            //         THIS as usize, hookdevice.ref_count
            //     ));
            // }

            // could just leak everything on device destroy.  but I know that will
            // come back to haunt me.  so make an effort to purge my stuff when the
            // resource count gets to the expected value, this way the device can be
            // properly disposed.

            let destroying = dev_state().d3d_resource_count > 0
                && hookdevice.ref_count == (dev_state().d3d_resource_count + 1);
            if destroying {
                // purge my stuff
                write_log_file(&format!(
                    "device {:x} refcount is same as internal resource count ({}),
                    it is being destroyed: purging resources",
                    THIS as usize, dev_state().d3d_resource_count
                ));
                purge_device_resources(DevicePointer::D3D9(THIS as *mut IDirect3DDevice9));
                // Note, hookdevice.ref_count is wrong now since we bypassed
                // this function during unload (no re-entrancy).  however the count on the
                // device should be 1 if I did the math right, anyway the release below
                // will fix the count.
            }

            if destroying || (dev_state().d3d_resource_count == 0 && hookdevice.ref_count == 1) {
                // release again to trigger destruction of the device
                hookdevice.ref_count = (hookdevice.real_release)(THIS);
                write_log_file(&format!(
                    "device released: {:x}, refcount: {}",
                    THIS as usize, hookdevice.ref_count
                ));
                if hookdevice.ref_count != 0 {
                    write_log_file(&format!(
                        "WARNING: unexpected ref count of {} after supposedly final
                        device release, device probably leaked",
                        hookdevice.ref_count
                    ));
                }
            }
            hookdevice.ref_count
        });
    GLOBAL_STATE.in_hook_release = false;

    if r == failret {
        oops_log_release_fail();
    }
    r
}

// TODO: maybe remove if not needed
// pub unsafe extern "system" fn hook_begin_scene(THIS: *mut IDirect3DDevice9) -> HRESULT {
//     if GLOBAL_STATE.in_any_hook_fn() {
//         return (GLOBAL_STATE.hook_direct3d9device.unwrap().real_begin_scene)(THIS);
//     }
//     GLOBAL_STATE.in_beginend_scene = true;

//     if let Err(e) = do_per_frame_operations(THIS) {
//         write_log_file(&format!("unexpected error: {:?}", e));
//         return E_FAIL;
//     }

//     let r = GLOBAL_STATE
//         .hook_direct3d9device
//         .as_ref()
//         .map_or(E_FAIL, |hookdevice| (hookdevice.real_begin_scene)(THIS));

//     GLOBAL_STATE.in_beginend_scene = false;
//     r
// }

decl_profile_globals!(hdip);

/// Render a mod using d3d9.  Returns true if the mod was rendered, false if not.
unsafe fn render_mod_d3d9(THIS:*mut IDirect3DDevice9, d3dd:&ModD3DData9, nmod:&NativeModData,
    override_texture: *mut IDirect3DBaseTexture9, override_stage:u32,
    primVerts:(u32,u32)) -> bool {
    if THIS == null_mut() {
        write_log_file("render_mod_d3d9: null device");
        return false;
    }

    profile_start!(hdip, mod_render);
    let (primCount,NumVertices) = primVerts;
    // save state
    let mut pDecl: *mut IDirect3DVertexDeclaration9 = null_mut();
    let ppDecl: *mut *mut IDirect3DVertexDeclaration9 = &mut pDecl;
    let hr = (*THIS).GetVertexDeclaration(ppDecl);
    if hr != 0 {
        write_log_file(&format!(
            "failed to save vertex declaration when trying to render mod {} {}",
            NumVertices, primCount
        ));
        return false;
    };

    let mut pStreamVB: *mut IDirect3DVertexBuffer9 = null_mut();
    let ppStreamVB: *mut *mut IDirect3DVertexBuffer9 = &mut pStreamVB;
    let mut offsetBytes: UINT = 0;
    let mut stride: UINT = 0;

    let hr = (*THIS).GetStreamSource(0, ppStreamVB, &mut offsetBytes, &mut stride);
    if hr != 0 {
        write_log_file(&format!(
            "failed to save vertex data when trying to render mod {} {}",
            NumVertices, primCount
        ));
        if pDecl != null_mut() {
            (*pDecl).Release();
        }
        return false;
    }

    // Note: C++ code did not change StreamSourceFreq...may need it for some games.
    (*THIS).SetVertexDeclaration(d3dd.decl);
    (*THIS).SetStreamSource(0, d3dd.vb, 0, nmod.mod_data.numbers.vert_size_bytes as u32);

    // set mod textures
    let mut save_tex:[Option<*mut IDirect3DBaseTexture9>; 4] = [None; 4];
    let mut _st_rods:Vec<ReleaseOnDrop<*mut IDirect3DBaseTexture9>> = vec![];
    for (i,tex) in d3dd.textures.iter().enumerate() {
        if *tex != null_mut() {
            //write_log_file(&format!("set override tex stage {} to {:x} for mod {}/{}", i, *tex as usize, NumVertices, primCount));
            let mut save:*mut IDirect3DBaseTexture9 = null_mut();
            (*THIS).GetTexture(i as u32, &mut save);
            save_tex[i] = Some(save);
            (*THIS).SetTexture(i as u32, *tex as *mut IDirect3DBaseTexture9);
            _st_rods.push(ReleaseOnDrop::new(save));
        }
    }

    // set the override tex, which is the (usually) the selection tex.  this might overwrite
    // the mod tex we just set.
    let mut save_texture: *mut IDirect3DBaseTexture9 = null_mut();
    let _st_rod = {
        if override_texture != null_mut() {
            (*THIS).GetTexture(override_stage, &mut save_texture);
            (*THIS).SetTexture(override_stage, override_texture);
            Some(ReleaseOnDrop::new(save_texture))
        } else {
            None
        }
    };

    // draw
    (*THIS).DrawPrimitive(
        nmod.mod_data.numbers.prim_type as u32,
        0,
        nmod.mod_data.numbers.prim_count as u32,
    );

    // restore state
    (*THIS).SetVertexDeclaration(pDecl);
    (*THIS).SetStreamSource(0, pStreamVB, offsetBytes, stride);
    // restore textures
    for (i,tex) in save_tex.iter().enumerate() {
        tex.map(|tex| {
            (*THIS).SetTexture(i as u32, tex);
        });
    }
    if override_texture != null_mut() {
        (*THIS).SetTexture(override_stage, save_texture);
    }
    (*pDecl).Release();
    (*pStreamVB).Release();
    profile_end!(hdip, mod_render);

    true
}

/// Return values from `check_and_render_mod`
pub enum CheckRenderModResult {
    /// A mod was found and rendered, the value is the mod type of the mod.
    Rendered(i32),
    /// No mod was found to render.
    NotRendered,
    /// A deletion mod was found.
    Deleted,
    /// A mod was found but data is not loaded for it, data load is now queued.
    /// The mod name is returned in case the caller needs to append any data
    /// to the native mod structure that is required to complete the load.
    NotRenderedButLoadRequested(String),
}
/// Check for a mod to render, and if one is found, render it using the supplied function `F`.
/// Returns `CheckRenderModResult` to indicate to result of this check.
pub unsafe fn check_and_render_mod<F>(primCount:u32, NumVertices: u32, rfunc:F) -> CheckRenderModResult
where
    F: FnOnce(&ModD3DData,&NativeModData) -> bool {

    let mut loading_mod_name = None;
    let res = GLOBAL_STATE.loaded_mods.as_mut()
        .and_then(|mods| {
            profile_start!(hdip, mod_select);

            let r = mod_render::select(mods,
                primCount, NumVertices,
                GLOBAL_STATE.metrics.total_frames);
            profile_end!(hdip, mod_select);
            r
        })
        .and_then(|nmod| {
            // early out if mod is a deletion mod
            if nmod.mod_data.numbers.mod_type == types::interop::ModType::Deletion as i32 {
                return Some(nmod.mod_data.numbers.mod_type);
            }
            // if the mod d3d data isn't loaded, can't render
            let d3dd = match nmod.d3d_data {
                native_mod::ModD3DState::Loaded(ref d3dd) => d3dd,
                // could observe partial if we noted it previously but the deferred load
                // hasn't happened yet (since it happens less often)
                native_mod::ModD3DState::Partial(_)
                | native_mod::ModD3DState::Unloaded => {
                    // tried to render an unloaded mod, make a note that it should be loaded
                    let load_next_hs = GLOBAL_STATE.load_on_next_frame.get_or_insert_with(
                        || fnv::FnvHashSet::with_capacity_and_hasher(
                            100,
                            Default::default(),
                        ));
                    loading_mod_name = Some(nmod.name.to_owned());
                    load_next_hs.insert(nmod.name.to_owned());
                    return None;
                }
            };

            let rendered = rfunc(d3dd,nmod);
            if rendered {
                Some(nmod.mod_data.numbers.mod_type)
            } else {
                None
            }
        });

    match (res,loading_mod_name) {
        (None,None) => CheckRenderModResult::NotRendered,
        (Some(mod_type),_) if mod_type == types::interop::ModType::Deletion as i32 => CheckRenderModResult::Deleted,
        (Some(mod_type),_) => CheckRenderModResult::Rendered(mod_type),
        (None,Some(name)) => CheckRenderModResult::NotRenderedButLoadRequested(name),
    }
}

pub unsafe extern "system" fn hook_draw_indexed_primitive(
    THIS: *mut IDirect3DDevice9,
    PrimitiveType: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT {
    let force_modding_off = false;

    profile_start!(hdip, hook_dip);

    // no re-entry please
    profile_start!(hdip, dip_check);
    if GLOBAL_STATE.in_dip {
        write_log_file(&format!("ERROR: i'm in DIP already!"));
        return S_OK;
    }
    profile_end!(hdip, dip_check);

    profile_start!(hdip, state_begin);

    let hookdevice = match dev_state().hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref mut dev) })) => dev,
        _ => {
            write_log_file(&format!("DIP: No d3d9 device found"));
            return E_FAIL;
        },
    };
    profile_end!(hdip, state_begin);

    let mut metrics = &mut GLOBAL_STATE.metrics;

    if !GLOBAL_STATE.is_snapping && (metrics.low_framerate || !GLOBAL_STATE.show_mods || force_modding_off) {
        return (hookdevice.real_draw_indexed_primitive)(
            THIS,
            PrimitiveType,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        );
    }

    // for snapshot selection, check to see if current selected texture is being rendered, and if
    // so obtain the override (selection) texture pointer
    let (override_texture, sel_stage, this_is_selected) = {
        get_override_tex_if_selected(|tp:&TexPtr| {
            match tp {
                &TexPtr::D3D9(ref tex) => *tex as *mut IDirect3DBaseTexture9,
                x => {
                    write_log_file(&format!("ERROR: unexpected texture type in snapshot selection: {:?}", x));
                    null_mut()
                }
            }
        }).unwrap_or((null_mut(), 0, false))
    };

    if GLOBAL_STATE.is_snapping {
        let mut sd = types::interop::SnapshotData {
            sd_size: std::mem::size_of::<types::interop::SnapshotData>() as u32,
            prim_type: PrimitiveType as i32,
            base_vertex_index: BaseVertexIndex,
            min_vertex_index: MinVertexIndex,
            num_vertices: NumVertices,
            start_index: startIndex,
            prim_count: primCount,
            vert_decl: null_mut(), // filled in by take()
            index_buffer: null_mut(), // filled in by take()
        };
        hook_snapshot::take(THIS, &mut sd, this_is_selected);
    }

    profile_start!(hdip, main_combinator);

    GLOBAL_STATE.in_dip = true;

    use global_state::RenderedPrimType::PrimVertCount;
    if global_state::METRICS_TRACK_PRIMS {
        metrics.rendered_prims.push(PrimVertCount(primCount, NumVertices));
    }

    // if there is a matching mod, render it
    let mod_status = check_and_render_mod(primCount, NumVertices,
        |d3dd,nmod| {
            if let ModD3DData::D3D9(d3dd) = d3dd {
                render_mod_d3d9(THIS, d3dd, nmod,
                    override_texture as *mut IDirect3DBaseTexture9, sel_stage,
                    (primCount,NumVertices))
            } else {
                false
            }
        });

    profile_end!(hdip, main_combinator);

    profile_start!(hdip, draw_input_check);
    // draw input if not modded or if mod is additive
    use types::interop::ModType::GPUAdditive;
    let draw_input = match mod_status {
        CheckRenderModResult::NotRendered => true,
        CheckRenderModResult::NotRenderedButLoadRequested(_) => true,
        CheckRenderModResult::Rendered(mtype) if GPUAdditive as i32 == mtype => true,
        CheckRenderModResult::Rendered(_) => false, // none-additive mod was rendered
        CheckRenderModResult::Deleted => false,
    };
    profile_end!(hdip, draw_input_check);

    profile_start!(hdip, real_dip);
    let dresult = if draw_input {
        let mut save_texture: *mut IDirect3DBaseTexture9 = null_mut();
        let _st_rod = {
            if override_texture != null_mut() {
                (*THIS).GetTexture(sel_stage, &mut save_texture);
                (*THIS).SetTexture(sel_stage, override_texture as *mut IDirect3DBaseTexture9);
                Some(ReleaseOnDrop::new(save_texture))
            } else {
                None
            }
        };
        let r = (hookdevice.real_draw_indexed_primitive)(
            THIS,
            PrimitiveType,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        );
        if override_texture != null_mut() {
            (*THIS).SetTexture(sel_stage, save_texture);
        }
        r
    } else {
        S_OK
    };
    profile_end!(hdip, real_dip);

    metrics.dip_calls += 1;


    GLOBAL_STATE.in_dip = false;
    profile_end!(hdip, hook_dip);

    profile_summarize!(hdip);

    dresult
}


#[cfg(test)]
// these tests require access to test internals which is nightly only
// to enable them, comment out this cfg then uncomment the 'extern crate test' line in lib.rs
#[cfg(nightly)]
mod tests {
    use super::*;

    extern crate test;

    use test::*;

    #[allow(unused)]
    pub unsafe extern "system" fn stub_draw_indexed_primitive(
        THIS: *mut IDirect3DDevice9,
        arg1: D3DPRIMITIVETYPE,
        BaseVertexIndex: INT,
        MinVertexIndex: UINT,
        NumVertices: UINT,
        startIndex: UINT,
        primCount: UINT,
    ) -> HRESULT {
        test::black_box(());
        S_OK
    }

    #[allow(unused)]
    pub unsafe extern "system" fn stub_begin_scene(THIS: *mut IDirect3DDevice9) -> HRESULT {
        test::black_box(());
        S_OK
    }

    #[allow(unused)]
    pub unsafe extern "system" fn stub_release(THIS: *mut IUnknown) -> ULONG {
        test::black_box(());
        0
    }

    #[allow(unused)]
    unsafe extern "system" fn stub_present(
        THIS: *mut IDirect3DDevice9,
        pSourceRect: *const RECT,
        pDestRect: *const RECT,
        hDestWindowOverride: HWND,
        pDirtyRegion: *const RGNDATA,
    ) -> HRESULT {
        test::black_box(());
        0
    }

    fn set_stub_device() {
        // let d3d9device = HookDirect3D9Device::new(
        //     stub_draw_indexed_primitive,
        //     stub_begin_scene,
        //     stub_present,
        //     stub_release);
        // set_hook_device(d3d9device);
    }

    #[test]
    fn can_create_d3d9() {
        //use test_e2e;
        // TODO: need to figure out why this behaves poorly WRT test_e2e.
        // is it a side effect of rust's threaded test framework or a system of issues.

        // let _lock = test_e2e::TEST_MUTEX.lock().unwrap();
        // let d3d9 = create_d3d9(32);
        // if let &Err(ref x) = &d3d9 {
        //     assert!(false, format!("unable to create d39: {:?}", x));
        // }
        // unsafe { d3d9.map(|d3d9| (*d3d9).Release()) };
        // let d3d9 = create_d3d9(32);
        // if let &Err(ref x) = &d3d9 {
        //     assert!(false, format!("unable to create d39: {:?}", x));
        // }
        // unsafe { d3d9.map(|d3d9| (*d3d9).Release()) };
        // println!("=============== exiting");
    }
    #[test]
    fn test_state_copy() {
        //set_stub_device();

        // TODO: re-enable when per-scene ops creates clr in global state
        // unsafe {
        //     let device = std::ptr::null_mut();
        //     hook_begin_scene(device);
        //     for _i in 0..10 {
        //         hook_draw_indexed_primitive(device, D3DPT_TRIANGLESTRIP, 0, 0, 0, 0, 0);
        //     }
        // };
    }

    #[bench]
    fn dip_call_time(_b: &mut Bencher) {
        //set_stub_device();

        // Core-i7-6700 3.4Ghz, 1.25 nightly 2018-01-13
        // 878600000 dip calls in 10.0006051 secs (87854683.91307643 dips/sec)
        // 111,695,214 ns/iter (+/- 2,909,577)
        // ~88K calls/millisecond

        // TODO: re-enable when per-scene ops creates clr in global state

        // let device = std::ptr::null_mut();
        // unsafe { hook_begin_scene(device) };
        // b.iter(|| {
        //     let range = 0..10_000_000;
        //     for _r in range {
        //         unsafe { hook_draw_indexed_primitive(device,
        //             D3DPT_TRIANGLESTRIP, 0, 0, 0, 0, 0) };
        //     }
        // });
    }
}
