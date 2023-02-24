use std::ptr::null_mut;

use winapi::um::d3dcommon::D3D_DRIVER_TYPE;
use winapi::um::d3dcommon::D3D_FEATURE_LEVEL;
use winapi::um::d3d11::ID3D11Device;
use winapi::shared::dxgi::IDXGIAdapter;
use winapi::um::d3d11::ID3D11DeviceContext;
use winapi::shared::dxgi::DXGI_SWAP_CHAIN_DESC;
use winapi::shared::dxgi::IDXGISwapChain;
use winapi::shared::winerror::HRESULT;
use winapi::shared::minwindef::{FARPROC, HMODULE, UINT};
use winapi::shared::winerror::E_FAIL;
use shared_dx9::error::*;

use crate::hook_device::{load_d3d_lib, init_device_state_once, init_log, mm_verify_load};
use shared_dx9::util::write_log_file;
//use shared_dx9::util::{set_log_file_path};

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
    // let _ = set_log_file_path("D:\\Temp\\", "ModelModTempLog.txt");
    // write_log_file("D3D11CreateDevice called");

    match load_d3d11_and_func("D3D11CreateDevice") {
        Ok(fptr) => unsafe {
            let create_fn:D3D11CreateDeviceFN = std::mem::transmute(fptr);
            let res = create_fn(pAdapter, DriverType, Software, Flags, pFeatureLevels, FeatureLevels, SDKVersion,
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
    // let _ = set_log_file_path("D:\\Temp\\", "ModelModTempLog.txt");
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

fn hook_d3d11(_swapchain:*mut IDXGISwapChain, _context:*mut ID3D11DeviceContext) -> Result<()> {
    Ok(())
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

        hook_d3d11(swapchain, context)?;

        // TODO: set in device state

        write_log_file(&format!("now would be a good time to hook the d3d11 device"));

        //let hook_d3d9device = hook_device(device, &lock)?;

        //(*DEVICE_STATE).d3d_window = hFocusWindow; // TODO: need to get this in d3d11
        //(*DEVICE_STATE).hook_direct3d9device = Some(hook_d3d9device);

        write_log_file(&format!(
            "hooked device on thread {:?}",
            std::thread::current().id()
        ));
    }

    Ok(())
}
