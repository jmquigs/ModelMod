use std::ptr::null_mut;

use winapi::ctypes::c_void;
use winapi::um::d3d11::ID3D11DeviceChildVtbl;
use winapi::um::d3dcommon::D3D_DRIVER_TYPE;
use winapi::um::d3dcommon::D3D_FEATURE_LEVEL;
use winapi::um::d3d11::ID3D11Device;
use winapi::shared::dxgi::IDXGIAdapter;
use winapi::um::d3d11::ID3D11DeviceContext;
use winapi::um::d3d11::ID3D11DeviceContextVtbl;
use winapi::shared::dxgi::DXGI_SWAP_CHAIN_DESC;
use winapi::shared::dxgi::IDXGISwapChain;
use winapi::shared::winerror::HRESULT;
use winapi::shared::minwindef::{FARPROC, HMODULE, UINT};
use winapi::shared::winerror::E_FAIL;
use shared_dx::error::*;
use winapi::um::unknwnbase::IUnknownVtbl;

use crate::hook_device::{load_d3d_lib, init_device_state_once, init_log, mm_verify_load};
use shared_dx::util::write_log_file;
use shared_dx::types_dx11::HookDirect3D911Context;
use shared_dx::types::HookDeviceState;
use shared_dx::types::HookD3D11State;
use device_state::DEVICE_STATE;
use crate::hook_render_d3d11::*;

//use shared_dx::util::{set_log_file_path};

use global_state::{GLOBAL_STATE, GLOBAL_STATE_LOCK};

type D3D11CreateDeviceFN = extern "system" fn (
    pAdapter: *mut IDXGIAdapter,
    DriverType: D3D_DRIVER_TYPE,
    Software: HMODULE,
    Flags: UINT,
    pFeatureLevels: *const D3D_FEATURE_LEVEL,
    FeatureLevels: UINT,
    SDKVersion: UINT,
    ppDevice: *mut *mut ID3D11Device,
    pFeatureLevel: *mut D3D_FEATURE_LEVEL,
    ppImmediateContext: *mut *mut ID3D11DeviceContext,
) -> HRESULT;

fn load_d3d11_and_func(func_name:&str) -> Result<FARPROC> {
    let handle = load_d3d_lib("d3d11.dll")?;
    let create = util::get_proc_address(handle, func_name)?;
    Ok(create)
}

#[allow(unused)]
#[no_mangle]
pub extern "system" fn D3D11CreateDevice(
    pAdapter: *mut IDXGIAdapter,
    DriverType: D3D_DRIVER_TYPE,
    Software: HMODULE,
    Flags: UINT,
    pFeatureLevels: *const D3D_FEATURE_LEVEL,
    FeatureLevels: UINT,
    SDKVersion: UINT,
    ppDevice: *mut *mut ID3D11Device,
    pFeatureLevel: *mut D3D_FEATURE_LEVEL,
    ppImmediateContext: *mut *mut ID3D11DeviceContext,
) -> HRESULT {
    // let _ = shared_dx::util::set_log_file_path("D:\\Temp\\", "ModelModTempLog.txt");
    // write_log_file("D3D11CreateDevice called");

    match load_d3d11_and_func("D3D11CreateDevice") {
        Ok(fptr) => unsafe {
            let create_fn:D3D11CreateDeviceFN = std::mem::transmute(fptr);
            // hopefully don't need to specify this anything differently from what
            // app requests.
            //let mut FeatureLevel = winapi::um::d3dcommon::D3D_FEATURE_LEVEL_11_0;

            let res = create_fn(pAdapter, DriverType, Software, Flags,
                pFeatureLevels, FeatureLevels,
                SDKVersion,
                ppDevice, pFeatureLevel, ppImmediateContext);
            if res == 0 && ppImmediateContext != null_mut() {
                // swap chain comes from DXGI in this code path, I'm probably going to have to hook
                // that too since I don't think there is another way to get the one the app creates.
                // (its not available from context or device?)
                match init_d3d11(std::ptr::null_mut(), (*ppImmediateContext)) {
                    Ok(_) => {},
                    Err(e) => { write_log_file(&format!("Error, init_d3d11 failed: {:?}", e))}
                }
            }
            res
        }
        Err(x) => {
            write_log_file(&format!("create_d3d failed: {:?}", x));
            E_FAIL
        }
    }
}

type D3D11CreateDeviceAndSwapChainFN = extern "system" fn (
    pAdapter: *mut IDXGIAdapter,
    DriverType: D3D_DRIVER_TYPE,
    Software: HMODULE,
    Flags: UINT,
    pFeatureLevels: *const D3D_FEATURE_LEVEL,
    FeatureLevels: UINT,
    SDKVersion: UINT,
    pSwapChainDesc: *const DXGI_SWAP_CHAIN_DESC,
    ppSwapChain: *mut *mut IDXGISwapChain,
    ppDevice: *mut *mut ID3D11Device,
    pFeatureLevel: *mut D3D_FEATURE_LEVEL,
    ppImmediateContext: *mut *mut ID3D11DeviceContext,
) -> HRESULT;

#[allow(unused)]
#[no_mangle]
pub extern "system" fn D3D11CreateDeviceAndSwapChain(
    pAdapter: *mut IDXGIAdapter,
    DriverType: D3D_DRIVER_TYPE,
    Software: HMODULE,
    Flags: UINT,
    pFeatureLevels: *const D3D_FEATURE_LEVEL,
    FeatureLevels: UINT,
    SDKVersion: UINT,
    pSwapChainDesc: *const DXGI_SWAP_CHAIN_DESC,
    ppSwapChain: *mut *mut IDXGISwapChain,
    ppDevice: *mut *mut ID3D11Device,
    pFeatureLevel: *mut D3D_FEATURE_LEVEL,
    ppImmediateContext: *mut *mut ID3D11DeviceContext,
) -> HRESULT {
    // let _ = shared_dx::util::set_log_file_path("D:\\Temp\\", "ModelModTempLog.txt");
    // write_log_file("D3D11CreateDeviceAndSwapChain called");

    match load_d3d11_and_func("D3D11CreateDeviceAndSwapChain") {
        Ok(fptr) => unsafe {
            let create_fn:D3D11CreateDeviceAndSwapChainFN = std::mem::transmute(fptr);
            let res = create_fn(pAdapter, DriverType, Software, Flags, pFeatureLevels, FeatureLevels, SDKVersion,
                pSwapChainDesc, ppSwapChain, ppDevice, pFeatureLevel, ppImmediateContext);
            // TODO: call init_d3d when that code is finished
            res
        }
        Err(x) => {
            write_log_file(&format!("create_d3d failed: {:?}", x));
            E_FAIL
        }
    }
}

pub unsafe fn apply_context_hooks(context:*mut ID3D11DeviceContext) -> Result<i32> {
    let vtbl: *mut ID3D11DeviceContextVtbl = std::mem::transmute((*context).lpVtbl);
    let vsize = std::mem::size_of::<ID3D11DeviceContextVtbl>();

    // TODO11: should only call this if I actually need to rehook
    // since it probably isn't cheap
    let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;
    let device_child = &mut (*vtbl).parent;
    let iunknown = &mut (*device_child).parent;

    let mut func_hooked = 0;

    if (*iunknown).Release as u64 != hook_release as u64 {
        (*iunknown).Release = hook_release;
        func_hooked += 1;
    }
    if (*vtbl).VSSetConstantBuffers as u64 != hook_VSSetConstantBuffers as u64 {
        (*vtbl).VSSetConstantBuffers = hook_VSSetConstantBuffers;
        func_hooked += 1;
    }
    if (*vtbl).DrawIndexed as u64 != hook_draw_indexed as u64 {
        (*vtbl).DrawIndexed = hook_draw_indexed;
        func_hooked += 1;
    }
    // TODO11: hook remaining draw functions (if needed)

    util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
    Ok(func_hooked)
}

unsafe fn hook_d3d11(_swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) ->
    Result<HookDirect3D911Context> {

    write_log_file(&format!("hooking new d3d11 context: {:x}", context as u64));
    let vtbl: *mut ID3D11DeviceContextVtbl = std::mem::transmute((*context).lpVtbl);
    let ct = (*context).GetType();
    let flags = (*context).GetContextFlags();
    write_log_file(&format!("context vtbl: {:x}, type {:x}, flags {:x}",
        vtbl as u64, ct, flags));
    //let vsize = std::mem::size_of::<ID3D11DeviceContextVtbl>();

    let device_child = &mut (*vtbl).parent;
    write_log_file(&format!("device_child vtbl: {:x}", device_child as *mut ID3D11DeviceChildVtbl as u64));
    let iunknown = &mut (*device_child).parent;
    write_log_file(&format!("iunknown vtbl: {:x}", iunknown as *mut IUnknownVtbl as u64));

    let real_release = (*iunknown).Release;
    let real_vs_setconstantbuffers = (*vtbl).VSSetConstantBuffers;
    let real_draw = (*vtbl).Draw;
    let real_draw_auto = (*vtbl).DrawAuto;
    let real_draw_indexed = (*vtbl).DrawIndexed;
    let real_draw_indexed_instanced = (*vtbl).DrawIndexedInstanced;
    let real_draw_instanced = (*vtbl).DrawInstanced;
    let real_draw_indexed_instanced_indirect = (*vtbl).DrawIndexedInstancedIndirect;
    let real_draw_instanced_indirect = (*vtbl).DrawInstancedIndirect;
    // check for already hook devices (useful in late-hook case)
    if real_release as u64 == hook_release as u64 {
        write_log_file(&format!("error: device already appears to be hooked, skipping"));
        return Err(HookError::D3D11DeviceHookFailed);
    }

    let func_hooked = apply_context_hooks(context)?;

    // Inc ref count on the device
    //(*context).AddRef(); // TODO11: dx9 does this, but needed here? and where is this decremented?

    write_log_file(&format!("context hook complete: {} functions hooked", func_hooked));
    let hook_context = HookDirect3D911Context {
        real_release,
        real_vs_setconstantbuffers,
        real_draw,
        real_draw_auto,
        real_draw_indexed,
        real_draw_instanced,
        real_draw_indexed_instanced,
        real_draw_instanced_indirect,
        real_draw_indexed_instanced_indirect,
    };

    Ok(hook_context)
}

fn init_d3d11(swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) -> Result<()> {
    init_device_state_once();
    let mm_root = match mm_verify_load() {
        Some(dir) => dir,
        None => {
            return Err(HookError::D3D9DeviceHookFailed)
        }
    };
    init_log(&mm_root);
    unsafe {
        GLOBAL_STATE.mm_root = Some(mm_root);

        let _lock = GLOBAL_STATE_LOCK
        .lock()
        .map_err(|_err| HookError::GlobalLockError)?;

        let h_context = hook_d3d11(swapchain, context)?;

        (*DEVICE_STATE).hook = Some(HookDeviceState::D3D11(HookD3D11State {
            context: h_context,
        }));

        //(*DEVICE_STATE).d3d_window = hFocusWindow; // TODO11: need to get this in d3d11
        // TODO11: d3d9 also has: d3d_resource_count: 0,

        write_log_file(&format!(
            "hooked device on thread {:?}",
            std::thread::current().id()
        ));
    }

    Ok(())
}
