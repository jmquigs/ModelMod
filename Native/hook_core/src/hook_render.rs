use device_state::dev_state_d3d11_write;
use global_state::HookState;
use global_state::MAX_STAGE;
use types::d3ddata::ModD3DData9;
use types::interop::D3D9SnapshotRendData;
use types::interop::SnapshotRendData;
use types::native_mod::ModD3DData;
use types::native_mod::NativeModData;
use winapi::um::unknwnbase::IUnknown;

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::wingdi::RGNDATA;
pub use winapi::um::winnt::HRESULT;

use dnclr::{init_clr, reload_managed_dll};

use util;
use mod_load;
use mod_load::AsyncLoadState;
use crate::debug_spam;
use crate::input_commands;
use crate::mod_render;
use mod_stats::mod_stats;
use global_state::{hook_state_read, hook_state_write, LOADED_MODS};
use device_state::{dev_state_read, dev_state_write};
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

fn get_current_texture(gs: &HookState) -> usize {
    let idx = gs.curr_texture_index;
    gs.active_texture_list
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

#[inline]
/// If selection mode is not active, this returns None.  If it is active, and the current texture
/// is selected, returns the texture that should be used to override the current (aka the
/// "selection texture") as well as the stage it should be set on.  For D3D9 this is the actual stage,
/// for D3D11 it is the index into the current pixel shader resource array.  If the current texture,
/// is not selected, returns None.
///
/// Takes `&HookState` so callers on the DIP hot path can pass through the
/// guard they already hold rather than re-acquiring.
pub unsafe fn get_override_tex_if_selected<'a, T, F>(gs: &HookState, extract_ptr:F) -> Option<(*mut T, DWORD, bool)>
where F: FnOnce(&TexPtr) -> *mut T {
    if gs.making_selection {
        get_selected_texture_stage(gs)
            .map(|stage| {
                gs.selection_texture.as_ref()
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
fn get_selected_texture_stage(gs: &HookState) -> Option<DWORD> {
    for i in 0..MAX_STAGE {
        if gs.selected_on_stage[i] {
            return Some(i as DWORD);
        }
    }
    None
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
pub fn process_metrics(preserve_prims:bool, interval:u32) {
    let (_gs_lck, gs) = match hook_state_write() {
        Some(p) => p,
        None => return,
    };
    let metrics = &mut gs.metrics;
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
                    dev_state_d3d11_write().map(|(_lck, state)| {
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
                    gs.active_texture_set.as_ref().map(|set| {
                        if set.len() > 0 {
                            write_log_file(&format!(
                                "active texture set contains: {} textures",
                                set.len()
                            ))
                        }
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
        metrics.rendered_prims.clear();


    } else {
        // not time for update, but clear the prim list unless caller said not to
        if !preserve_prims {
            metrics.rendered_prims.clear();
        }
    }
}

/// Should be called periodically to complete initialization of the .net common language
/// runtime.  In DX9, this is called by `do_per_frame_operations` once per frame.  No-ops if
/// CLR is already loaded.  Should not be cpu-intensive to call this unless the CLR does need to
/// be loaded, in which case its at least a few hundred ms, but it only happens once.
pub fn frame_init_clr(run_context:&'static str) -> Result<()> {
    // Quick read-only check first; CLR init is a one-shot.
    let needs_init = hook_state_read()
        .map(|(_lck, gs)| gs.clr.runtime_pointer.is_none())
        .unwrap_or(false);
    if !needs_init {
        return Ok(());
    }
    // mm_root is needed by both init_clr and reload_managed_dll. Clone it
    // out so we can drop the lock before the (long) CLR init, which itself
    // calls back into us via the OnInitialized callback and would deadlock
    // if we held the write guard.
    let mm_root: Option<String> = hook_state_read()
        .and_then(|(_lck, gs)| gs.mm_root.clone());
    write_log_file("creating CLR");
    let result = init_clr(&mm_root)
        .and_then(|_x| reload_managed_dll(&mm_root, Some(run_context)));
    if let Some((_lck, gs)) = hook_state_write() {
        match result {
            Ok(_) => {
                gs.clr.runtime_pointer = Some(CLR_OK);
                gs.clr.run_context = run_context.to_owned();
            }
            Err(ref e) => {
                write_log_file(&format!("Error creating CLR: {:?}", e));
                gs.clr.runtime_pointer = Some(CLR_FAIL);
            }
        }
    }
    result.map(|_| ())
}

pub fn frame_load_mods(deviceptr: DevicePointer) {
    // Snapshot the bits of interop state we need to make decisions, then
    // drop the read lock so the managed callbacks (which can call back into
    // our hooks) don't deadlock.
    let snap = match hook_state_read() {
        Some((_lck, gs)) => match gs.interop_state.as_ref() {
            Some(is) => Some((
                is.loading_mods,
                is.done_loading_mods,
                is.conf_data.LoadModsOnStart,
                is.callbacks,
            )),
            None => None,
        },
        None => None,
    };
    let (mut loading_mods, mut done_loading_mods, load_on_start, callbacks) = match snap {
        Some(s) => s,
        None => return,
    };

    if !loading_mods && !done_loading_mods && load_on_start {
        let loadstate = unsafe { (callbacks.GetLoadingState)() };
        if loadstate == AsyncLoadState::InProgress as i32 {
            loading_mods = true;
            done_loading_mods = false;
        } else if loadstate != AsyncLoadState::Pending as i32 {
            let r = unsafe { (callbacks.LoadModDB)() };
            if r == AsyncLoadState::Pending as i32 {
                loading_mods = true;
                done_loading_mods = false;
            }
            if r == AsyncLoadState::Complete as i32 {
                loading_mods = false;
                done_loading_mods = true;
            }
            write_log_file(&format!("mod db load returned: {}", r));
        }
    }

    if loading_mods
        && unsafe { (callbacks.GetLoadingState)() } == AsyncLoadState::Complete as i32
    {
        write_log_file("mod loading complete");
        loading_mods = false;
        done_loading_mods = true;

        match deviceptr {
            DevicePointer::D3D11(_)
            | DevicePointer::D3D9(_) =>
                unsafe { mod_load::setup_mod_data(deviceptr, callbacks) },
        }
    }

    // Persist any state changes back into the interop state.
    if let Some((_lck, gs)) = hook_state_write() {
        if let Some(is) = gs.interop_state.as_mut() {
            is.loading_mods = loading_mods;
            is.done_loading_mods = done_loading_mods;
        }
    }

    let has_pending_mods = hook_state_read()
        .and_then(|(_lck, gs)| gs.load_on_next_frame.as_ref().map(|hs| !hs.is_empty()))
        .unwrap_or(false);

    if has_pending_mods && done_loading_mods && !loading_mods {
        match deviceptr {
            DevicePointer::D3D11(_)
            | DevicePointer::D3D9(_) =>
                unsafe { mod_load::load_deferred_mods(deviceptr, callbacks) },
        }
    }
}
pub fn do_per_frame_operations(device: *mut IDirect3DDevice9) -> Result<()> {
    // write_log_file(&format!("performing per-scene ops on thread {:?}",
    //         std::thread::current().id()));

    frame_init_clr(dnclr::RUN_CONTEXT_D3D9)?;
    frame_load_mods(DevicePointer::D3D9(device));

    const METRICS_DIPS_INTERVAL:u32 = 1_000_000;
    process_metrics(false, METRICS_DIPS_INTERVAL);

    mod_stats::update(&SystemTime::now());

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
        let curr = get_current_texture(global_state);
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
    // Skip selection tracking when the call comes from inside our own DIP
    // path (e.g. render_mod_d3d9 setting mod/override textures). In that
    // case the DIP guard holds the hook-state lock and we'd deadlock; the
    // synthetic SetTexture is also not a "natural" game binding, so we
    // don't want to track it as selected anyway.
    if !global_state::IN_DIP.load(std::sync::atomic::Ordering::Acquire) {
        if let Some((_lck, gs)) = hook_state_write() {
            if gs.making_selection {
                track_set_texture(pTexture as usize, Stage, gs);
            }
        }
    }

    // Pull the real fn pointer out and drop the guard before calling, in case
    // it re-enters our hooks on the same thread.
    let real_set_texture = match dev_state_read() {
        Some((_lck, ds)) => match &ds.hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                dev.real_set_texture
            },
            _ => return E_FAIL,
        },
        None => return E_FAIL,
    };
    real_set_texture(THIS, Stage, pTexture)
}

/// Record the pointer of the vertex buffer bound to slot `slot`. We only
/// care about stream 0 for secondary mesh identification; bindings on other
/// slots are ignored here.
pub fn track_bound_vertex_buffer(vb_as_int: usize, slot: u32, global_state: &mut HookState) {
    if slot == 0 {
        global_state.bound_vertex_buffer = vb_as_int;
    }
}

pub (crate) unsafe extern "system" fn hook_set_stream_source(
    THIS: *mut IDirect3DDevice9,
    StreamNumber: UINT,
    pStreamData: *mut IDirect3DVertexBuffer9,
    OffsetInBytes: UINT,
    Stride: UINT,
) -> HRESULT {
    // Same re-entry skip as in hook_set_texture: render_mod_d3d9 invokes
    // SetStreamSource as part of mod rendering while the DIP guard holds
    // the hook-state lock.
    if !global_state::IN_DIP.load(std::sync::atomic::Ordering::Acquire) {
        if let Some((_lck, gs)) = hook_state_write() {
            track_bound_vertex_buffer(pStreamData as usize, StreamNumber, gs);
        }
    }

    let real_set_stream_source = match dev_state_read() {
        Some((_lck, ds)) => match &ds.hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                dev.real_set_stream_source
            },
            _ => return E_FAIL,
        },
        None => return E_FAIL,
    };
    real_set_stream_source(THIS, StreamNumber, pStreamData, OffsetInBytes, Stride)
}

/// Hook for IDirect3DDevice9::Reset.
///
/// Clean up any state that may be invalidated by device reset.
pub (crate) unsafe extern "system" fn hook_reset(
    THIS: *mut IDirect3DDevice9,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
) -> HRESULT {
    if let Some((_lck, gs)) = hook_state_write() {
        gs.bound_vertex_buffer = 0;
        if let Some(map) = gs.vb_checksums.as_mut() {
            map.clear();
        }
    }

    let real_reset = match dev_state_read() {
        Some((_lck, ds)) => match &ds.hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                dev.real_reset
            },
            _ => return E_FAIL,
        },
        None => return E_FAIL,
    };
    real_reset(THIS, pPresentationParameters)
}

// TODO: hook this up to device release at the proper time
unsafe fn purge_device_resources(device: DevicePointer) {
    if device.is_null() {
        write_log_file("WARNING: ignoring insane attempt to purge devices on a null device");
        return;
    }
    mod_load::clear_loaded_mods(device);
    let seltext = hook_state_write().and_then(|(_lck, gs)| gs.selection_texture.take());
    seltext.map(|t| t.release());

    if let Some((_lck, gs)) = hook_state_write() {
        gs.input.as_mut().map(|input| input.clear_handlers());
    }
    if let Some((_lck, ds)) = dev_state_write() {
        ds.d3d_resource_count = 0;
    }
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
        // Pull the real fn pointer out under a read guard, drop the guard,
        // then call it (the call may re-enter our hooks on this thread).
        let real_present = match dev_state_read() {
            Some((_lck, ds)) => match &ds.hook {
                Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                    dev.real_present
                },
                _ => return E_FAIL,
            },
            None => return E_FAIL,
        };
        real_present(THIS, pSourceRect, pDestRect, hDestWindowOverride, pDirtyRegion)
    };
    if global_state::in_any_hook_fn() {
        return call_real_present();
    }

    if let Err(e) = do_per_frame_operations(THIS) {
        write_log_file(&format!(
            "unexpected error from do_per_scene_operations: {:?}",
            e
        ));
        return call_real_present()
    }

    // Pull a snapshot of the bits of HookState we need for this present
    // pass under a short read lock.
    let (snap_use_sysmem, min_fps, has_seltex, is_snapping) = match hook_state_read() {
        Some((_lck, gs)) => (
            gs.run_conf.profile.snap_use_sysmemtexturetracking,
            gs.interop_state.map(|is| is.conf_data.MinimumFPS).unwrap_or(0) as f64,
            gs.selection_texture.is_some(),
            gs.is_snapping,
        ),
        None => (false, 0.0, false, false),
    };

    // Periodically GC the DX9 UpdateTexture source-tracking state.
    if snap_use_sysmem {
        let now = SystemTime::now();
        let last_gc = device_state::dev_state_d3d9_read()
            .map(|(_lck, h)| h.update_texture_last_gc);
        let due = match last_gc {
            Some(t) => now.duration_since(t).map(|d| d.as_secs() >= 30).unwrap_or(false),
            None => false,
        };
        if due {
            crate::hook_device::dx9_update_texture_gc();
            if let Some((_lck, h)) = device_state::dev_state_d3d9_write() {
                h.update_texture_last_gc = now;
            }
        }
    }

    let has_hook = match dev_state_read() {
        Some((_lck, ds)) => ds.hook.is_some(),
        None => false,
    };
    let present_ret = if !has_hook { S_OK } else {
        // Update frame metrics under a write guard, then drop it before
        // calling into the real present.
        if let Some((_lck, gs)) = hook_state_write() {
            let metrics = &mut gs.metrics;
            metrics.frames += 1;
            metrics.total_frames += 1;
            if metrics.frames % 90 == 0 {
                // enforce min fps
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
                    metrics.last_fps_update = now;
                    metrics.frames = 0;
                }
            }
        }
        call_real_present()
    };

    if !has_seltex {
        input_commands::create_selection_texture_d3d9(THIS);
    }

    let d3d_window = match dev_state_read() {
        Some((_lck, ds)) => ds.d3d_window,
        None => null_mut(),
    };
    if util::appwnd_is_foreground(d3d_window) {
        // Process input under the write guard. Input processing can fire
        // press handlers but those are user code (key bindings) that
        // should not re-enter our hooks.
        if let Some((_lck, gs)) = hook_state_write() {
            if let Some(inp) = gs.input.as_mut() {
                if inp.get_press_fn_count() == 0 {
                    input_commands::setup_input(DevicePointer::D3D9(THIS), inp)
                        .unwrap_or_else(|e| write_log_file(&format!("input setup error: {:?}", e)));
                }
                inp.process()
                    .unwrap_or_else(|e| write_log_file(&format!("input error: {:?}", e)));
            }
        }
    }

    if is_snapping {
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

    // Helper: extract the real_release fn pointer (no guard held on return).
    use shared_dx::defs_dx9::IUnknownReleaseFn as D3D9ReleaseFn;
    let get_real_release = || -> Option<D3D9ReleaseFn> {
        match dev_state_read() {
            Some((_lck, ds)) => match &ds.hook {
                Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                    Some(dev.real_release)
                },
                _ => None,
            },
            None => None,
        }
    };

    let _release_guard = match global_state::ReentryGuard::try_enter(&global_state::IN_HOOK_RELEASE) {
        Some(g) => g,
        None => {
            return match get_real_release() {
                Some(real_release) => real_release(THIS),
                None => {
                    oops_log_release_fail();
                    failret
                }
            };
        }
    };

    let real_release = match get_real_release() {
        Some(f) => f,
        None => {
            oops_log_release_fail();
            return failret;
        }
    };

    let r = (|| -> ULONG {
        // First release.  Call without holding the lock since real_release
        // can re-enter our hooks on the same thread.
        let mut new_ref_count = real_release(THIS);

        // Persist ref_count and read d3d_resource_count under a write guard.
        let (destroying, mut do_final_release) = match dev_state_write() {
            Some((_lck, ds)) => {
                let drc = ds.d3d_resource_count;
                if let Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) = ds.hook.as_mut() {
                    dev.ref_count = new_ref_count;
                }
                let destroying = drc > 0 && new_ref_count == (drc + 1);
                let do_final = destroying || (drc == 0 && new_ref_count == 1);
                (destroying, do_final)
            },
            None => return failret,
        };

        // could just leak everything on device destroy.  but I know that will
        // come back to haunt me.  so make an effort to purge my stuff when the
        // resource count gets to the expected value, this way the device can be
        // properly disposed.
        if destroying {
            let drc = match dev_state_read() {
                Some((_lck, ds)) => ds.d3d_resource_count,
                None => 0,
            };
            // purge my stuff
            write_log_file(&format!(
                "device {:x} refcount is same as internal resource count ({}),
                it is being destroyed: purging resources",
                THIS as usize, drc
            ));
            purge_device_resources(DevicePointer::D3D9(THIS as *mut IDirect3DDevice9));
            // Note, ref_count is wrong now since we bypassed this function
            // during unload (no re-entrancy). However the count on the device
            // should be 1 if I did the math right; the release below will
            // fix the count.  Re-evaluate the final-release condition after
            // purge since d3d_resource_count may have changed.
            if let Some((_lck, ds)) = dev_state_read() {
                let drc = ds.d3d_resource_count;
                do_final_release = destroying || (drc == 0 && new_ref_count == 1);
            }
        }

        if do_final_release {
            // release again to trigger destruction of the device
            new_ref_count = real_release(THIS);
            write_log_file(&format!(
                "device released: {:x}, refcount: {}",
                THIS as usize, new_ref_count
            ));
            if new_ref_count != 0 {
                write_log_file(&format!(
                    "WARNING: unexpected ref count of {} after supposedly final
                    device release, device probably leaked",
                    new_ref_count
                ));
            }
            if let Some((_lck, ds)) = dev_state_write() {
                if let Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) = ds.hook.as_mut() {
                    dev.ref_count = new_ref_count;
                }
            }
        }
        new_ref_count
    })();
    // _release_guard drops here, clearing IN_HOOK_RELEASE.

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
///
/// Acquires the hook-state lock internally for short windows; drops it
/// before invoking `rfunc` (which calls into hooked d3d functions like
/// SetTexture/SetStreamSource that would otherwise deadlock).
pub unsafe fn check_and_render_mod<F>(primCount:u32, NumVertices: u32, mut rfunc:F) -> CheckRenderModResult
where
    F: FnMut(&ModD3DData,&NativeModData) -> bool {

    let mut loading_mod_name = None;
    // Snapshot the bits of HookState we need to call mod_render::select.
    // Pre-resolve the bound VB's checksum so we don't have to clone the
    // whole vb_checksums map across the lock boundary on every draw call.
    let (bound_vb_ptr, bound_vb_checksum, total_frames) = match hook_state_read() {
        Some((_lck, gs)) => {
            let ptr = gs.bound_vertex_buffer;
            let cksum = gs.vb_checksums.as_ref()
                .and_then(|m| m.get(&ptr))
                .and_then(|s| s.checksum());
            (ptr, cksum, gs.metrics.total_frames)
        },
        None => return CheckRenderModResult::NotRendered,
    };
    let bound_vb = mod_render::BoundVB {
        ptr: bound_vb_ptr,
        checksum: bound_vb_checksum,
    };
    let mut loaded_mods_guard = match LOADED_MODS.lock() {
        Ok(g) => g,
        Err(e) => {
            write_log_file(&format!("check_and_render_mod: LOADED_MODS lock poisoned: {}", e));
            return CheckRenderModResult::NotRendered;
        }
    };
    let res = loaded_mods_guard.as_mut()
        .and_then(|mods| {
            profile_start!(hdip, mod_select);

            let r = mod_render::select(mods,
                primCount, NumVertices,
                total_frames,
                &bound_vb);
            profile_end!(hdip, mod_select);
            r
        })
        .and_then(|nmods| {

            let modslice = nmods.as_slice();
            let nmodlen = modslice.len();

            debug_spam!(|| format!("select returned {} mods, first: {}", nmodlen, if nmodlen > 0 {
                &modslice[0].name
            } else {
                "none"
            }));
            for nmod in modslice {
                // early out if mod is a deletion mod
                if nmod.mod_data.numbers.mod_type == types::interop::ModType::Deletion as i32 {
                    return Some(nmod.mod_data.numbers.mod_type);
                }
                // if the mod d3d data isn't loaded, can't render
                match nmod.d3d_data {
                    native_mod::ModD3DState::Loaded(_) => (),
                    // could observe partial if we noted it previously but the deferred load
                    // hasn't happened yet (since it happens less often)
                    native_mod::ModD3DState::Partial(_)
                    | native_mod::ModD3DState::Unloaded => {
                        debug_spam!(|| format!("starting load of requested mod: {}", nmod.name));
                        // tried to render an unloaded mod, make a note that it should be loaded.
                        // Acquire the write lock briefly to update load_on_next_frame.
                        let name = nmod.name.to_owned();
                        if let Some((_lck, gs)) = hook_state_write() {
                            let load_next_hs = gs.load_on_next_frame.get_or_insert_with(
                                || fnv::FnvHashSet::with_capacity_and_hasher(
                                    100,
                                    Default::default(),
                                ));
                            load_next_hs.insert(name.clone());
                        }
                        loading_mod_name = Some(name);
                        return None;
                    }
                };
            }
            // If it returned multiple mods we currently assume they have the same type.  Since all these mods must share the same 
            // ref, it doesn't make sense to have (for instance) two mods where one is a deletion, the other a replacement, 
            // or one a replacement other an addition; the semantics are mostly exclusive.
            let mut first_mod_type = None;
            for nmod in modslice {
                debug_spam!(|| format!("rend mod: {}, loadstate: {:?}, ({} total)", nmod.name, nmod.d3d_data, modslice.len()));
                if let native_mod::ModD3DState::Loaded(ref d3dd) = nmod.d3d_data {
                    debug_spam!(|| format!("rendering loaded mod: {}", nmod.name));
                    let rendered = rfunc(d3dd,nmod);
                    if rendered {
                        debug_spam!(|| format!("{} was rendered", nmod.name));
                        if first_mod_type.is_none() {
                            first_mod_type = Some(nmod.mod_data.numbers.mod_type)
                        }
                    }
                } else {
                    debug_spam!(|| format!("cannot render mod {} because it isn't loaded", nmod.name));
                }
                debug_spam!(|| format!("finish rend: {}", nmod.name));
            }
            debug_spam!(|| format!("done rend loop for {} mods", modslice.len() ));
            first_mod_type
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
    let _dip_guard = match global_state::ReentryGuard::try_enter(&global_state::IN_DIP) {
        Some(g) => g,
        None => {
            write_log_file(&format!("ERROR: i'm in DIP already!"));
            return S_OK;
        }
    };
    profile_end!(hdip, dip_check);

    profile_start!(hdip, state_begin);

    let real_dip = match dev_state_read() {
        Some((_lck, ds)) => match &ds.hook {
            Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(dev), .. })) => {
                dev.real_draw_indexed_primitive
            },
            _ => {
                write_log_file(&format!("DIP: No d3d9 device found"));
                return E_FAIL;
            }
        },
        None => {
            write_log_file(&format!("DIP: No d3d9 device found"));
            return E_FAIL;
        }
    };
    profile_end!(hdip, state_begin);

    // Snapshot the hot-path scalars under a brief read lock.
    let (is_snapping, low_framerate, show_mods, bound_vb_ptr, vb_checksum_match, making_selection) = match hook_state_read() {
        Some((_lck, gs)) => (
            gs.is_snapping,
            gs.metrics.low_framerate,
            gs.show_mods,
            gs.bound_vertex_buffer,
            global_state::vb_checksum_target_matches_with(gs, primCount, NumVertices),
            gs.making_selection,
        ),
        None => return E_FAIL,
    };

    if !is_snapping && (low_framerate || !show_mods || force_modding_off) {
        return (real_dip)(
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
    let (override_texture, sel_stage, this_is_selected) = if making_selection {
        match hook_state_read() {
            Some((_lck, gs)) => get_override_tex_if_selected(gs, |tp:&TexPtr| {
                match tp {
                    &TexPtr::D3D9(ref tex) => *tex as *mut IDirect3DBaseTexture9,
                    x => {
                        write_log_file(&format!("ERROR: unexpected texture type in snapshot selection: {:?}", x));
                        null_mut()
                    }
                }
            }).unwrap_or((null_mut(), 0, false)),
            None => (null_mut(), 0, false),
        }
    } else {
        (null_mut(), 0, false)
    };

    // Compute a CRC for the currently bound VB if we haven't already, but
    // only when we actually need it: during a snapshot (so the CRC is
    // available to the snapshot meta) or when a loaded mod has a
    // VB-checksum constraint for this draw's (prim,vert) counts.
    if bound_vb_ptr != 0 && (is_snapping || vb_checksum_match) {
        if let Some((_lck, gs)) = hook_state_write() {
            crate::hook_device::ensure_vb_checksum_dx9(
                gs,
                bound_vb_ptr as *mut IDirect3DVertexBuffer9,
            );
        }
    }

    if is_snapping {
        let mut sd = types::interop::SnapshotData {
            sd_size: std::mem::size_of::<types::interop::SnapshotData>() as u32,
            was_reset: false,
            clear_sd_on_reset: false,
            prim_type: PrimitiveType as i32,
            base_vertex_index: BaseVertexIndex,
            min_vertex_index: MinVertexIndex,
            num_vertices: NumVertices,
            start_index: startIndex,
            prim_count: primCount,
            rend_data: SnapshotRendData {
                // this value is overwritten by hook_snapshot::take()
                d3d9: D3D9SnapshotRendData::new()
            },
        };
        let mut dp = DevicePointer::D3D9(THIS);
        hook_snapshot::take(&mut dp, &mut sd, this_is_selected);
    }

    profile_start!(hdip, main_combinator);

    use global_state::RenderedPrimType::PrimVertCount;
    if global_state::METRICS_TRACK_PRIMS {
        if let Some((_lck, gs)) = hook_state_write() {
            gs.metrics.rendered_prims.push(PrimVertCount(primCount, NumVertices));
        }
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
        let r = (real_dip)(
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

    if let Some((_lck, gs)) = hook_state_write() {
        gs.metrics.dip_calls += 1;
    }

    // _dip_guard drops here, clearing IN_DIP.
    profile_end!(hdip, hook_dip);

    profile_summarize!(hdip, 10.0);

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
