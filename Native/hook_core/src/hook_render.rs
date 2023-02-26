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

fn get_current_texture() -> *mut IDirect3DBaseTexture9 {
    unsafe {
        let idx = GLOBAL_STATE.curr_texture_index;
        GLOBAL_STATE
            .active_texture_list
            .as_ref()
            .map(|list| {
                if idx >= list.len() {
                    null_mut()
                } else {
                    list[idx]
                }
            })
            .unwrap_or(null_mut())
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

pub fn do_per_frame_operations(device: *mut IDirect3DDevice9) -> Result<()> {
    // init the clr if needed
    {
        let hookstate = unsafe { &mut GLOBAL_STATE };
        if hookstate.clr_pointer.is_none() {
            let lock = GLOBAL_STATE_LOCK.lock();
            match lock {
                Ok(_ignored) => {
                    if hookstate.clr_pointer.is_none() {
                        // store something in clr_pointer even if it create fails,
                        // so that we don't keep trying to create it.  clr_pointer is
                        // really just a bool right now, it remains to be
                        // seen whether storing anything related to clr in
                        // global state is actually useful.
                        write_log_file("creating CLR");
                        init_clr(&hookstate.mm_root)
                            .and_then(|_x| {
                                reload_managed_dll(&hookstate.mm_root)
                            })
                            .and_then(|_x| {
                                hookstate.clr_pointer = Some(CLR_OK);
                                Ok(_x)
                            })
                            .map_err(|e| {
                                write_log_file(&format!("Error creating CLR: {:?}", e));
                                hookstate.clr_pointer = Some(CLR_FAIL);
                                e
                            })?;
                    }
                }
                Err(e) => write_log_file(&format!("{:?} should never happen", e)),
            };
        }
    }
    // write_log_file(&format!("performing per-scene ops on thread {:?}",
    //         std::thread::current().id()));

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

            unsafe { mod_load::setup_mod_data(device, is.callbacks) };
        }

        let has_pending_mods =
            unsafe {&GLOBAL_STATE}.load_on_next_frame
                .as_ref().map_or(false, |hs| hs.len() > 0);

        if has_pending_mods && is.done_loading_mods && !is.loading_mods {
            unsafe { mod_load::load_deferred_mods(device, is.callbacks) };
        }
    });

    let metrics = &mut unsafe {&mut GLOBAL_STATE}.metrics;

    if metrics.dip_calls > 1_000_000 {
        let now = SystemTime::now();
        let elapsed = now.duration_since(metrics.last_call_log);
        let mut wrote_dip_stats = false;
        match elapsed {
            Ok(d) => {
                let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                if secs >= 3.0 {
                    let dipsec = metrics.dip_calls as f64 / secs;

                    let epocht = now
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_secs(1))
                        .as_secs()
                        * 1000;

                    wrote_dip_stats = true;
                    write_log_file(&format!(
                        "{:?}: {} dip calls in {:.*} secs ({:.*} dips/sec (fps: {:.*}))",
                        epocht, metrics.dip_calls, 2, secs, 2, dipsec, 2, metrics.last_fps
                    ));
                    unsafe {&mut GLOBAL_STATE}.active_texture_set.as_ref().map(|set| {
                        write_log_file(&format!(
                            "active texture set contains: {} textures",
                            set.len()
                        ))
                    });
                    metrics.last_call_log = now;
                }
            }
            Err(e) => write_log_file(&format!("Error getting elapsed duration: {:?}", e)),
        }
        metrics.dip_calls = 0;

        // dump out the prim list every so often if we are tracking that.
        // note this only dumps out the primitives for the most recent frame.
        // also only write these out when we also just wrote a dip summary line
        // above.
        if global_state::METRICS_TRACK_PRIMS && wrote_dip_stats {
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
                            for (prim,vert) in &metrics.rendered_prims {
                                //writeln!(res_combined, "{},{}\r", prim, vert);
                                // PERF: ugh, a lot of little allocations here...
                                res_combined.push_str(&format!("{},{}\r", prim, vert));
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
    }
    unsafe {&mut GLOBAL_STATE}.metrics.rendered_prims.clear();

    Ok(())
}

pub (crate) unsafe extern "system" fn hook_set_texture(
    THIS: *mut IDirect3DDevice9,
    Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,
) -> HRESULT {
    let has_it = GLOBAL_STATE
        .active_texture_set
        .as_ref()
        .map(|set| set.contains(&pTexture))
        .unwrap_or(true);
    if !has_it {
        GLOBAL_STATE.active_texture_set.as_mut().map(|set| {
            set.insert(pTexture);
        });
        GLOBAL_STATE.active_texture_list.as_mut().map(|list| {
            list.push(pTexture);
        });
    }

    if Stage < MAX_STAGE as u32 {
        let curr = get_current_texture();
        if curr != null_mut() && pTexture == curr {
            GLOBAL_STATE.selected_on_stage[Stage as usize] = true;
        } else if GLOBAL_STATE.selected_on_stage[Stage as usize] {
            GLOBAL_STATE.selected_on_stage[Stage as usize] = false;
        }
    }

    match (dev_state()).hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
            (dev.real_set_texture)(THIS, Stage, pTexture)
        },
        _ => E_FAIL
    }
}

// TODO: hook this up to device release at the proper time
unsafe fn purge_device_resources(device: *mut IDirect3DDevice9) {
    if device == null_mut() {
        write_log_file("WARNING: ignoring insane attempt to purge devices on a null device");
        return;
    }
    mod_load::clear_loaded_mods(device);
    if GLOBAL_STATE.selection_texture != null_mut() {
        (*GLOBAL_STATE.selection_texture).Release();
        GLOBAL_STATE.selection_texture = null_mut();
    }
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

    if GLOBAL_STATE.selection_texture == null_mut() {
        input_commands::create_selection_texture(THIS);
    }

    if util::appwnd_is_foreground(dev_state().d3d_window) {
        GLOBAL_STATE.input.as_mut().map(|inp| {
            if inp.get_press_fn_count() == 0 {
                input_commands::setup_input(THIS, inp)
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
            //         THIS as u64, hookdevice.ref_count
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
                    THIS as u64, dev_state().d3d_resource_count
                ));
                purge_device_resources(THIS as *mut IDirect3DDevice9);
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
                    THIS as u64, hookdevice.ref_count
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

    // snapshotting
    let (override_texture, sel_stage, this_is_selected) = {
        let default = (null_mut(), 0, false);
        if GLOBAL_STATE.making_selection {
            get_selected_texture_stage()
                .map(|stage| {
                    (
                        std::mem::transmute(GLOBAL_STATE.selection_texture),
                        stage,
                        true,
                    )
                })
                .unwrap_or(default)
        } else {
            default
        }
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
    profile_start!(hdip, mod_key_prep);

    GLOBAL_STATE.in_dip = true;

    let mut drew_mod = false;

    if global_state::METRICS_TRACK_PRIMS {
        metrics.rendered_prims.push((primCount, NumVertices));
    }

    // if there is a matching mod, render it
    let modded =
        GLOBAL_STATE.loaded_mods.as_mut()
        .and_then(|mods| {
            profile_end!(hdip, mod_key_prep);
            profile_start!(hdip, mod_select);

            let r = mod_render::select(mods,
                primCount, NumVertices,
                GLOBAL_STATE.metrics.total_frames);
            profile_end!(hdip, mod_select);
            r
        })
        .and_then(|nmod| {
            if nmod.mod_data.numbers.mod_type == types::interop::ModType::Deletion as i32 {
                return Some(nmod.mod_data.numbers.mod_type);
            }
            // if the mod d3d data isn't loaded, can't render
            let d3dd = match nmod.d3d_data {
                native_mod::ModD3DState::Loaded(ref d3dd) => d3dd,
                native_mod::ModD3DState::Unloaded => {
                    // tried to render an unloaded mod, make a note that it should be loaded
                    let load_next_hs = GLOBAL_STATE.load_on_next_frame.get_or_insert_with(
                        || fnv::FnvHashSet::with_capacity_and_hasher(
                            100,
                            Default::default(),
                        ));
                    load_next_hs.insert(nmod.name.to_owned());
                    return None;
                }
            };

            profile_start!(hdip, mod_render);
            // save state
            let mut pDecl: *mut IDirect3DVertexDeclaration9 = null_mut();
            let ppDecl: *mut *mut IDirect3DVertexDeclaration9 = &mut pDecl;
            let hr = (*THIS).GetVertexDeclaration(ppDecl);
            if hr != 0 {
                write_log_file(&format!(
                    "failed to save vertex declaration when trying to render mod {} {}",
                    NumVertices, primCount
                ));
                return None;
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
                return None;
            }

            // Note: C++ code did not change StreamSourceFreq...may need it for some games.
            // draw override
            (*THIS).SetVertexDeclaration(d3dd.decl);
            (*THIS).SetStreamSource(0, d3dd.vb, 0, nmod.mod_data.numbers.vert_size_bytes as u32);

            // and set mod textures
            let mut save_tex:[Option<*mut IDirect3DBaseTexture9>; 4] = [None; 4];
            let mut _st_rods:Vec<ReleaseOnDrop<*mut IDirect3DBaseTexture9>> = vec![];
            for (i,tex) in d3dd.textures.iter().enumerate() {
                if *tex != null_mut() {
                    //write_log_file(&format!("set override tex stage {} to {:x} for mod {}/{}", i, *tex as u64, NumVertices, primCount));
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
                    (*THIS).GetTexture(sel_stage, &mut save_texture);
                    (*THIS).SetTexture(sel_stage, override_texture);
                    Some(ReleaseOnDrop::new(save_texture))
                } else {
                    None
                }
            };

            (*THIS).DrawPrimitive(
                nmod.mod_data.numbers.prim_type as u32,
                0,
                nmod.mod_data.numbers.prim_count as u32,
            );
            drew_mod = true;

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
                (*THIS).SetTexture(sel_stage, save_texture);
            }
            (*pDecl).Release();
            (*pStreamVB).Release();
            profile_end!(hdip, mod_render);

            Some(nmod.mod_data.numbers.mod_type)
        });
    profile_end!(hdip, main_combinator);

    profile_start!(hdip, draw_input_check);
    // draw input if not modded or if mod is additive
    let draw_input = match modded {
        None => true,
        Some(mtype) if types::interop::ModType::GPUAdditive as i32 == mtype => true,
        Some(_) => false,
    };
    profile_end!(hdip, draw_input_check);

    profile_start!(hdip, real_dip);
    let dresult = if draw_input {
        let mut save_texture: *mut IDirect3DBaseTexture9 = null_mut();
        let _st_rod = {
            if override_texture != null_mut() {
                (*THIS).GetTexture(sel_stage, &mut save_texture);
                (*THIS).SetTexture(sel_stage, override_texture);
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
