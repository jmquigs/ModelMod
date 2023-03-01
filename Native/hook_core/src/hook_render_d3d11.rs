
use global_state::GLOBAL_STATE;
use shared_dx::types::{HookDeviceState, HookD3D11State};
use shared_dx::types_dx11::HookDirect3D911Context;
use shared_dx::util::write_log_file;
use winapi::um::d3d11::ID3D11Buffer;
use winapi::shared::ntdef::ULONG;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::{d3d11::ID3D11DeviceContext, winnt::INT};
use winapi::shared::minwindef::UINT;
use device_state::dev_state;
use shared_dx::error::Result;

use crate::hook_device_d3d11::apply_context_hooks;
use crate::hook_render::process_metrics;

fn get_hook_context<'a>() -> Result<&'a mut HookDirect3D911Context> {
    let hookcontext = match dev_state().hook {
        Some(HookDeviceState::D3D11(HookD3D11State { context: ref mut ctx })) => ctx,
        _ => {
            write_log_file(&format!("draw: No d3d11 context found"));
            return Err(shared_dx::error::HookError::D3D11NoContext);
        },
    };
    Ok(hookcontext)
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

pub unsafe extern "system" fn hook_draw(
    THIS: *mut ID3D11DeviceContext,
    VertexCount: UINT,
    StartVertexLocation: UINT,
) -> () {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    write_log_file("hook_draw called");

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

    write_log_file("hook_draw_auto called");

    return (hook_context.real_draw_auto)(
        THIS,
    );
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

    // prim count, assuming triangle lists, = index count / 3??
    // vertex count == ?? need to get that from the vertex buffer probably, because I
    // don't think I can compute it here (a 2 primitive set could be 4 verts if the triangles
    // make a square, 5 if they share only one point, or 6 if they are completely distinct)
    // however based on initial observations the primtive count of indexcount/3 is accurate
    // (at least assuming triangle lists, which I should also check)
    GLOBAL_STATE.metrics.rendered_prims.push((IndexCount/3, 0));

    process_metrics(&mut GLOBAL_STATE.metrics, true, 100000);

    GLOBAL_STATE.in_dip = false;
}

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

    write_log_file("hook_draw_instanced called");

    return (hook_context.real_draw_instanced)(
        THIS,
        VertexCountPerInstance,
        InstanceCount,
        StartVertexLocation,
        StartInstanceLocation,
    );
}