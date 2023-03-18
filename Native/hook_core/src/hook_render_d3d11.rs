use std::ptr::null_mut;
use std::time::SystemTime;

use global_state::{GLOBAL_STATE, METRICS_TRACK_MOD_PRIMS, HWND};
use shared_dx::dx11rs::DX11RenderState;
use shared_dx::types::{HookDeviceState, DevicePointer, DX11Metrics};
use shared_dx::types_dx11::{HookDirect3D11Context};
use shared_dx::util::{write_log_file};
use types::d3ddata::ModD3DData11;
use types::native_mod::{ModD3DData, ModD3DState, NativeModData};
use winapi::ctypes::c_void;
use winapi::shared::dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN};
use winapi::shared::winerror::{E_NOINTERFACE};
use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout, D3D11_PRIMITIVE_TOPOLOGY, ID3D11ShaderResourceView, D3D11_SHADER_RESOURCE_VIEW_DESC};
use winapi::shared::ntdef::ULONG;
use winapi::um::d3dcommon::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D11_SRV_DIMENSION_TEXTURE2D};
use winapi::um::processthreadsapi::GetCurrentProcessId;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winuser::{EnumWindows, GetWindowThreadProcessId, GetParent, GetDesktopWindow, GetForegroundWindow};
use winapi::um::{d3d11::ID3D11DeviceContext, winnt::INT};
use winapi::shared::minwindef::UINT;
use device_state::{dev_state, dev_state_d3d11_nolock};
use shared_dx::error::Result;
use crate::hook_render::{process_metrics, frame_init_clr, frame_load_mods, check_and_render_mod, CheckRenderModResult};
use crate::{input_commands, debugmode};
use winapi::um::d3d11::D3D11_BUFFER_DESC;
use crate::debugmode::DebugModeCalledFns;

/// Return the d3d11 context hooks.
fn get_hook_context<'a>() -> Result<&'a mut HookDirect3D11Context> {
    let hooks = match dev_state().hook {
        Some(HookDeviceState::D3D11(ref mut rs)) => &mut rs.hooks,
        _ => {
            write_log_file("draw: No d3d11 context found");
            return Err(shared_dx::error::HookError::D3D11NoContext);
        },
    };
    Ok(&mut hooks.context)
}

pub fn u8_slice_to_hex_string(slice: &[u8]) -> String {
    let mut s = String::new();
    for b in slice {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub unsafe extern "system" fn hook_context_QueryInterface(
    THIS: *mut IUnknown,
    riid: *const winapi::shared::guiddef::GUID,
    ppvObject: *mut *mut winapi::ctypes::c_void,
) -> winapi::shared::winerror::HRESULT {
    write_log_file(&format!("Context: hook_context_QueryInterface: for id {:x} {:x} {:x} {}",
        (*riid).Data1, (*riid).Data2, (*riid).Data3, u8_slice_to_hex_string(&(*riid).Data4)));

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return E_NOINTERFACE,
    };

    let hr = (hook_context.real_query_interface)(THIS, riid, ppvObject);
    write_log_file(&format!("Context: hook_context_QueryInterface: hr {:x}", hr));
    hr
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
    debugmode::note_called(DebugModeCalledFns::Hook_ContextRelease);

    // see note in d3d9 hook_release as to why this is needed, but it "should never happen".
    let failret:ULONG = 0xFFFFFFFF;
    let oops_log_release_fail = || {
        write_log_file(&format!("OOPS hook_release returning {} due to bad state", failret));
    };

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => {
            oops_log_release_fail();
            return failret;
        }
    };

    if GLOBAL_STATE.in_hook_release {
        //write_log_file(&format!("warn: re-entrant hook release"));
        return (hook_context.real_release)(THIS);
    }
    GLOBAL_STATE.in_hook_release = true;
    let rc = (hook_context.real_release)(THIS);
    // if >= 1 then this spams when Discord is running, wonder what its doing
    if rc < 1 {
        write_log_file(&format!("hook release: rc now {}", rc));
    }
    GLOBAL_STATE.in_hook_release = false;

    rc
}

// this was only needed when I was "rehooking" constantly (before I switched to copying
// the vtable).  so its disabled now.  its a fairly hot function so better not to be doing
// stuff here if I can avoid it.
// pub unsafe extern "system" fn hook_VSSetConstantBuffers(
//     THIS: *mut ID3D11DeviceContext,
//     StartSlot: UINT,
//     NumBuffers: UINT,
//     ppConstantBuffers: *const *mut ID3D11Buffer,
// ) {
//     debugmode::note_called(DebugModeCalledFns::Hook_ContextVSSetConstantBuffers);

//     let hook_context = match get_hook_context() {
//         Ok(ctx) => ctx,
//         Err(_) => return,
//     };

//     // TODO11: probably need to get more zealous about locking around this as DX11 and later
//     // games are more likely to use multihreaded rendering, though hopefully i'll just never use
//     // MM with one of those :|
//     // But these metrics should just be thread local

//     // let was_hooked = if debugmode::rehook_enabled(DebugModeCalledFns::Hook_ContextVSSetConstantBuffers) {
//     //     let func_hooked = apply_context_hooks(THIS, false);
//     //     match func_hooked {
//     //         Ok(n) if n > 0 => true,
//     //         Ok(_) => false,
//     //         _ => {
//     //             write_log_file("error late hooking");
//     //             false
//     //         }
//     //     }
//     // } else {
//     //     false
//     // };

//     // match dev_state_d3d11_nolock() {
//     //     Some(state) => {
//     //         state.metrics.vs_set_const_buffers_calls += 1;
//     //         if was_hooked {
//     //             state.metrics.vs_set_const_buffers_hooks += 1;
//     //         }
//     //     },
//     //     None => {}
//     // };

//     (hook_context.real_vs_setconstantbuffers)(
//         THIS,
//         StartSlot,
//         NumBuffers,
//         ppConstantBuffers
//     )
// }

pub unsafe extern "system" fn hook_IASetPrimitiveTopology (
    THIS: *mut ID3D11DeviceContext,
    Topology: D3D11_PRIMITIVE_TOPOLOGY,
) {
    debugmode::note_called(DebugModeCalledFns::Hook_ContextIASetPrimitiveTopology);

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // if debugmode::rehook_enabled(DebugModeCalledFns::Hook_ContextIASetPrimitiveTopology) {
    //     // rehook to reduce flickering
    //     let _func_hooked = apply_context_hooks(THIS, false);
    // }

    match dev_state_d3d11_nolock() {
        Some(state) => {
            state.rs.prim_topology = Topology;
        },
        None => {}
    };

    (hook_context.real_ia_set_primitive_topology)(THIS, Topology);
}
pub unsafe extern "system" fn hook_IASetVertexBuffers(
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumBuffers: UINT,
    ppVertexBuffers: *const *mut ID3D11Buffer,
    pStrides: *const UINT,
    pOffsets: *const UINT,
) {
    debugmode::note_called(DebugModeCalledFns::Hook_ContextIASetVertexBuffers);

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // if debugmode::rehook_enabled(DebugModeCalledFns::Hook_ContextIASetVertexBuffers) {
    //     // rehook to reduce flickering
    //     let _func_hooked = apply_context_hooks(THIS, false);
    // }

    // TODO11 use the lock function here or switch to thread local for RS
    let state = dev_state_d3d11_nolock();
    match state {
        Some(state) => {
            if NumBuffers > 0 && ppVertexBuffers != null_mut() {
                for idx in 0..NumBuffers {
                    let pbuf = (*ppVertexBuffers).offset(idx as isize);

                    if pbuf != null_mut() {
                        // clear on first add of a valid buffer, the game appears to be calling this
                        // with 1 null buffer sometimes (and then calling draw) and I don't know why its
                        // doing that.
                        if idx == 0 {
                            state.rs.vb_state.clear();
                        }
                        let mut desc:D3D11_BUFFER_DESC = std::mem::zeroed();
                        (*pbuf).GetDesc(&mut desc);
                        let bw = desc.ByteWidth;
                        let stride = desc.StructureByteStride;
                        let vbinfo = (idx,bw,stride);
                        state.rs.vb_state.push(vbinfo);
                    }
                }
                // if GLOBAL_STATE.metrics.dip_calls % 10000 == 0 {
                //     write_log_file(&format!("hook_IASetVertexBuffers: {}, added {}", NumBuffers, GLOBAL_STATE.dx11rs.vb_state.len()));
                // }
            } else if NumBuffers == 0 {
                state.rs.vb_state.clear();
            }
        },
        None => {}
    };

    (hook_context.real_ia_set_vertex_buffers)(
        THIS,
        StartSlot,
        NumBuffers,
        ppVertexBuffers,
        pStrides,
        pOffsets,
    )
}

pub unsafe extern "system" fn hook_IASetInputLayout(
    THIS: *mut ID3D11DeviceContext,
    pInputLayout: *mut ID3D11InputLayout,
) {
    debugmode::note_called(DebugModeCalledFns::Hook_ContextIASetInputLayout);

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // if debugmode::rehook_enabled(DebugModeCalledFns::Hook_ContextIASetInputLayout) {
    //     // rehook to reduce flickering
    //     let _func_hooked = apply_context_hooks(THIS, false);
    // }

    // TODO11 use the lock function here or switch to thread local for RS
    dev_state_d3d11_nolock().map(|state| {
        if pInputLayout != null_mut() {
            state.rs.current_input_layout = pInputLayout;
        } else {
            state.rs.current_input_layout = null_mut();
        }
    });

    (hook_context.real_ia_set_input_layout)(
        THIS,
        pInputLayout
    )
}

fn compute_prim_vert_count(index_count: UINT, rs:&DX11RenderState) -> Option<(u32,u32)> {
    if index_count <= 6 { // = 2 triangles generally, mods can't be this small or even close to this small
        // don't bother
        return None;
    }
    // assumes triangle list, actual topology is in render state but we shouldn't even be in
    // here if its not triangle list.
    let prim_count = index_count / 3;

    // vert count has to be computed from the current vertex buffer
    // stream and the current input layout (vertex size)
    let vb_state = &rs.vb_state;
    let vb_size = match vb_state.len() {
        1 => {
            let (_index,byteWidth,_stride) = vb_state[0];
            if byteWidth == 0 {
                write_log_file("compute_prim_vert_count: current vb has zero byte size");
                return None;
            }
            byteWidth
        },
        // TODO11: log warning but it could be spammy, maybe throttle it
        0 => {
            write_log_file("compute_prim_vert_count: no current vertex buffer set");
            return None;
        },
        _n => {
            // not sure how to figure out which one to use, maybe log warning
            return None;
        }
    };
    let curr_input_layout = &rs.current_input_layout;
    let curr_layouts = &rs.input_layouts_by_ptr;
    let vert_size = {
        let curr_input_layout = *curr_input_layout as usize;
        if curr_input_layout > 0 {
            curr_layouts.get(&curr_input_layout).map(|vf| vf.size)
            .unwrap_or(0)
        } else {
            0
        }
    };
    if vert_size == 0 {
        return None;
    }

    let vert_count = if vert_size > 0 {
        vb_size / vert_size
    } else {
        0
    };

    Some((prim_count,vert_count))
}

fn update_drawn_recently(metrics:&mut DX11Metrics, prim_count:u32, vert_count: u32, checkres:&CheckRenderModResult) {
    if METRICS_TRACK_MOD_PRIMS {
        use shared_dx::types::MetricsDrawStatus::*;
        match checkres {
            CheckRenderModResult::NotRendered => {},
            CheckRenderModResult::Rendered(mtype) => {
                metrics.drawn_recently
                .entry((prim_count,vert_count))
                .and_modify(|ds| ds.incr_count())
                .or_insert(Referenced(*mtype,1));
            }
            ,
            CheckRenderModResult::Deleted => {
                metrics.drawn_recently
                .entry((prim_count,vert_count))
                .and_modify(|ds| ds.incr_count())
                .or_insert(Referenced(types::interop::ModType::Deletion as i32,1));
            },
            CheckRenderModResult::NotRenderedButLoadRequested(ref name) => {
                metrics.drawn_recently
                .entry((prim_count,vert_count))
                .and_modify(|ds| ds.incr_count())
                .or_insert(LoadReq(name.clone(),1));
            },
        }
    }
}

pub unsafe extern "system" fn hook_draw_indexed(
    THIS: *mut ID3D11DeviceContext,
    IndexCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
) {
    if GLOBAL_STATE.in_dip {
        write_log_file("ERROR: i'm in DIP already!");
        return;
    }
    debugmode::note_called(DebugModeCalledFns::Hook_ContextDrawIndexed);

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };
    GLOBAL_STATE.in_dip = true;

    GLOBAL_STATE.metrics.dip_calls += 1;

    // TODO11 use the lock function here or switch to thread local for RS
    let state = dev_state_d3d11_nolock();
    let draw_input = state.map(|state| {
        // this is the only prim type I support but don't log if it is something else since
        // it would be spammy (maybe log if trying to take a snapshot)
        if state.rs.prim_topology != D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST {
            return true;
        }
        match compute_prim_vert_count(IndexCount, &state.rs) {
            Some((prim_count,vert_count)) if vert_count > 2  => {
                // if primitive tracking is enabled, log just the primcount,vertcount if we were able
                // to compute it, otherwise log whatever we have
                if global_state::METRICS_TRACK_PRIMS && prim_count > 2 { // filter out some spammy useless stuff
                    if vert_count > 0 {
                        use global_state::RenderedPrimType::PrimVertCount;
                        GLOBAL_STATE.metrics.rendered_prims.push(PrimVertCount(prim_count, vert_count))
                    } else {
                        use global_state::RenderedPrimType::PrimCountVertSizeAndVBs;
                        GLOBAL_STATE.metrics.rendered_prims.push(
                        PrimCountVertSizeAndVBs(prim_count, vert_count, state.rs.vb_state.clone()));
                    }
                }

                // if there is a matching mod, render it
                let mod_status = check_and_render_mod(prim_count, vert_count,
                    |d3dd,nmod| {
                        let override_texture = null_mut();
                        let override_stage = 0_u32;
                        if let ModD3DData::D3D11(d3d11d) = d3dd {
                            render_mod_d3d11(THIS, hook_context, d3d11d, nmod, override_texture, override_stage, (prim_count,vert_count))
                        } else {
                            false
                        }
                    });

                use types::interop::ModType::GPUAdditive;
                let draw_input = match mod_status {
                    CheckRenderModResult::NotRendered => true,
                    CheckRenderModResult::Rendered(mtype) if GPUAdditive as i32 == mtype => true,
                    CheckRenderModResult::Rendered(_) => false, // non-additive mod was rendered
                    CheckRenderModResult::Deleted => false,
                    CheckRenderModResult::NotRenderedButLoadRequested(ref name) => {
                        // setup data to begin mod load
                        let nmod = mod_load::get_mod_by_name(name, &mut GLOBAL_STATE.loaded_mods);
                        if let Some(nmod) = nmod {
                            // need to store current input layout in the d3d data
                            if let ModD3DState::Unloaded =  nmod.d3d_data {
                                let il = state.rs.current_input_layout;
                                if !il.is_null() {
                                    // we're officially keeping an extra reference to the input layout now
                                    // so note that.
                                    (*il).AddRef();
                                    nmod.d3d_data = ModD3DState::Partial(
                                        ModD3DData::D3D11(ModD3DData11::with_layout(il)));
                                    write_log_file(&format!("created partial mod load state for mod {}", nmod.name));
                                    //write_log_file(&format!("current in layout is: {}", il as usize));
                                }
                            }
                        }
                        true
                    },
                };

                //  update metrics
                if METRICS_TRACK_MOD_PRIMS {
                    update_drawn_recently(&mut state.metrics, prim_count, vert_count, &mod_status);
                }

                draw_input
            },
            _ => true
        }
    }).unwrap_or(true);


    if draw_input {
        (hook_context.real_draw_indexed)(
            THIS,
            IndexCount,
            StartIndexLocation,
            BaseVertexLocation,
        );
    }

    // do "per frame" operations this often since I don't have any idea of when the frame
    // ends in this API right now
    if GLOBAL_STATE.metrics.dip_calls % 20000 == 0 {
        draw_periodic();
    }

    // input needs faster processing but it won't update faster than 1 per 16ms
    let fore = dev_state_d3d11_nolock().map(|state| state.app_foreground).unwrap_or(false);
    if GLOBAL_STATE.metrics.dip_calls % 250 == 0 && fore {
        GLOBAL_STATE.input.as_mut().map(|inp| {
            if inp.get_press_fn_count() > 0 {
                inp.process()
                .unwrap_or_else(|e| write_log_file(&format!("input error: {:?}", e)));
            }
        });
    }

    process_metrics(&mut GLOBAL_STATE.metrics, true, 250000);

    GLOBAL_STATE.in_dip = false;
}

/// Call a function with the d3d11 device pointer if it's available.  If pointer is a different,
/// type or is null, does nothing.
fn with_dev_ptr<F>(f: F) where F: FnOnce(DevicePointer) {
    match dev_state().hook {
        Some(HookDeviceState::D3D11(ref dev)) => {
            if !dev.devptr.is_null() {
                f(dev.devptr);
            }
        }
        _ => {},
    };
}

use winapi::shared::minwindef::BOOL;
use winapi::shared::minwindef::TRUE;
unsafe extern "system" fn enum_windows_proc(hwnd:HWND, lparam:isize) -> BOOL {

    dev_state_d3d11_nolock().map(|state| {
        // get the process id that owns the window
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == lparam as u32 {
            state.app_hwnds.push(hwnd);
        }
    });

    TRUE
}
/// Enumerate application top level windows amd return their handles in a vector.
unsafe fn find_app_windows() {
    if let Some(state) = dev_state_d3d11_nolock() { state.app_hwnds.clear(); }

    // get my process id
    let my_pid = GetCurrentProcessId();
    EnumWindows(Some(enum_windows_proc), my_pid as isize);
}

unsafe fn time_based_update(mselapsed:u128, now:SystemTime) {
    if mselapsed > 1000 {
        if let Some(state) = dev_state_d3d11_nolock() { state.last_timebased_update = now; }
        let wnd_count = dev_state_d3d11_nolock().map(|state| {
            state.app_hwnds.len()
        }).unwrap_or(0);
        if wnd_count == 0 {
            find_app_windows();
            let dw = GetDesktopWindow();
            dev_state_d3d11_nolock().map(|state| {
                //let ocount = state.app_hwnds.len();
                let wnds:Vec<HWND> = state.app_hwnds.iter().filter(|wnd| {
                    **wnd != dw && GetParent(**wnd).is_null()
                }).copied().collect();
                state.app_hwnds = wnds;
                //write_log_file(&format!("found {} app windows, filtered to: {:?}", ocount, state.app_hwnds));
            });
        }

        if GLOBAL_STATE.input.is_none() {
            // init input if needed
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
        }

        // find the app main foreground window
        let fwnd = GetForegroundWindow();
        let appfwnd = dev_state_d3d11_nolock().and_then(|state| {
            state.app_hwnds.iter().find(|wnd| {
                **wnd == fwnd
            }).copied()
        });

        // store fore/back status in rs so that input processing doesn't have to do it
        let app_foreground = appfwnd.map(|hwnd| {
            util::appwnd_is_foreground(hwnd)
        }).unwrap_or(false);

        if let Some(state) = dev_state_d3d11_nolock() { state.app_foreground = app_foreground; }

        // finish input setup if needed and app is foreground
        if app_foreground {
            GLOBAL_STATE.input.as_mut().map(|inp| {
                if inp.get_press_fn_count() == 0 {
                    with_dev_ptr(|devptr| {
                        input_commands::setup_input(devptr, inp)
                        .unwrap_or_else(|e| write_log_file(&format!("input setup error: {:?}", e)));
                    })
                }
            });
        }
    }
}

/// Called by DrawIndexed every few 10s of MS but not exactly every frame.
fn draw_periodic() {
    frame_init_clr(dnclr::RUN_CONTEXT_D3D11).unwrap_or_else(|e|
        write_log_file(&format!("init clr failed: {:?}", e)));

    with_dev_ptr(|deviceptr| {
        frame_load_mods(deviceptr);
    });

    unsafe {
        let now = SystemTime::now();
        let (el_sec,el_ms) =
            dev_state_d3d11_nolock().map(|state| {
                let elapsed = now.duration_since(state.last_timebased_update);

                match elapsed {
                    Ok(elapsed) => {
                        (elapsed.as_secs(), elapsed.as_millis())
                    },
                    Err(_) => (0,0)
                }
            }).unwrap_or((0,0));
        let time = (el_sec * 1000) as u128 + el_ms;
        time_based_update(time, now);
    }
}

unsafe fn render_mod_d3d11(context:*mut ID3D11DeviceContext, hook_context: &mut HookDirect3D11Context,
     d3dd:&ModD3DData11, _nmod:&NativeModData,
    _override_texture: *mut c_void, _override_stage:u32,
    _primVerts:(u32,u32)) -> bool {
    if context.is_null() {
        return false;
    }

    // save current device index buffer into local variables
    let mut curr_ibuffer: *mut ID3D11Buffer = null_mut();
    let mut curr_ibuffer_offset: UINT = 0;
    let mut curr_ibuffer_format: DXGI_FORMAT = DXGI_FORMAT_UNKNOWN;
    (*context).IAGetIndexBuffer(&mut curr_ibuffer, &
        mut curr_ibuffer_format, &mut curr_ibuffer_offset);

    // save current device vertex buffer into local variables
    const MAX_VBUFFERS: usize = 16;
    let mut curr_vbuffers: [*mut ID3D11Buffer; MAX_VBUFFERS] = [null_mut(); MAX_VBUFFERS];
    let mut curr_vbuffer_strides: [UINT; MAX_VBUFFERS] = [0; MAX_VBUFFERS];
    let mut curr_vbuffer_offsets: [UINT; MAX_VBUFFERS] = [0; MAX_VBUFFERS];
    (*context).IAGetVertexBuffers(0, MAX_VBUFFERS as u32,
        curr_vbuffers.as_mut_ptr(),
        curr_vbuffer_strides.as_mut_ptr(),
        curr_vbuffer_offsets.as_mut_ptr());

    // set the mod vertex buffer
    let vbuffer = d3dd.vb;
    let vbuffer_stride = [d3dd.vert_size as UINT];
    let vbuffer_offset = [0 as UINT];

    // call direct to avoid entering our hook function
    (hook_context.real_ia_set_vertex_buffers)(
        context,
        0,
        1,
        &vbuffer,
        vbuffer_stride.as_ptr(),
        vbuffer_offset.as_ptr());

    // if the mod has textures, need to set the pixel shader resources for them
    let mut orig_srvs: [*mut ID3D11ShaderResourceView; 16] = [null_mut(); 16];
    // keep this outside of if block so it doesn't get dropped while the context (maybe)
    // still has a reference to it
    let mut mod_srvs;
    if d3dd.has_textures {
        // save the current shader resources
        (*context).PSGetShaderResources(0, 16, orig_srvs.as_mut_ptr());

        // clone the resource list, then replace any texture srvs sequentially with the mod textures
        mod_srvs = orig_srvs.clone();

        let mut next_mod_tex_idx = 0;
        for srv in mod_srvs.iter_mut() {
            if next_mod_tex_idx >= d3dd.srvs.len() {
                break;
            }
            if !srv.is_null() {
                let mut desc: D3D11_SHADER_RESOURCE_VIEW_DESC = std::mem::zeroed();
                (**srv).GetDesc(&mut desc);
                if desc.ViewDimension == D3D11_SRV_DIMENSION_TEXTURE2D {
                    // don't slam it unless we have a value, but increment the index anyway
                    // (in case we only have overrides on later slot(s))
                    if !d3dd.srvs[next_mod_tex_idx].is_null() {
                        *srv = d3dd.srvs[next_mod_tex_idx];
                    }
                    next_mod_tex_idx += 1;
                }
            }
        }

        // set the modded srvs
        (*context).PSSetShaderResources(0, 16, mod_srvs.as_ptr());
    }

    // draw
    (*context).Draw(d3dd.vert_count as UINT, 0);

    // restore srvs
    if d3dd.has_textures {
        (*context).PSSetShaderResources(0, 16, orig_srvs.as_ptr());
    }

    // restore index buffer
    (*context).IASetIndexBuffer(curr_ibuffer, curr_ibuffer_format, curr_ibuffer_offset);

    // restore vertex buffer
    // find first null vbuffer to get actual number of buffers to restore
    let first_null = curr_vbuffers.iter()
        .position(|&x| x.is_null()).unwrap_or(0);

    (hook_context.real_ia_set_vertex_buffers)(
        context,
        0,
        first_null as UINT,
        curr_vbuffers.as_ptr(),
        curr_vbuffer_strides.as_ptr(),
        curr_vbuffer_offsets.as_ptr());

    true
}
//==============================================================================
// Unimplemented draw function hooks

// pub unsafe extern "system" fn hook_draw_instanced(
//     THIS: *mut ID3D11DeviceContext,
//     VertexCountPerInstance: UINT,
//     InstanceCount: UINT,
//     StartVertexLocation: UINT,
//     StartInstanceLocation: UINT,
// ) -> () {
//     let hook_context = match get_hook_context() {
//         Ok(ctx) => ctx,
//         Err(_) => return,
//     };

//     // write_log_file("hook_draw_instanced called");

//     return (hook_context.real_draw_instanced)(
//         THIS,
//         VertexCountPerInstance,
//         InstanceCount,
//         StartVertexLocation,
//         StartInstanceLocation,
//     );
// }

// pub unsafe extern "system" fn hook_draw(
//     THIS: *mut ID3D11DeviceContext,
//     VertexCount: UINT,
//     StartVertexLocation: UINT,
// ) -> () {
//     let hook_context = match get_hook_context() {
//         Ok(ctx) => ctx,
//         Err(_) => return,
//     };

//     // write_log_file("hook_draw called");

//     return (hook_context.real_draw)(
//         THIS,
//         VertexCount,
//         StartVertexLocation,
//     );
// }

// pub unsafe extern "system" fn hook_draw_auto (
//     THIS: *mut ID3D11DeviceContext,
// ) -> () {
//     let hook_context = match get_hook_context() {
//         Ok(ctx) => ctx,
//         Err(_) => return,
//     };

//     // write_log_file("hook_draw_auto called");

//     return (hook_context.real_draw_auto)(
//         THIS,
//     );
// }
