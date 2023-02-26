
use global_state::GLOBAL_STATE;
use shared_dx::types::{HookDeviceState, HookD3D11State};
use shared_dx::types_dx11::HookDirect3D911Context;
use shared_dx::util::write_log_file;
use winapi::shared::ntdef::ULONG;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::{d3d11::ID3D11DeviceContext, winnt::INT};
use winapi::shared::minwindef::UINT;
use device_state::dev_state;
use shared_dx::error::Result;

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

    // horrible hack test to just log one time when this function is called
    // (as if this writing, it isn't)
    // if !GLOBAL_STATE.is_snapping {
    //     write_log_file("congrats hook_draw_indexed was called at least once");
    //     GLOBAL_STATE.is_snapping = true;
    // }
    //write_log_file("hook_draw_indexed called");

    (hook_context.real_draw_indexed)(
        THIS,
        IndexCount,
        StartIndexLocation,
        BaseVertexLocation,
    );

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