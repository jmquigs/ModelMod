use std::ptr::null_mut;

use global_state::GLOBAL_STATE;
use shared_dx::types::{HookDeviceState, HookD3D11State, DevicePointer};
use shared_dx::types_dx11::{HookDirect3D11Context};
use shared_dx::util::write_log_file;
use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout};
use winapi::shared::ntdef::ULONG;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::{d3d11::ID3D11DeviceContext, winnt::INT};
use winapi::shared::minwindef::UINT;
use device_state::dev_state;
use shared_dx::error::Result;

use crate::hook_device_d3d11::apply_context_hooks;
use crate::hook_render::{process_metrics, frame_init_clr, frame_load_mods, check_and_render_mod};
use winapi::um::d3d11::D3D11_BUFFER_DESC;

/// Return the d3d11 context hooks.
fn get_hook_context<'a>() -> Result<&'a mut HookDirect3D11Context> {
    let hooks = match dev_state().hook {
        Some(HookDeviceState::D3D11(HookD3D11State { devptr: _p, hooks: ref mut h })) => h,
        _ => {
            write_log_file(&format!("draw: No d3d11 context found"));
            return Err(shared_dx::error::HookError::D3D11NoContext);
        },
    };
    Ok(&mut hooks.context)
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
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
    if rc < 100 {
        write_log_file(&format!("hook release: rc now {}", rc));
    }
    GLOBAL_STATE.in_hook_release = false;

    rc
}

pub unsafe extern "system" fn hook_VSSetConstantBuffers(
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumBuffers: UINT,
    ppConstantBuffers: *const *mut ID3D11Buffer,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // TODO11: probably need to get more zealous about locking around this as DX11 and later
    // games are more likely to use multihreaded rendering, though hopefully i'll just never use
    // MM with one of those :|

    GLOBAL_STATE.metrics.dx11.vs_set_const_buffers_calls += 1;

    let func_hooked = apply_context_hooks(THIS);
    match func_hooked {
        Ok(n) => {
            if n > 0 {
                GLOBAL_STATE.metrics.dx11.vs_set_const_buffers_hooks += 1;
            }
            //write_log_file(&format!("hook_VSSetConstantBuffers: {} funcs rehooked; call count: {}", n, GLOBAL_STATE.curr_texture_index));
        },
        _ => {
            write_log_file("error late hooking");
        }
    };

    (hook_context.real_vs_setconstantbuffers)(
        THIS,
        StartSlot,
        NumBuffers,
        ppConstantBuffers
    )
}

pub unsafe extern "system" fn hook_IASetVertexBuffers(
    THIS: *mut ID3D11DeviceContext,
    StartSlot: UINT,
    NumBuffers: UINT,
    ppVertexBuffers: *const *mut ID3D11Buffer,
    pStrides: *const UINT,
    pOffsets: *const UINT,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if NumBuffers > 0 && ppVertexBuffers != null_mut() {
        for idx in 0..NumBuffers {
            let pbuf = (*ppVertexBuffers).offset(idx as isize);

            if pbuf != null_mut() {
                // clear on first add of a valid buffer, the game appears to be calling this
                // with 1 null buffer sometimes (and then calling draw) and I don't know why its
                // doing that.
                if idx == 0 {
                    GLOBAL_STATE.dx11rs.vb_state.clear();
                }
                let mut desc:D3D11_BUFFER_DESC = std::mem::zeroed();
                (*pbuf).GetDesc(&mut desc);
                let bw = desc.ByteWidth;
                let stride = desc.StructureByteStride;
                let vbinfo = (idx,bw,stride);
                GLOBAL_STATE.dx11rs.vb_state.push(vbinfo);
            }
        }
        // if GLOBAL_STATE.metrics.dip_calls % 10000 == 0 {
        //     write_log_file(&format!("hook_IASetVertexBuffers: {}, added {}", NumBuffers, GLOBAL_STATE.dx11rs.vb_state.len()));
        // }
    } else if NumBuffers == 0 {
        GLOBAL_STATE.dx11rs.vb_state.clear();
    }

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
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if pInputLayout != null_mut() {
        GLOBAL_STATE.dx11rs.current_input_layout = pInputLayout as u64;
    } else {
        GLOBAL_STATE.dx11rs.current_input_layout = 0;
    }

    (hook_context.real_ia_set_input_layout)(
        THIS,
        pInputLayout
    )
}

pub unsafe extern "system" fn hook_draw_indexed(
    THIS: *mut ID3D11DeviceContext,
    IndexCount: UINT,
    StartIndexLocation: UINT,
    BaseVertexLocation: INT,
) -> () {
    if GLOBAL_STATE.in_dip {
        write_log_file(&format!("ERROR: i'm in DIP already!"));
        return;
    }

    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };
    GLOBAL_STATE.in_dip = true;


    (hook_context.real_draw_indexed)(
        THIS,
        IndexCount,
        StartIndexLocation,
        BaseVertexLocation,
    );

    GLOBAL_STATE.metrics.dip_calls += 1;

    // TODO11 assumes triangle list, should get this from primitive topology
    let prim_count = IndexCount / 3;

    // vert count has to be computed from the current vertex buffer
    // stream and the current input layout (vertex size)
    let curr_input_layout = &GLOBAL_STATE.dx11rs.current_input_layout;
    let curr_layouts = &GLOBAL_STATE.dx11rs.input_layouts_by_ptr;
    let vb_state = &GLOBAL_STATE.dx11rs.vb_state;
    let compute_vert_info = || -> Option<(u32,u32)> {
        let vb_size = match vb_state.len() {
            1 => {
                let (_index,byteWidth,_stride) = vb_state[0];
                if byteWidth == 0 {
                    write_log_file("hook draw indexed: current vb has zero byte size");
                    return None;
                }
                byteWidth
            },
            // TODO11: log warning but it could be spammy, maybe throttle it
            0 => {
                write_log_file("hook draw indexed: no current vertex buffer set");
                return None;
            },
            _n => {
                // not sure how to figure out which one to use, maybe log warning
                return None;
            }
        };
        let vert_size = {
            if *curr_input_layout > 0 {
                curr_layouts.as_ref().and_then(|hm| {
                    hm.get(&curr_input_layout)
                }).and_then(|vf|
                    Some(vf.size)
                ).unwrap_or(0)
            } else {
                0
            }
        };
        if vert_size == 0 {
            return None;
        }
        Some((vb_size,vert_size))
    };

    let (vb_size,vert_size) = compute_vert_info().unwrap_or((0,0));
    let vert_count = if vert_size > 0 {
        vb_size / vert_size
    } else {
        0
    };

    // if primitive tracking is enabled, log just the primcount,vertcount if we were able
    // to compute it, otherwise log whatever we have
    if global_state::METRICS_TRACK_PRIMS && prim_count > 2 { // filter out some spammy useless stuff
        if vert_count > 0 {
            use global_state::RenderedPrimType::PrimVertCount;
            GLOBAL_STATE.metrics.rendered_prims.push(PrimVertCount(prim_count, vert_count))
        } else {
            use global_state::RenderedPrimType::PrimCountVertSizeAndVBs;
            GLOBAL_STATE.metrics.rendered_prims.push(
            PrimCountVertSizeAndVBs(prim_count, vert_size, GLOBAL_STATE.dx11rs.vb_state.clone()));
        }

    }

    if prim_count > 2 && vert_count > 2 {
        // if there is a matching mod, render it
        let _modded = check_and_render_mod(prim_count, vert_count,
            |_d3dd,nmod| {
                // this doesn't log ATM because there is no mod data loaded, but I can tell
                // that it's finding a mod because its logging about how it wants to
                // load deferred mods but can't due to missing device
                if GLOBAL_STATE.metrics.dip_calls % 1000 == 0 {
                    write_log_file(&format!("want to render mod {}", nmod.name));
                }
                false
                // render_mod_d3d9(THIS, d3dd, nmod,
                //     override_texture, sel_stage,
                //     (primCount,NumVertices))
            });
    }

    // do "per frame" operations this often since I don't have any idea of when the frame
    // ends in this API right now
    if GLOBAL_STATE.metrics.dip_calls % 20000 == 0 {
        draw_periodic();
    }

    process_metrics(&mut GLOBAL_STATE.metrics, true, 50000);

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

/// Called by DrawIndexed every few 10s of MS but not exactly every frame.
fn draw_periodic() {
    frame_init_clr(dnclr::RUN_CONTEXT_D3D11).unwrap_or_else(|e|
        write_log_file(&format!("init clr failed: {:?}", e)));

    with_dev_ptr(|deviceptr| frame_load_mods(deviceptr));
}

//==============================================================================
// Unimplemented draw function hooks

pub unsafe extern "system" fn hook_draw_instanced(
    THIS: *mut ID3D11DeviceContext,
    VertexCountPerInstance: UINT,
    InstanceCount: UINT,
    StartVertexLocation: UINT,
    StartInstanceLocation: UINT,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // write_log_file("hook_draw_instanced called");

    return (hook_context.real_draw_instanced)(
        THIS,
        VertexCountPerInstance,
        InstanceCount,
        StartVertexLocation,
        StartInstanceLocation,
    );
}

pub unsafe extern "system" fn hook_draw(
    THIS: *mut ID3D11DeviceContext,
    VertexCount: UINT,
    StartVertexLocation: UINT,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // write_log_file("hook_draw called");

    return (hook_context.real_draw)(
        THIS,
        VertexCount,
        StartVertexLocation,
    );
}

pub unsafe extern "system" fn hook_draw_auto (
    THIS: *mut ID3D11DeviceContext,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // write_log_file("hook_draw_auto called");

    return (hook_context.real_draw_auto)(
        THIS,
    );
}
