use std::ffi::CStr;
use std::ptr::null_mut;

use shared_dx::types::DevicePointer;
use winapi::ctypes::c_void;
use winapi::shared::basetsd::SIZE_T;
use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::um::d3d11::D3D11_APPEND_ALIGNED_ELEMENT;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;
use winapi::um::d3d11::ID3D11DeviceVtbl;
use winapi::um::d3d11::ID3D11InputLayout;
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

use shared_dx::types_dx11::HookDirect3D11Context;
use shared_dx::types_dx11::HookDirect3D11Device;
use shared_dx::error::*;
use device_state::dev_state;
use global_state::new_fnv_map;
use global_state::dx11rs::{VertexFormat};

use crate::hook_device::{load_d3d_lib, init_device_state_once, init_log, mm_verify_load};
use shared_dx::util::write_log_file;
use shared_dx::types_dx11::HookDirect3D11;
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
                match init_d3d11( (*ppDevice), std::ptr::null_mut(), (*ppImmediateContext)) {
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
    // actually I think technically I may not need to do this as the vtable is part of the
    // object and therefore unprotected memory (its not in the code segment).
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
    if (*vtbl).IASetVertexBuffers as u64 != hook_IASetVertexBuffers as u64 {
        (*vtbl).IASetVertexBuffers = hook_IASetVertexBuffers;
        func_hooked += 1;
    }
    if (*vtbl).IASetInputLayout as u64 != hook_IASetInputLayout as u64 {
        (*vtbl).IASetInputLayout = hook_IASetInputLayout;
        func_hooked += 1;
    }
    // TODO11: hook remaining draw functions (if needed)

    util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
    Ok(func_hooked)
}

unsafe fn hook_d3d11(device:*mut ID3D11Device,_swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) ->
    Result<HookDirect3D11> {

    let hook_device = {
        write_log_file(&format!("hooking new d3d11 device: {:x}", device as u64));
        let vtbl: *mut ID3D11DeviceVtbl = std::mem::transmute((*device).lpVtbl);
        let vsize = std::mem::size_of::<ID3D11DeviceVtbl>();
        let real_create_input_layout = (*vtbl).CreateInputLayout;

        let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;
        (*vtbl).CreateInputLayout = hook_CreateInputLayoutFn;
        util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

        HookDirect3D11Device {
            real_create_input_layout,
        }
    };

    write_log_file(&format!("hooking new d3d11 context: {:x}", context as u64));
    let vtbl: *mut ID3D11DeviceContextVtbl = std::mem::transmute((*context).lpVtbl);
    let ct = (*context).GetType();
    let flags = (*context).GetContextFlags();
    write_log_file(&format!("context vtbl: {:x}, type {:x}, flags {:x}",
        vtbl as u64, ct, flags));
    //let vsize = std::mem::size_of::<ID3D11DeviceContextVtbl>();

    let device_child = &mut (*vtbl).parent;
    let iunknown = &mut (*device_child).parent;

    let real_release = (*iunknown).Release;
    let real_vs_setconstantbuffers = (*vtbl).VSSetConstantBuffers;
    let real_draw = (*vtbl).Draw;
    let real_draw_auto = (*vtbl).DrawAuto;
    let real_draw_indexed = (*vtbl).DrawIndexed;
    let real_draw_indexed_instanced = (*vtbl).DrawIndexedInstanced;
    let real_draw_instanced = (*vtbl).DrawInstanced;
    let real_draw_indexed_instanced_indirect = (*vtbl).DrawIndexedInstancedIndirect;
    let real_draw_instanced_indirect = (*vtbl).DrawInstancedIndirect;
    let real_ia_set_vertex_buffers = (*vtbl).IASetVertexBuffers;
    let real_ia_set_input_layout = (*vtbl).IASetInputLayout;
    // check for already hook devices (useful in late-hook case)
    if real_release as u64 == hook_release as u64 {
        write_log_file(&format!("error: device already appears to be hooked, skipping"));
        return Err(HookError::D3D11DeviceHookFailed);
    }

    let func_hooked = apply_context_hooks(context)?;

    // Inc ref count on the device
    //(*context).AddRef(); // TODO11: dx9 does this, but needed here? and where is this decremented?

    write_log_file(&format!("context hook complete: {} functions hooked", func_hooked));
    let hook_context = HookDirect3D11Context {
        real_release,
        real_vs_setconstantbuffers,
        real_draw,
        real_draw_auto,
        real_draw_indexed,
        real_draw_instanced,
        real_draw_indexed_instanced,
        real_draw_instanced_indirect,
        real_draw_indexed_instanced_indirect,
        real_ia_set_vertex_buffers,
        real_ia_set_input_layout,
    };

    Ok(HookDirect3D11 { device: hook_device, context: hook_context })
}

fn init_d3d11(device:*mut ID3D11Device, swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) -> Result<()> {
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

        let hooks = hook_d3d11(device, swapchain, context)?;

        (*DEVICE_STATE).hook = Some(HookDeviceState::D3D11(HookD3D11State {
            hooks,
            devptr: DevicePointer::D3D11(device),
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

// ===============
// device hook fns

/// Returns the hooks for the device.  Note this does not actually return the device pointer,
/// since it is assumed the caller already has that.
fn get_hook_device<'a>() -> Result<&'a mut HookDirect3D11Device> {
    let hooks = match dev_state().hook {
        Some(HookDeviceState::D3D11(HookD3D11State { devptr: _p, hooks: ref mut h })) => h,
        _ => {
            write_log_file(&format!("draw: No d3d11 context found"));
            return Err(shared_dx::error::HookError::D3D11NoContext);
        },
    };
    Ok(&mut hooks.device)
}


pub fn get_format_size_bytes(format:&DXGI_FORMAT) -> Option<u32> {
    use winapi::shared::dxgiformat::*;
    // there are a zillion formats and I don't care about most so just defining sizes
    // for the ones I've observed
    let size =
        match format {
            &DXGI_FORMAT_R32G32_FLOAT => 8,
            &DXGI_FORMAT_R32G32B32_FLOAT => 12,
            &DXGI_FORMAT_R32G32B32A32_FLOAT => 16,
            &DXGI_FORMAT_R32G32_UINT => 8,
            &DXGI_FORMAT_R32G32B32_UINT => 12,
            &DXGI_FORMAT_R32G32B32A32_UINT => 16,
            &DXGI_FORMAT_R32G32_SINT => 8,
            &DXGI_FORMAT_R32G32B32_SINT => 12,
            &DXGI_FORMAT_R32G32B32A32_SINT => 16,
            _ => 0,
        };
    Some(size)
}

fn vertex_format_from_layout(layout: Vec<D3D11_INPUT_ELEMENT_DESC>) -> VertexFormat {
    use winapi::shared::dxgiformat::DXGI_FORMAT_UNKNOWN;
    use winapi::um::d3d11::D3D11_INPUT_PER_VERTEX_DATA;
    // try to compute size, but if any offsets are D3D11_APPEND_ALIGNED_ELEMENT, give up
    // because I don't want to write the code to interpret that right now.

    // sort by offset, then size is highest offset + size of format for it
    let size = {
        let mut layout = layout.clone();
        layout.sort_by_key( |el| el.AlignedByteOffset);

        let append_aligned_found =
            layout.iter().find(|x| x.AlignedByteOffset == D3D11_APPEND_ALIGNED_ELEMENT);
        if append_aligned_found.is_some() {
            write_log_file(&format!("WARNING: vertex has dynamic size, not computed"));
            0
        } else {
            let high_el = layout.iter().rev().find(|el|
                el.Format != DXGI_FORMAT_UNKNOWN && el.InputSlotClass == D3D11_INPUT_PER_VERTEX_DATA);
            match high_el {
                Some(el) => {
                    let fmtsize = get_format_size_bytes(&el.Format)
                        .unwrap_or_else(|| {
                            write_log_file(&format!("ERROR: no size for format: {:?}", el.Format));
                            0
                        });
                    el.AlignedByteOffset + fmtsize
                },
                None => {
                    write_log_file(
                        &format!("ERROR: can't compute vertex size, no high offset found"));
                    0
                }
            }
        }
    };
    VertexFormat {
        layout,
        size
    }
}

unsafe extern "system" fn hook_CreateInputLayoutFn(
    THIS: *mut ID3D11Device,
    pInputElementDescs: *const D3D11_INPUT_ELEMENT_DESC,
    NumElements: UINT,
    pShaderBytecodeWithInputSignature: *const c_void,
    BytecodeLength: SIZE_T,
    ppInputLayout: *mut *mut ID3D11InputLayout,
) -> HRESULT {
    let hook_device = match get_hook_device() {
        Ok(dev) => dev,
        Err(_) => {
            write_log_file(&format!("OOPS hook_CreateInputLayoutFn returning {} due to bad state", E_FAIL));
            return E_FAIL;
        }
    };

    // ignore layouts that don't have "POSITION" (i.e. only want vertex layout)
    let mut has_position = false;

    let mut elements:Vec<D3D11_INPUT_ELEMENT_DESC> = Vec::new();
    for i in 0..NumElements {
        let p = *pInputElementDescs.offset(i as isize);
        let name =  CStr::from_ptr(p.SemanticName).to_string_lossy().to_ascii_lowercase();
        if name.starts_with("position") { // hopefully these idents aren't localized?
            has_position = true;
        }
        elements.push(p);
    }

    let res = (hook_device.real_create_input_layout)(
        THIS,
        pInputElementDescs,
        NumElements,
        pShaderBytecodeWithInputSignature,
        BytecodeLength,
        ppInputLayout
    );

    if res == 0 && has_position && ppInputLayout != null_mut() && (*ppInputLayout) != null_mut() {
        let vf = vertex_format_from_layout(elements);

        if GLOBAL_STATE.dx11rs.input_layouts_by_ptr.is_none() {
            GLOBAL_STATE.dx11rs.input_layouts_by_ptr = Some(new_fnv_map(1024));
        }
        // TODO11: when is this cleared?  what happens if it gets big?
        // (maybe game recreates layouts on device reset?)
        // could hook Release on the layout to remove them, ugh.
        GLOBAL_STATE.dx11rs.input_layouts_by_ptr
            .as_mut().map(|hm| {
                hm.insert(*ppInputLayout as u64, vf);

                if hm.len() % 20 == 0 {
                    write_log_file(&format!("vertex layout table now has {} elements",
                    hm.len()));
                }
            });
    }

    res
}
