use std::ptr::null_mut;

use global_state::GLOBAL_STATE;
use global_state::dx11rs::DX11RenderState;
use shared_dx::types::{HookDeviceState, HookD3D11State, DevicePointer};
use shared_dx::types_dx11::{HookDirect3D11Context};
use shared_dx::util::write_log_file;
use types::d3ddata::ModD3DData11;
use types::native_mod::{ModD3DData, ModD3DState, NativeModData};
use winapi::ctypes::c_void;
use winapi::shared::dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN};
use winapi::um::d3d11::{ID3D11Buffer, ID3D11InputLayout};
use winapi::shared::ntdef::ULONG;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::{d3d11::ID3D11DeviceContext, winnt::INT};
use winapi::shared::minwindef::UINT;
use device_state::dev_state;
use shared_dx::error::Result;

use crate::hook_device_d3d11::apply_context_hooks;
use crate::hook_render::{process_metrics, frame_init_clr, frame_load_mods, check_and_render_mod, CheckRenderModResult};
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

    // rehook to reduce flickering
    let _func_hooked = apply_context_hooks(THIS);

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

    // rehook to reduce flickering
    let _func_hooked = apply_context_hooks(THIS);

    if pInputLayout != null_mut() {
        GLOBAL_STATE.dx11rs.current_input_layout = pInputLayout;
    } else {
        GLOBAL_STATE.dx11rs.current_input_layout = null_mut();
    }

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
    // TODO11 assumes triangle list, should get this from primitive topology
    let prim_count = index_count / 3;

    // vert count has to be computed from the current vertex buffer
    // stream and the current input layout (vertex size)
    let curr_input_layout = &rs.current_input_layout;
    let curr_layouts = &rs.input_layouts_by_ptr;
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
    let vert_size = {
        let curr_input_layout = *curr_input_layout as u64;
        if curr_input_layout > 0 {
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

    let vert_count = if vert_size > 0 {
        vb_size / vert_size
    } else {
        0
    };

    Some((prim_count,vert_count))
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

    GLOBAL_STATE.metrics.dip_calls += 1;

    let draw_input = match compute_prim_vert_count(IndexCount, &GLOBAL_STATE.dx11rs) {
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
                    PrimCountVertSizeAndVBs(prim_count, vert_count, GLOBAL_STATE.dx11rs.vb_state.clone()));
                }
            }

            // if there is a matching mod, render it
            let mod_status = check_and_render_mod(prim_count, vert_count,
                |d3dd,nmod| {
                    let override_texture = null_mut();
                    let override_stage = 0 as u32;
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
                CheckRenderModResult::NotRenderedButLoadRequested(name) => {
                    // setup data to begin mod load
                    let nmod = mod_load::get_mod_by_name(&name, &mut GLOBAL_STATE.loaded_mods);
                    if let Some(nmod) = nmod {
                        // need to store current input layout in the d3d data
                        if let ModD3DState::Unloaded =  nmod.d3d_data {
                            let il = GLOBAL_STATE.dx11rs.current_input_layout;
                            if !il.is_null() {
                                // we're officially keeping an extra reference to the input layout now
                                // so note that.
                                (*il).AddRef();
                                nmod.d3d_data = ModD3DState::Partial(
                                    ModD3DData::D3D11(ModD3DData11::with_layout(il)));
                                write_log_file(&format!("created partial mod load state for mod {}", nmod.name));
                                //write_log_file(&format!("current in layout is: {}", il as u64));
                            }
                        }
                    }
                    true
                },
            };

            draw_input
        },
        _ => true
    };

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
        // let vinfo = compute_prim_vert_count(IndexCount, &GLOBAL_STATE.dx11rs);
        // write_log_file(&format!("  last vinfo: {:?}", vinfo));

        // let cil = GLOBAL_STATE.dx11rs.current_input_layout as u64;
        // GLOBAL_STATE.dx11rs.input_layouts_by_ptr.as_ref()
        //     .and_then(|hm| {
        //         hm.get(&cil)
        //     })
        //     .and_then(|vf| {
        //         write_log_file(&format!("  last layout vertex: {}", vf));
        //         Some(())
        //     });

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

    with_dev_ptr(|deviceptr| {
        frame_load_mods(deviceptr);
    });
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

    // draw
    (*context).Draw(d3dd.vert_count as UINT, 0);

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
