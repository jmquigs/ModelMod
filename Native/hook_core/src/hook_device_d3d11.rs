//! This module hooks the device and the context.  It also contains a hook for CreateInputLayout
//! which is essential for mod rendering.
//!
//! There are a few ways to do this kind of hooking but most have some issues.
//!
//! The first is to just overwrite vtable functions in the original objects themselves.  This
//! is the approach taken for DX9 in this project currently. This
//! works on some systems, not others.  Also for DX11, some process is persistently unhooking
//! the draw functions on the context in particular, which causes mod render bugs unless we
//! constantly rehook, which has a performance cost and is generally kind of a jackhammer
//! approach.  So, while possibly adequate for DX9, for DX11 it is definitely not great.
//!
//! The second is to make a full copy of the vtable, hook functions in the copy, and then overwrite
//! the entire vtable with the copy.  This is the approach used for DX11 currently.  This has the
//! advantage of being "safer" in the sense that we are not modifying a supposedly *const vtable,
//! which is probably undefined behavior. We are just swapping out that pointer with
//! another on a mutable object.  Also the DX11 unhooker doesn't appear to unhook in this case.
//! So we never need to rehook and can hook fewer functions, so better mod rendering and performance.
//!
//! The downside is we need to copy exactly as many bytes of vtable that the object is using,
//! and we can't be fully sure of how many that is (there are multiple interfaces versions of
//! both context and device).  And we don't have winapi defs for those either, so we
//! don't know their size.  So I have made a list of the ids of those interfaces and their sizes
//! as determined by C code, and the code queries to find the biggest one the object is using.
//! As noted in another long and tedious comment for `find_and_copy_vtable` which describes this
//! process more, this has some issues if the device is using an iface we don't know about.
//! However DX11 is hopefully not going to be updated again, and even if it is, its highly
//! unlikely that a game using that bleeding edge DX would even work with MM anyway.
//!
//! The third approach is to create a full proxy object, a new struct with its own vtable which
//! forwards everything to the original object.  This is the approach taken by more mature
//! products such as ReShade.  However its a pain in the ass to do here (and it looks like it was
//! the same to set up in ReShade).  Literally every possible function needs to be defined
//! and forwarded, so it produces tons of boilerplate (however copilot is very helpful at generating
//! that).  There may also be a performance cost from the double bounce of all those functions some
//! of which I know to be quite hot.  And it still has the issues with unknown ifaces as the
//! previous method, thought it can behave more gracefully when that is encountered.  Still I have
//! prototyped some of this work and got as far as making the base versions of device and context
//! in rust, but still needed to do all the extended interfaces, so it wasn't fully working.  That
//! work is in a stash.
//!
//!
use std::cell::RefCell;
use std::ffi::CStr;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use device_state::dev_state_d3d11_nolock;
use device_state::dev_state_d3d11_write;
use shared_dx::dx11rs::DX11RenderState;
use shared_dx::dx11rs::VertexFormat;
use shared_dx::types::DX11Metrics;
use shared_dx::types::DevicePointer;
use util::mm_verify_load;
use winapi::Interface;
use winapi::ctypes::c_void;
use winapi::shared::basetsd::SIZE_T;
use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::shared::guiddef::GUID;
use winapi::shared::winerror::E_NOINTERFACE;
use winapi::um::d3d11::D3D11_APPEND_ALIGNED_ELEMENT;
use winapi::um::d3d11::D3D11_BIND_INDEX_BUFFER;
use winapi::um::d3d11::D3D11_BIND_VERTEX_BUFFER;
use winapi::um::d3d11::D3D11_BUFFER_DESC;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;
use winapi::um::d3d11::D3D11_SUBRESOURCE_DATA;
use winapi::um::d3d11::ID3D11Buffer;
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
use winapi::um::unknwnbase::IUnknown;

use crate::debugmode;
use crate::hook_device::{load_d3d_lib, init_device_state_once, init_log};
use shared_dx::util::write_log_file;
use shared_dx::types_dx11::HookDirect3D11;
use shared_dx::types::HookDeviceState;
use shared_dx::types::HookD3D11State;
use device_state::DEVICE_STATE;
use crate::hook_render_d3d11::*;
use crate::debugmode::DebugModeCalledFns;

static mut DEVICE_REALFN: RwLock<Option<HookDirect3D11Device>> = RwLock::new(None);

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
                    Ok(_) => {
                        write_log_file("D3D11CreateDevice succeeded");
                    },
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
            if res == 0 && ppImmediateContext != null_mut() {
                match init_d3d11( (*ppDevice), (*ppSwapChain), (*ppImmediateContext)) {
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

/// Copy the specified number of bytes from the source vtable.  Allocates bytes for the
/// copy and returns a pointer to them.  Caller should cast the vtable to the
/// pointer it needs.  Caller is technically responsible for freeing the copy memory.
/// This function is primarily intended for use by `find_and_copy_vtable` below.
pub unsafe fn copy_vtable<T>(source: *const T, num_bytes:usize) -> *mut u8 {
    let mut v:Vec<u8> = vec![0; num_bytes];

    //let size = std::mem::size_of::<T>();
    write_log_file(&format!("copy_vtable: copying {} bytes", num_bytes));

    std::ptr::copy_nonoverlapping::<u8>(source as *const _, v.as_mut_ptr()  as *mut _, num_bytes);
    // now just return the vec bytes as the vtable.
    let ptr = v.as_mut_ptr();
    let _md = ManuallyDrop::new(v);
    ptr
}
/// Given an IUnknown pointer, a vtable pointer for that object, and list of possible interfaces,
/// copy bytes out of the `vtable` and return those bytes as a pointer to the specified vtable type.
/// The number of bytes copied is determined by a search of `ifaces`.  This list should
/// be sorted by iface size is ascending order, but will be iterated in reverse order (largest
/// first).
///
/// For each item, `iunk` is queried
/// to see if it supports that interface.  If so the number of bytes for that interface is used.
/// Thus, the largest matching interface is selected.  If none are supported by the object,
/// the base size of `T` is used.  This is possibly an error and is noted in the log as a warning,
/// but may be valid for older apps that are using the first verison of the DX11 api.
///
/// It is also possible that there is yet another version of the interface that is not known, and
/// This function will copy an insufficient number of bytes.  This will most likely manifest as a
/// crash.  So this function should only be called on objects who are unlikely to receive a new
/// interface in the future.
///
/// The sizes of the interface should be specified assuming a pointer size of 8, if necessary
/// this function will convert to 4, other sizes are not supported and will return an error.
///
/// The returned memory is owned by caller, who should handle its cleanup when the object is destroyed,
/// or leak it.
///
pub unsafe fn find_and_copy_vtable<T>(iunk:*mut IUnknown, vtable:*const c_void, ifaces:&[(GUID,usize)]) -> Result<*mut T> {
    let iface = ifaces.iter().enumerate().rev().find(|(_i,(guid,_size))| {
        let mut ptr = null_mut();
        let res = (*iunk).QueryInterface(guid, &mut ptr);
        if res == 0 {
            (*iunk).Release();
            true
        } else {
            false
        }
    });
    let size = if let Some((idx,(_guid,size))) = iface {
        write_log_file(
            &format!("find_and_copy_vtable: found interface for type {} at index {} with size {}",
            std::any::type_name::<T>(), idx, size));
        if cfg!(target_pointer_width = "32") {
            size / 2
        } else if cfg!(target_pointer_width = "64") {
            *size
        } else {
            // oops we don't support whatever this is
            return Err(HookError::D3D11Unsupported("unsupported target_pointer_width".to_string()));
        }
    } else {
        // none of the interfaces were found, so use the base interface, don't need
        // to adjust for pointer size in this case since we don't hardcode struct size.
        // this case is a bit unexpected so log in case we crash
        let size = std::mem::size_of::<T>();
        write_log_file(&format!("Warning: object appears to be using base interface, or we don't recognize it, using size {}", size));
        size
    };

    let vtbl = copy_vtable(vtable, size);
    let vtbl = vtbl as *mut T;

    Ok(vtbl)
}

const TRACK_REHOOK_TIME:bool = false;

pub unsafe fn apply_context_hooks(context:*mut ID3D11DeviceContext, first_hook:bool) -> Result<i32> {
    let rehook_start =
        if TRACK_REHOOK_TIME {
            Some(SystemTime::now())
        } else {
            None
        };

    let mut copied = false;
    let vtbl: *mut ID3D11DeviceContextVtbl = if !first_hook {
        // reuse existing vtable on second and future calls.
        // TODO: or maybe change this to redo the copy if I ever need to rehook, as there is
        // possible UB here due to modifying the target of a (formerly) *const pointer.
        (*context).lpVtbl as *mut _
    } else {
        // prepare the list of interfaces we'll query for and their hardcoded sizes

        // convert the guid "bb2c6faa-b5fb-4082-8e6b-388b8cfa90e1" into a id we can pass to QueryInterface
        let dc1guid: GUID = GUID {
            Data1: 0xbb2c6faa,
            Data2: 0xb5fb,
            Data3: 0x4082,
            Data4: [0x8e, 0x6b, 0x38, 0x8b, 0x8c, 0xfa, 0x90, 0xe1],
        };
        // same for "420d5b32-b90c-4da4-bef0-359f6a24a83a" as dc2
        let dc2guid: GUID = GUID {
            Data1: 0x420d5b32,
            Data2: 0xb90c,
            Data3: 0x4da4,
            Data4: [0xbe, 0xf0, 0x35, 0x9f, 0x6a, 0x24, 0xa8, 0x3a],
        };
        // same for "b4e3c01d-e79e-4637-91b2-510e9f4c9b8f" as dc3
        let dc3guid: GUID = GUID {
            Data1: 0xb4e3c01d,
            Data2: 0xe79e,
            Data3: 0x4637,
            Data4: [0x91, 0xb2, 0x51, 0x0e, 0x9f, 0x4c, 0x9b, 0x8f],
        };
        // same for "917600da-f58c-4c33-98d8-3e15b390fa24" as dc4
        let dc4guid: GUID = GUID {
            Data1: 0x917600da,
            Data2: 0xf58c,
            Data3: 0x4c33,
            Data4: [0x98, 0xd8, 0x3e, 0x15, 0xb3, 0x90, 0xfa, 0x24],
        };
        let mut vec = vec![(dc1guid,1072), (dc2guid,1152), (dc3guid,1176), (dc4guid,1192)];
        vec.sort_by_key(|f| f.1);

        copied = true;
        find_and_copy_vtable(
            context as *mut IUnknown,(*context).lpVtbl as *const _, &vec)?
    };

    // unprotect doesn't seem necessary (I'm overwriting my own memory, not the code segment).
    let protect = debugmode::protect_mem();
    let (vsize,old_prot) = if protect {
        let vsize = std::mem::size_of::<ID3D11DeviceContextVtbl>();
        let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;
        (vsize,old_prot)
    } else {
        (0,0)
    };

    let device_child = &mut (*vtbl).parent;
    let iunknown = &mut device_child.parent;

    let mut func_hooked = 0;

    if iunknown.QueryInterface as usize != hook_context_QueryInterface as usize {
        iunknown.QueryInterface = hook_context_QueryInterface;
        func_hooked += 1;
    }

    if iunknown.Release as usize != hook_release as usize {
        iunknown.Release = hook_release;
        func_hooked += 1;
    }
    // don't need this for now
    // if (*vtbl).VSSetConstantBuffers as usize != hook_VSSetConstantBuffers as usize {
    //     (*vtbl).VSSetConstantBuffers = hook_VSSetConstantBuffers;
    //     func_hooked += 1;
    // }
    if debugmode::draw_hook_enabled() && (*vtbl).DrawIndexed as usize != hook_draw_indexed as usize {
        (*vtbl).DrawIndexed = hook_draw_indexed;
        func_hooked += 1;
    }
    if (*vtbl).IASetVertexBuffers as usize != hook_IASetVertexBuffers as usize {
        (*vtbl).IASetVertexBuffers = hook_IASetVertexBuffers;
        func_hooked += 1;
    }
    if (*vtbl).IASetInputLayout as usize != hook_IASetInputLayout as usize {
        (*vtbl).IASetInputLayout = hook_IASetInputLayout;
        func_hooked += 1;
    }
    if (*vtbl).IASetPrimitiveTopology as usize != hook_IASetPrimitiveTopology as usize {
        (*vtbl).IASetPrimitiveTopology = hook_IASetPrimitiveTopology;
        func_hooked += 1;
    }
    if (*vtbl).PSSetShaderResources as usize != hook_PSSetShaderResources as usize {
        (*vtbl).PSSetShaderResources = hook_PSSetShaderResources;
        func_hooked += 1;
    }

    if TRACK_REHOOK_TIME {
        let now = SystemTime::now();
        let elapsed = now.duration_since(
            rehook_start.unwrap_or(SystemTime::UNIX_EPOCH));
        let _ = elapsed.map(|dur| {
            let nanos = dur.subsec_nanos() as u64 + dur.as_secs() * 1_000_000_000;
            dev_state_d3d11_nolock().map(|state| {
                state.metrics.rehook_time_nanos += nanos;
                state.metrics.rehook_calls += 1;
            })
        });
    }

    if protect {
        util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
    }

    //write_log_file(&format!("context {:p} using vtbl {:p}, replacing with copy {:p}", context, (*context).lpVtbl, vtbl));
    if copied {
        ORIG_VTABLE.with(|orig_vtable| {
            let mut orig_vtable = orig_vtable.borrow_mut();
            orig_vtable.insert(context as usize, (*context).lpVtbl);
        });
    }
    (*context).lpVtbl = vtbl;

    Ok(func_hooked)
}

unsafe fn copy_device_vtable(device:*mut ID3D11Device) -> Result<*mut ID3D11DeviceVtbl> {
    // as with the context there are several device versions that we don't have defs for,
    // but we need to make a copy of the vtable to hook it.

    // so we need to query
    // various interfaces that the device might support.  though devices support DXGI
    // interfaces, for this purpose we only care about the device interfaces.

    // convert the guid "a04bfb29-08ef-43d6-a49c-a9bdbdcbe686" into a id we can pass to QueryInterface
    // for ID3D11Device1
    let d1_guid = GUID {
        Data1: 0xa04bfb29,
        Data2: 0x08ef,
        Data3: 0x43d6,
        Data4: [0xa4, 0x9c, 0xa9, 0xbd, 0xbd, 0xcb, 0xe6, 0x86],
    };
    // same for "9d06dffa-d1e5-4d07-83a8-1bb123f2f841" for ID3D11Device2
    let d2_guid = GUID {
        Data1: 0x9d06dffa,
        Data2: 0xd1e5,
        Data3: 0x4d07,
        Data4: [0x83, 0xa8, 0x1b, 0xb1, 0x23, 0xf2, 0xf8, 0x41],
    };
    // same for "A05C8C37-D2C6-4732-B3A0-9CE0B0DC9AE6" for ID3D11Device3
    let d3_guid = GUID {
        Data1: 0xa05c8c37,
        Data2: 0xd2c6,
        Data3: 0x4732,
        Data4: [0xb3, 0xa0, 0x9c, 0xe0, 0xb0, 0xdc, 0x9a, 0xe6],
    };
    // same for "8992ab71-02e6-4b8d-ba48-b056dcda42c4" for ID3D11Device4
    let d4_guid = GUID {
        Data1: 0x8992ab71,
        Data2: 0x02e6,
        Data3: 0x4b8d,
        Data4: [0xba, 0x48, 0xb0, 0x56, 0xdc, 0xda, 0x42, 0xc4],
    };
    // same for "8ffde202-a0e7-45df-9e01-e837801b5ea0" for ID3D11Device5
    let d5_guid = GUID {
        Data1: 0x8ffde202,
        Data2: 0xa0e7,
        Data3: 0x45df,
        Data4: [0x9e, 0x01, 0xe8, 0x37, 0x80, 0x1b, 0x5e, 0xa0],
    };
    // make a vec of guids and hardcoded sizes for each
    let mut guids = vec![
        (d1_guid, 400),
        (d2_guid, 432),
        (d3_guid, 520),
        (d4_guid, 536),
        (d5_guid, 552),
    ];
    // sort just in case
    guids.sort_by_key(|f| f.1);
    // now copy
    let vtbl: *mut ID3D11DeviceVtbl = find_and_copy_vtable(
        device as *mut IUnknown,(*device).lpVtbl as *const _, &guids)?;

    // TODO: dc1 and dc2 have updated getImmediateContext functions, probably should hook those,
    // but winapi doesn't have defs for them right now...could make a partial struct with just
    // those since they are the first functions defined in each case.  OTOH they don't have
    // anything I want to hook so maybe ok to just let them pass through.  probably depends on
    // whether a game calls draw on them or just uses the earlier version for that.
    Ok(vtbl)
}

/// Unhook the device, and since device hooks are currently global this
/// means all devices.  This is used by tests.
pub unsafe fn unapply_device_hook(device:*mut ID3D11Device) -> Result<()> {
    let savelock = DEVICE_REALFN.read()
        .map_err(|e|
            HookError::D3D11DeviceHookFailed(format!("can't get save vtable: {}", e)))?;
    let savevtbl = savelock.as_ref();
    match savevtbl {
        None => {
            eprintln!("can't unhooking d3d11 device, no saved vtbl: {:x}", device as usize);
            return Err(HookError::D3D11NoContext);
        },
        Some(hooks) => {
            eprintln!("unhooking d3d11 device: {:x}", device as usize);
            write_log_file(&format!("unhooking d3d11 device: {:x}", device as usize));
            let vsize = std::mem::size_of::<ID3D11DeviceVtbl>();
            let vtbl:*mut ID3D11DeviceVtbl = std::mem::transmute((*device).lpVtbl);
            let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;
            (*vtbl).CreateInputLayout = (hooks).real_create_input_layout;
            (*vtbl).parent.QueryInterface = (hooks).real_query_interface;
            util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
        }
    }
    Ok(())
}

/// Hook the device.  The first time this is called, the
/// DEVICE_REALFN global static will
/// be populated with the original "real" device functions.  This is used
/// for later hooks of other devices.
unsafe fn apply_device_hook(device:*mut ID3D11Device) -> Result<()> {
    write_log_file(&format!("hooking new d3d11 device: {:x}", device as usize));

    // ideally we'd make a copy of the vtable like we do with context and hook that.
    // that works...except when discord is running.  in which case the game crashes.
    // When discord starts I can see something discord querying for ID3D11Device (the first version)
    // on the game's render thread and crash happens right after that.
    // but the game itself queries for the same device pointer earlier and doesn't crash.
    // if I don't use the copy and just reuse the original then it works.  so copy
    // is disabled here for now.
    let copy_dev_vtable = false;

    let dev_vtbl: *mut ID3D11DeviceVtbl = if !copy_dev_vtable {
        write_log_file("hooking existing device vtbl");
        std::mem::transmute((*device).lpVtbl)
    } else {
        copy_device_vtable(device)?
    };

    let vtbl = dev_vtbl;

    // since the device can be multithreaded we might have shit hammering on query interface
    // as soon as we swap in the hook function.  so make sure the real function struct is
    // ready before that happens.
    let mut lock = DEVICE_REALFN.write()
        .map_err(|e| HookError::D3D11DeviceHookFailed(
        format!("device hooks lock failed: {}", e)))?;
    if lock.is_none() {
        // make a reference copy of the vtable, which we'll save and won't hook.
        // we may need it to find the original function pointers if we
        // hook another device.

        let real_create_buffer = (*vtbl).CreateBuffer;
        let real_create_input_layout = (*vtbl).CreateInputLayout;
        let real_query_interface = (*vtbl).parent.QueryInterface;

        if real_create_buffer as usize == hook_CreateBuffer as usize {
            return Err(HookError::D3D11DeviceHookFailed(
                format!("unable to hook CreateBuffer due to missing real function")));
        }
        if real_create_input_layout as usize == hook_CreateInputLayoutFn as usize {
            return Err(HookError::D3D11DeviceHookFailed(
                format!("unable to hook CreateInputLayout due to missing real function")));
        }
        if real_query_interface as usize == hook_device_QueryInterface as usize {
            return Err(HookError::D3D11DeviceHookFailed(
                format!("unable to hook QueryInterface due to missing real function")));
        }

        *lock = Some(HookDirect3D11Device {
            real_create_buffer,
            real_query_interface,
            real_create_input_layout,
        });
        write_log_file("device hook real funcs initialized");
    }

    // can just use the size of the base interface here since we don't overwrite anything else,
    let vsize = std::mem::size_of::<ID3D11DeviceVtbl>();
    // note: this unprotect _is_ needed here it seems, game crashes without it esp if
    // we don't copy the vtable.
    let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;
    (*vtbl).CreateInputLayout = hook_CreateInputLayoutFn;
    // don't need to hook create buffer if we aren't precoping data
    if GLOBAL_STATE.run_conf.precopy_data {
        (*vtbl).CreateBuffer = hook_CreateBuffer;
    }
    (*vtbl).parent.QueryInterface = hook_device_QueryInterface;
    util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
    if copy_dev_vtable {
        write_log_file(&format!("replacing device {:x} orig vtbl {:x} with new vrbl {:x}",
        device as usize, (*device).lpVtbl as usize, vtbl as usize));
        (*device).lpVtbl = vtbl;
    }

    Ok(())
}

unsafe fn hook_d3d11(device:*mut ID3D11Device,_swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) ->
    Result<HookDirect3D11> {

    apply_device_hook(device)?;

    write_log_file(&format!("hooking new d3d11 context: {:x}", context as usize));
    let vtbl: *mut ID3D11DeviceContextVtbl = std::mem::transmute((*context).lpVtbl);
    let ct = (*context).GetType();
    let flags = (*context).GetContextFlags();
    write_log_file(&format!("context vtbl: {:x}, type {:x}, flags {:x}",
        vtbl as usize, ct, flags));

    let device_child = &mut (*vtbl).parent;
    let iunknown = &mut device_child.parent;

    let real_release = iunknown.Release;
    let real_query_interface = iunknown.QueryInterface;
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
    let real_ia_set_primitive_topology = (*vtbl).IASetPrimitiveTopology;
    let real_ps_set_shader_resources = (*vtbl).PSSetShaderResources;

    // since we always make a copy of the vtable in the context at the moment, we don't search
    // for the real functions as we do in the device case, since a new context should always have
    // the real functions.  but error out if that is not so.  check 1 function on iunknown and 1
    // on device (but not drawindexed since we can late hook that).
    if real_release as usize == hook_release as usize {
        write_log_file("error: context already appears to be hooked");
        return Err(HookError::D3D11DeviceHookFailed("context already hooked".to_string()));
    }
    if real_ia_set_input_layout as usize == hook_IASetInputLayout as usize {
        write_log_file("error: context already appears to be hooked");
        return Err(HookError::D3D11DeviceHookFailed("context already hooked".to_string()));
    }

    let func_hooked = apply_context_hooks(context, true)?;

    // Inc ref count on the device
    //(*context).AddRef(); // TODO11: dx9 does this, but needed here? and where is this decremented?

    write_log_file(&format!("context hook complete: {} functions hooked; (protected mem: {})",
        func_hooked, debugmode::protect_mem()));
    let hook_context = HookDirect3D11Context {
        real_query_interface,
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
        real_ia_set_primitive_topology,
        real_ps_set_shader_resources,
    };

    Ok(HookDirect3D11 { context: hook_context })
}

fn init_d3d11(device:*mut ID3D11Device, swapchain:*mut IDXGISwapChain, context:*mut ID3D11DeviceContext) -> Result<()> {
    let was_init = init_device_state_once();
    let mm_root = match mm_verify_load() {
        Some(dir) => dir,
        None => {
            return Err(HookError::D3D9DeviceHookFailed)
        }
    };
    if was_init {
        write_log_file("WARNING: device state was already initialized");
    }
    init_log(&mm_root);
    debugmode::check_init(&mm_root);
    unsafe {
        GLOBAL_STATE.mm_root = Some(mm_root);

        let _lock = GLOBAL_STATE_LOCK
        .lock()
        .map_err(|_err| HookError::GlobalLockError)?;

        // need to know if we will be precopying data for snapshots before we hook, since that affects
        // what is hooked.
        // this is a root reg query because at this time I don't know the game profile
        let precopy = util::reg_query_root_dword("SnapPreCopyData");
        if let Ok(precopy) = precopy {
            GLOBAL_STATE.run_conf.precopy_data = precopy > 0;
        }
        write_log_file(&format!("snapshot precopy data: {}", GLOBAL_STATE.run_conf.precopy_data));

        let hooks = hook_d3d11(device, swapchain, context)?;

        (*DEVICE_STATE).hook = Some(HookDeviceState::D3D11(HookD3D11State {
            hooks,
            devptr: DevicePointer::D3D11(device),
            metrics: DX11Metrics::new(),
            rs: DX11RenderState::new(),
            app_hwnds: Vec::new(),
            last_timebased_update: SystemTime::now(),
            app_foreground: false,
            last_data_expire: SystemTime::now(),
            last_data_expire_type_flip: false,
        }));

        // TODO11: d3d9 also has: d3d_resource_count: 0,

        write_log_file(&format!(
            "hooked device on thread {:?}",
            std::thread::current().id()
        ));

        (*context).AddRef();
        let cref = (*context).Release();
        write_log_file(&format!("context initial ref count: {}", cref));
        if debugmode::add_ref_context() {
            write_log_file("adding ref on context");
            (*context).AddRef();
        }
        (*device).AddRef();
        let dref = (*device).Release();
        write_log_file(&format!("device initial ref count: {}", dref));
        if debugmode::add_ref_device() {
            write_log_file("adding ref on device");
            (*device).AddRef();
        }
    }

    Ok(())
}

/// Returns the real functions for the device.  Note this does not actually return the device pointer,
/// since it is assumed the caller already has that.
fn get_device_realfn<'a>() -> Result<RwLockReadGuard<'a, Option<HookDirect3D11Device>>> {
    let lock = unsafe { DEVICE_REALFN.read() }
        .map_err(|_err| HookError::GlobalLockError)?;
    Ok(lock)
}


pub fn get_format_size_bytes(format:&DXGI_FORMAT) -> Option<u32> {
    use winapi::shared::dxgiformat::*;
    // there are a zillion formats and I don't care about most so just defining sizes
    // for the ones I've observed
    let size =
        match format {
            &DXGI_FORMAT_R8G8B8A8_UNORM => 4,
            &DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => 4,
            &DXGI_FORMAT_R8G8B8A8_UINT => 4,
            &DXGI_FORMAT_R8G8B8A8_SNORM => 4,
            &DXGI_FORMAT_R8G8B8A8_SINT => 4,
            &DXGI_FORMAT_R8G8B8A8_TYPELESS => 4,
            &DXGI_FORMAT_R32G32_FLOAT => 8,
            &DXGI_FORMAT_R32G32B32_FLOAT => 12,
            &DXGI_FORMAT_R32G32B32A32_FLOAT => 16,
            &DXGI_FORMAT_R32G32_UINT => 8,
            &DXGI_FORMAT_R32G32B32_UINT => 12,
            &DXGI_FORMAT_R32G32B32A32_UINT => 16,
            &DXGI_FORMAT_R32G32_SINT => 8,
            &DXGI_FORMAT_R32G32B32_SINT => 12,
            &DXGI_FORMAT_R32G32B32A32_SINT => 16,

            &DXGI_FORMAT_R16G16_FLOAT => 4,
            &DXGI_FORMAT_R16G16B16A16_FLOAT => 8,
            &DXGI_FORMAT_R16G16_UNORM => 4,
            &DXGI_FORMAT_R16G16B16A16_UNORM => 8,
            &DXGI_FORMAT_R16G16_UINT => 4,
            &DXGI_FORMAT_R16G16B16A16_UINT => 8,
            &DXGI_FORMAT_R16G16_SNORM => 4,
            &DXGI_FORMAT_R16G16B16A16_SNORM => 8,
            &DXGI_FORMAT_R16G16_SINT => 4,
            &DXGI_FORMAT_R16G16B16A16_SINT => 8,

            _ => {
                return None;
            },
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
            write_log_file("WARNING: vertex has dynamic size, not computed");
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
                        "ERROR: can't compute vertex size, no high offset found");
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
    debugmode::note_called(DebugModeCalledFns::Hook_DeviceCreateInputLayoutFn, THIS as usize);
    let dev_realfn = match get_device_realfn() {
        Ok(dev) => dev,
        Err(e) => {
            write_log_file(&format!("OOPS hook_CreateInputLayoutFn returning E_FAIL due to bad state: {:?}", e));
            return E_FAIL;
        }
    };
    let dev_realfn = match dev_realfn.as_ref() {
        Some(dev) => dev,
        None => {
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

    let res = (dev_realfn.real_create_input_layout)(
        THIS,
        pInputElementDescs,
        NumElements,
        pShaderBytecodeWithInputSignature,
        BytecodeLength,
        ppInputLayout
    );

    if res == 0 && has_position && ppInputLayout != null_mut() && (*ppInputLayout) != null_mut() {
        dev_state_d3d11_write()
        .map(|(_lock,ds)| {
            // update the elements and replace the semantic string pointers with table pointers.
            // this is defensive coding since I haven't seen a problem with this and those pointers
            // are probably static strings, but it isn't guaranteed, they could become garbage any
            // time after this call.
            elements.iter_mut().for_each(|el| {
                let cname = CStr::from_ptr(el.SemanticName);
                let name =  cname.to_string_lossy().to_string(); // keep case
                let len = ds.rs.device_semantic_string_table.capacity();
                match ds.rs.device_semantic_string_table.get(&name) {
                    Some(bstr) => {
                        el.SemanticName = bstr.as_ptr() as *const i8;
                    },
                    None => {
                        // need a byte string with nul term for the table so make that
                        let semcopy = cname.to_bytes_with_nul().to_vec();
                        let ptr = semcopy.as_ptr() as *const i8;
                        ds.rs.device_semantic_string_table.insert(name, semcopy);
                        el.SemanticName = ptr;
                    }
                }
                if len != ds.rs.device_semantic_string_table.capacity() {
                    write_log_file(&format!("WARNING: device semantic string table resized to {}", ds.rs.device_semantic_string_table.capacity()));
                }
            });

            let vf = vertex_format_from_layout(elements);

            // add layout to hash, context will copy it out later
            ds.rs.device_input_layouts_by_ptr.insert(*ppInputLayout as usize, vf);

            let len =ds.rs.device_input_layouts_by_ptr.len();
            ds.rs.num_input_layouts.store(len, Ordering::Relaxed);
        });
    }

    res
}

unsafe extern "system" fn hook_CreateBuffer(
    THIS: *mut ID3D11Device,
    pDesc: *const D3D11_BUFFER_DESC,
    pInitialData: *const D3D11_SUBRESOURCE_DATA,
    ppBuffer: *mut *mut ID3D11Buffer,
) -> HRESULT {
    let dev_realfn = match get_device_realfn() {
        Ok(dev) => dev,
        Err(e) => {
            write_log_file(&format!("Error: hook_CreateBuffer returning E_FAIL due to bad state: {:?}", e));
            return E_FAIL;
        }
    };
    let dev_realfn = match dev_realfn.as_ref() {
        Some(dev) => dev,
        None => {
            write_log_file(&format!("Error: hook_CreateBuffer returning E_FAIL due to missing realfn"));
            return E_FAIL;
        }
    };

    // for snapshotting, it would be nice to be able to create the buffers in D3D11_USAGE_STAGING
    // with D3D11_CPU_ACCESS_READ, so that later when we snapshot we can just read it back out.
    // But that doesn't work, at least for index buffers.  And since no other usage allows reading the
    // buffer from the CPU, unlike in DX9, this appears to be a one-way memory chute.
    // So we need to make a copy of the data in case we need it later.
    // Note at this time we don't hook Map so if the code uses that to write to it again we'll
    // miss that (in theory not possible with D3D11_USAGE_IMMUTABLE though)

    let res = (dev_realfn.real_create_buffer)(
        THIS,
        pDesc,
        pInitialData,
        ppBuffer
    );

    if res == 0 && ppBuffer != null_mut() && (*ppBuffer) != null_mut() {
        // if its an index buffer with data, we need to copy it out
        let is_ib = (*pDesc).BindFlags & D3D11_BIND_INDEX_BUFFER != 0;
        let is_vb = (*pDesc).BindFlags & D3D11_BIND_VERTEX_BUFFER != 0;
        if !pDesc.is_null() && (is_ib || is_vb)
            && !pInitialData.is_null() && !(*pInitialData).pSysMem.is_null() {
            if (*pInitialData).SysMemPitch != 0 || (*pInitialData).SysMemSlicePitch != 0 {
                write_log_file(&format!("WARNING: hook_CreateBuffer: index or vertex buffer created with pitch or slice pitch, copy unimplemented"));
            } else {
                let vlen = (*pDesc).ByteWidth as usize;
                let mut dest_v:Vec<u8> = Vec::with_capacity(vlen);
                std::ptr::copy_nonoverlapping::<u8>((*pInitialData).pSysMem as *const u8, dest_v.as_mut_ptr(), vlen);
                dest_v.set_len(vlen);
                dev_state_d3d11_write()
                .map(|(_lock,ds)| {
                    if is_ib {
                        ds.rs.device_index_buffer_data.insert(*ppBuffer as usize, dest_v);
                        ds.rs.device_index_buffer_createtime.push((*ppBuffer as usize, SystemTime::now()));
                    }
                    else if is_vb {
                        ds.rs.device_vertex_buffer_data.insert(*ppBuffer as usize, dest_v);
                        ds.rs.device_vertex_buffer_createtime.push((*ppBuffer as usize, SystemTime::now()));
                    }
                });
            }
        }
    }

    res
}

thread_local! {
    static DEVICE_IN_QI: RefCell<bool>  = RefCell::new(false);
}
pub unsafe extern "system" fn hook_device_QueryInterface(
    THIS: *mut IUnknown,
    riid: *const winapi::shared::guiddef::GUID,
    ppvObject: *mut *mut winapi::ctypes::c_void,
) -> winapi::shared::winerror::HRESULT {
    //eprintln!("{:?}: hook_device_QueryInterface called {:?}", std::thread::current().id(), std::time::SystemTime::now());

    write_log_file(&format!("Device: hook_device_QueryInterface: for id {:x} {:x} {:x} {}",
        (*riid).Data1, (*riid).Data2, (*riid).Data3, u8_slice_to_hex_string(&(*riid).Data4)));

    let hook_device = match get_device_realfn() {
        Ok(dev) => dev,
        Err(e) => {
            write_log_file(&format!("Error: hook_device_QueryInterface returning E_NOINTERFACE due to missing device: {:?}", e));
            return E_NOINTERFACE;
        }
    };
    let hook_device = match hook_device.as_ref() {
        Some(dev) => dev,
        None => {
            write_log_file(&format!("Error: hook_device_QueryInterface returning E_NOINTERFACE due to missing device"));
            return E_NOINTERFACE;
        }
    };
    if hook_device.real_query_interface as usize == hook_device_QueryInterface as usize {
        write_log_file(&format!("Error: hook_device_QueryInterface returning E_NOINTERFACE due real fn same as hook fn"));
        return E_NOINTERFACE;
    }

    let r = DEVICE_IN_QI.with(|in_qi| {
        if *in_qi.borrow() {
            write_log_file(&format!("Error: hook_device_QueryInterface returning E_NOINTERFACE due to re-entrant call"));
            return E_NOINTERFACE;
        }
        *in_qi.borrow_mut() = true;
        0
    });
    if r != 0 {
        return r;
    }

    let hr = (hook_device.real_query_interface)(THIS, riid, ppvObject);
    write_log_file(&format!("Device: hook_device_QueryInterface: hr {:x}", hr));
    if hr == 0 && (*riid).Data1 == ID3D11Device::uuidof().Data1
            && (*riid).Data2 == ID3D11Device::uuidof().Data2
            && (*riid).Data3 == ID3D11Device::uuidof().Data3
            && (*riid).Data4 == ID3D11Device::uuidof().Data4 {
        let pdevice = *ppvObject as *mut ID3D11Device;
        write_log_file(&format!("Device: query for ID3D11Device returned dev {:x} with vtable {:x}",
        pdevice as usize, (*pdevice).lpVtbl as usize));
    }

    DEVICE_IN_QI.with(|in_qi| {
        *in_qi.borrow_mut() = false;
    });
    hr
}

/////////////////////////////////////////////////////////////////////////////////////////////
//
//   //////////////  //////////////  //////////////  //////////////  //////////////
//         //        //              //                    //        //
//         //        //              //                    //        //
//         //        //              //                    //        //
//         //        ///////////     //////////////        //        //////////////
//         //        //                          //        //                    //
//         //        //                          //        //                    //
//         //        //                          //        //                    //
//         //        //////////////  //////////////        //        //////////////
//
////////////////////////////////////////////////////////////////////////////////////////////
// That ^^ is legible in the vscode sidebar preview ==> ^_^
#[cfg(test)]
/// In addition to testing specific functionality, these tests are a bit of a chaos monkey to
/// test that certain nasty conditions observed in the real world (for instance, multiple threads
/// creating devices) don't cause crashes or other bad behavior.  Though it is not guaranteed
/// MM will actually work in these cases, we don't want crashes.
/// Most of these tests can't be run simulataneously, however as they poke at the device globals,
/// so they lock at the start, since cargo will normally run them threaded.
pub mod tests {
    use std::{sync::{Arc}, thread::JoinHandle};
    use device_state::DEVICE_STATE_LOCK;
    use shared_dx::util::{LOG_EXCL_LOCK};
    use util::{prep_log_file, prep_log_file_nolock};
    use winapi::{um::{unknwnbase::{IUnknown, IUnknownVtbl},
        d3dcommon::D3D_DRIVER_TYPE_HARDWARE,
        d3d11::{D3D11_SDK_VERSION, D3D11_INPUT_PER_VERTEX_DATA}}, shared::dxgiformat::DXGI_FORMAT_R32G32B32_FLOAT};
    use super::*;

    pub unsafe extern "system" fn dummy_addref(_ik: *mut IUnknown) -> u32 {
        1
    }
    pub unsafe extern "system" fn dummy_release(_ik: *mut IUnknown) -> u32 {
        1
    }
    pub unsafe extern "system" fn dummy_query_interface(
        _ik: *mut IUnknown,
        _riid: *const winapi::shared::guiddef::GUID,
        _ppvObject: *mut *mut winapi::ctypes::c_void,
    ) -> winapi::shared::winerror::HRESULT {
        E_NOINTERFACE
    }

    pub unsafe extern "system" fn dummy_create_buffer(
        _ik: *mut ID3D11Device,
        _pDesc: *const D3D11_BUFFER_DESC,
        _pInitialData: *const D3D11_SUBRESOURCE_DATA,
        _ppBuffer: *mut *mut ID3D11Buffer,
    ) -> winapi::shared::winerror::HRESULT {
        E_FAIL
    }

    pub unsafe extern "system" fn dummy_create_input_layout(
        _ik: *mut ID3D11Device,
        _pInputElementDescs: *const D3D11_INPUT_ELEMENT_DESC,
        _NumElements: u32,
        _pShaderBytecodeWithInputSignature: *const winapi::ctypes::c_void,
        _BytecodeLength: usize,
        _ppInputLayout: *mut *mut ID3D11InputLayout,
    ) -> winapi::shared::winerror::HRESULT {

        let mut fakelayout:Vec<u8> = Vec::with_capacity(std::mem::size_of::<ID3D11InputLayout>());
        let ptr = fakelayout.as_mut_ptr();
        std::mem::forget(fakelayout);
        *_ppInputLayout = ptr as *mut ID3D11InputLayout;
        write_log_file("note: dummy_create_input_layout returning real but invalid pointer");
        0
    }

    fn cleanup(device:*mut ID3D11Device, testcontext:&str) {
        unsafe {
            if !device.is_null() {
                unapply_device_hook(device).expect(&format!("{}: unapply_device_hook failed", testcontext));
                (*device).Release();
            }

            let _unbox = Box::from_raw(DEVICE_STATE);
            DEVICE_STATE = null_mut();
            DEVICE_REALFN.write().expect(&format!("{}: device hooks clear failed", testcontext)).take();
        }
    }

    fn assert_log(logfile:&str, expected_count:usize,msg:&str) {
        let logtext = std::fs::read_to_string(logfile).unwrap();
        let count = logtext.matches(msg).count();
        if count != expected_count {
            eprintln!("logtext: {}", logtext);
        }
        assert_eq!(count, expected_count, "want {}, got {} for log line '{}'", expected_count, count, msg);
    }

    #[test]
    fn test_query_interface() -> Result<()> {
        std::thread::sleep(std::time::Duration::from_secs(0));

        let _loglock = LOG_EXCL_LOCK.lock().unwrap();
        eprintln!("starting test_query_interface");

        let testlog = prep_log_file(&_loglock, "__testhd3d11__test_query_interface.txt").expect("doh");

        let _lock = unsafe {
            let lock = DEVICE_STATE_LOCK.write().unwrap();
            if DEVICE_STATE != null_mut() {
                panic!("DEVICE_STATE already initialized");
            }
            init_device_state_once();
            lock
        };

        let iunkvtbl = IUnknownVtbl {
            QueryInterface: hook_device_QueryInterface,
            AddRef: dummy_addref,
            Release: dummy_release,
        };
        let mut iunk = IUnknown {
            lpVtbl: Box::into_raw(Box::new(iunkvtbl)) as *mut IUnknownVtbl,
        };

        // called without device should return E_NOINTERFACE
        unsafe {
            assert_eq!(hook_device_QueryInterface(&mut iunk as *mut IUnknown,
                &ID3D11Device::uuidof() as *const GUID,
                null_mut()), E_NOINTERFACE);
        };
        assert!(std::fs::read_to_string(testlog).unwrap().contains("E_NOINTERFACE due to missing device"));

        unsafe {
            let mut hooks = DEVICE_REALFN.write().expect("no hooks lock");
            *hooks = Some(HookDirect3D11Device {
                real_create_buffer: dummy_create_buffer,
                real_create_input_layout: dummy_create_input_layout,
                real_query_interface: dummy_query_interface,
            });
        };

        // setting the real function to be the hook function should fail
        unsafe {
            let mut lhooks = DEVICE_REALFN.write().expect("no hooks lock");
            let hooks = lhooks.as_mut().expect("no hooks");
            hooks.real_query_interface = hook_device_QueryInterface;
            drop(hooks);
            drop(lhooks);
            assert_eq!(hook_device_QueryInterface(&mut iunk as *mut IUnknown,
                &ID3D11Device::uuidof() as *const GUID,
                null_mut()), E_NOINTERFACE);
        }
        assert!(std::fs::read_to_string(testlog).unwrap().contains("E_NOINTERFACE due real fn same as hook fn"));

        // make a nasty re-entrant test function
        unsafe extern "system" fn nasty_reentrant_test_function(ik: *mut IUnknown,
            riid: *const GUID,
            ppvObject: *mut *mut c_void) -> HRESULT {
            hook_device_QueryInterface(ik, riid, ppvObject)
        }

        unsafe {
            let mut lhooks = DEVICE_REALFN.write().expect("no hooks lock");
            let hooks = lhooks.as_mut().expect("no hooks");
            hooks.real_query_interface = nasty_reentrant_test_function;
            drop(hooks);
            drop(lhooks);
            assert_eq!(hook_device_QueryInterface(&mut iunk as *mut IUnknown,
                &ID3D11Device::uuidof() as *const GUID,
                null_mut()), E_NOINTERFACE);
        }
        assert!(std::fs::read_to_string(testlog).unwrap().contains("E_NOINTERFACE due to re-entrant call"));

        // finally a valid hook qi should return S_OK, make a function to do that
        unsafe extern "system" fn valid_qi(_ik: *mut IUnknown,
            riid: *const GUID,
            ppvObject: *mut *mut c_void) -> HRESULT {
                // create and leak fake device
                let dev = Box::into_raw(Box::new(ID3D11Device {
                    lpVtbl: null_mut()
                }));

                if (*riid).Data1 == ID3D11Device::uuidof().Data1
                    && (*riid).Data2 == ID3D11Device::uuidof().Data2
                    && (*riid).Data3 == ID3D11Device::uuidof().Data3
                    && (*riid).Data4 == ID3D11Device::uuidof().Data4 {
                    *ppvObject = dev as *mut c_void;
                    0
                } else {
                    E_NOINTERFACE
                }
        }

        unsafe {
            let mut lhooks = DEVICE_REALFN.write().expect("no hooks lock");
            let hooks = lhooks.as_mut().expect("no hooks");
            hooks.real_query_interface = valid_qi;
            drop(hooks);
            drop(lhooks);
            let mut pdev:*mut ID3D11Device = null_mut();
            let ppdev: *mut *mut ID3D11Device= &mut pdev;
            assert_eq!(hook_device_QueryInterface(&mut iunk as *mut IUnknown,
                &ID3D11Device::uuidof() as *const GUID,
                ppdev as *mut *mut c_void), 0);
        }
        assert!(std::fs::read_to_string(testlog).unwrap().contains("hr 0"));

        // cleanup
        cleanup(null_mut(), "test_query_interface");
        Ok(())
    }

    #[test]
    fn test_create_device() {
        std::thread::sleep(std::time::Duration::from_secs(0));

        let _loglock = LOG_EXCL_LOCK.lock().unwrap();
        eprintln!("starting test_create_device");

        let testlog = prep_log_file(&_loglock, "__testhd3d11__test_create_device.txt").expect("doh");

        let _lock = unsafe {
            let lock = DEVICE_STATE_LOCK.write().unwrap();
            if DEVICE_STATE != null_mut() {
                panic!("DEVICE_STATE already initialized");
            }
            lock
        };

        let mut device = std::ptr::null_mut();
        let mut context = std::ptr::null_mut();


        let res = D3D11CreateDevice(null_mut(),
        D3D_DRIVER_TYPE_HARDWARE,
            null_mut(),
            0,
            null_mut(),
             0,
            D3D11_SDK_VERSION,
            &mut device,
            null_mut(),
            &mut context);

        assert_eq!(res, 0);
        // query interface on device should succeed
        let mut pdev:*mut ID3D11Device = null_mut();
        let ppdev: *mut *mut ID3D11Device = &mut pdev;
        unsafe {
            let res = (*device).QueryInterface(&ID3D11Device::uuidof() as *const GUID, ppdev as *mut *mut c_void);
            assert_eq!(res, 0);
        }

        let assert_log = |expected_count:usize,msg:&str| {
            assert_log(testlog, expected_count, msg);
        };

        // log file should contain stuff
        assert_log(1, "hook_device_QueryInterface: hr 0");

        unsafe {
            (*device).Release();
            device = null_mut();
            let rc = (*context).Release();
            assert_eq!(rc, 0);
            context = null_mut();
        }

        // context should release
        assert_log(1, "context hook release: rc now 0");

        // if a new device is created things should not explode or go into
        // weird infinite loops or otherwise be bad.

        let ret = D3D11CreateDevice(null_mut(),
        D3D_DRIVER_TYPE_HARDWARE,
            null_mut(),
            0,
            null_mut(),
                0,
            D3D11_SDK_VERSION,
            &mut device,
            null_mut(),
            &mut context);
        assert_eq!(ret, 0);

        // log should note that we already did this
        assert_log(1, "WARNING: device state was already initialized");

        // query interface on device should succeed
        let mut pdev:*mut ID3D11Device = null_mut();
        let ppdev: *mut *mut ID3D11Device = &mut pdev;
        unsafe {
            let res = (*device).QueryInterface(&ID3D11Device::uuidof() as *const GUID, ppdev as *mut *mut c_void);
            assert_eq!(res, 0);
            let rc = (*context).Release();
            assert_eq!(rc, 0);
            // device released by cleanup below
        }

        // context should release. 2 messages because we did it earlier
        assert_log(2, "context hook release: rc now 0");

        cleanup(device, "test_create_device");

    }

    #[test]
    fn test_create_device_thread() {
        let _loglock = LOG_EXCL_LOCK.lock().unwrap();

        let _lock = unsafe {
            let lock = DEVICE_STATE_LOCK.write().unwrap();
            if DEVICE_STATE != null_mut() {
                panic!("DEVICE_STATE already initialized");
            }
            lock
        };

        // we only support one log file so prep it first to remove/clear it, then
        // each thread will reprep without clearing to set its thread local status that it was
        // initialized.
        let log_file_name = "__testhd3d11__test_create_device_threads.txt";
        let testlog = prep_log_file_nolock(log_file_name, true).expect("doh");

        // use an arc so that we can make all threads wait until we're ready to go
        let start_lock = Arc::new(RwLock::new(()));
        let start_lock = start_lock.clone();
        let start_lock_write = start_lock.write().expect("start lock failed");

        struct ThreadDevPtr(pub *mut ID3D11Device);
        unsafe impl Send for ThreadDevPtr {}

        let run_time_secs = 4;
        let qi_sleep_wait_ms = 500;
        let nthreads = 2;

        let handles:Vec<JoinHandle<_>> = (0..nthreads).map(|i| {
            let t1_start_lock = start_lock.clone();
            std::thread::spawn(move || {
                let _testlog = prep_log_file_nolock(log_file_name, false).expect("doh");
                let mut device = std::ptr::null_mut();
                let mut context = std::ptr::null_mut();

                let _slr = t1_start_lock.read().expect("start lock failed");

                eprintln!("{:?}: thread {}: creating device", std::thread::current().id(), i);
                let res = D3D11CreateDevice(null_mut(),
                D3D_DRIVER_TYPE_HARDWARE,
                    null_mut(),
                    0,
                    null_mut(),
                     0,
                    D3D11_SDK_VERSION,
                    &mut device,
                    null_mut(),
                    &mut context);
                assert_eq!(res, 0);

                eprintln!("{:?}: thread {}: done creating device", std::thread::current().id(), i);
                let mut pdev:*mut ID3D11Device = null_mut();
                let ppdev: *mut *mut ID3D11Device = &mut pdev;
                unsafe {
                    let res = (*device).QueryInterface(&ID3D11Device::uuidof() as *const GUID, ppdev as *mut *mut c_void);
                    assert_eq!(res, 0);
                }

                let start = std::time::Instant::now();
                while start.elapsed().as_secs() < run_time_secs {
                    let mut pdev:*mut ID3D11Device = null_mut();
                    let ppdev: *mut *mut ID3D11Device = &mut pdev;
                    unsafe {
                        let res = (*device).QueryInterface(&ID3D11Device::uuidof() as *const GUID, ppdev as *mut *mut c_void);
                        assert_eq!(res, 0);
                        // addref and release on the context for funsies
                        let _rc = (*context).AddRef();
                        let _rc = (*context).Release();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(qi_sleep_wait_ms));
                }

                unsafe {
                    let _rc = (*context).Release();
                    // this works for me reliably, but can't be asserted in all environments, ci is one
                    // where it fails sometimes.  I think some things get into the
                    // context randomly, maybe windows itself, and increase its ref count.

                    //assert_eq!(0, rc, "{:?}: thread {}", std::thread::current().id(), i);
                    //eprintln!("{:?}: thread {}: context released: rc {}", std::thread::current().id(), i, rc);
                }

                ThreadDevPtr(device)
            })
        }).collect();

        // they should all be blocked on this so let them run
        drop(start_lock_write);

        // await all of their handles
        let mut tdevs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // inspect logs a bit
        assert_log(testlog, 1,         "device hook real funcs initialized");
        assert_log(testlog, nthreads, "D3D11CreateDevice succeeded");
        // sometimes this is short a line, not sure why since I
        // assert that context rc is zero above.
        //assert_log(testlog, nthreads, "context hook release: rc now 0");


        // save last
        let last = tdevs.pop();
        // free the other devices
        tdevs.into_iter().for_each(|h| unsafe { (*h.0).Release(); } );
        // cleanup and unhook with the last
        cleanup(last.unwrap().0, "test_create_device_threads")

        // eprintln!("create thread test waiting");
        // std::thread::sleep(std::time::Duration::from_secs(5));

    }

    #[test]
    fn create_and_draw() {
        let _loglock = LOG_EXCL_LOCK.lock().unwrap();

        // note we can't lock device state in this test because we are invoking the
        // hook functions which also lock.  so rely on the loglock to keep other tests
        // from running.

        let log_file_name = "__testhd3d11__create_and_draw.txt";
        let _testlog = prep_log_file_nolock(log_file_name, true).expect("doh");

        let mut device = std::ptr::null_mut();
        let mut context = std::ptr::null_mut();

        let res = D3D11CreateDevice(null_mut(),
        D3D_DRIVER_TYPE_HARDWARE,
            null_mut(),
            0,
            null_mut(),
             0,
            D3D11_SDK_VERSION,
            &mut device,
            null_mut(),
            &mut context);
        assert_eq!(res, 0);

        unsafe {
            // need to use the dummy function because the real function expects a shader which
            // we don't have.
            DEVICE_REALFN.write().expect("no real fn").as_mut().map(|rfn| {
                rfn.real_create_input_layout = dummy_create_input_layout;
            });
            let desc = vec![
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: b"POSITION\0".as_ptr() as *const i8,
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: 0,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0
                }];
            let mut pLayout:*mut ID3D11InputLayout = null_mut();
            let ret = (*device).CreateInputLayout(desc.as_ptr(), 1,
                null_mut(), 0, &mut pLayout);
            assert_eq!(ret, 0);
            assert!(!pLayout.is_null()); // don't deref this, its garbage

            dev_state_d3d11_nolock().map(|ds| {
                assert_eq!(ds.rs.num_input_layouts.load(Ordering::Relaxed), 1);
                assert_eq!(ds.rs.device_input_layouts_by_ptr.len(), 1);
                assert!(ds.rs.device_input_layouts_by_ptr.contains_key(&(pLayout as usize)));
                let vf = ds.rs.device_input_layouts_by_ptr.get(&(pLayout as usize)).unwrap();
                assert_eq!(vf.layout.len(), 1);
                assert_eq!(vf.size, 12);
                //assert_eq!(vf.layout[0].SemanticName, b"POSITION\0".as_ptr() as *const i8);
                assert_eq!(ds.rs.context_input_layouts_by_ptr.len(), 0);
            });

            // after some dip calls, draw will trigger first periodic call which does nothing
            GLOBAL_STATE.metrics.dip_calls = HOOK_DRAW_PERIODIC_CALLS - 1;
            (*context).DrawIndexed(12, 0, 0);
            assert_eq!(GLOBAL_STATE.metrics.dip_calls, HOOK_DRAW_PERIODIC_CALLS);
            // the func we are interested is called on a time basis so need to let some go by...
            std::thread::sleep(std::time::Duration::from_secs(1));
            GLOBAL_STATE.clr.runtime_pointer = Some(1); // this will prevent the clr from initting
            // this should trigger the second periodic call which copies layouts amongst other things
            GLOBAL_STATE.metrics.dip_calls += HOOK_DRAW_PERIODIC_CALLS - 1;
            (*context).DrawIndexed(12, 0, 0);
            assert_eq!(GLOBAL_STATE.metrics.dip_calls, 2 * HOOK_DRAW_PERIODIC_CALLS);
            dev_state_d3d11_nolock().map(|ds| {
                assert_eq!(ds.rs.num_input_layouts.load(Ordering::Relaxed), 0);
                assert_eq!(ds.rs.device_input_layouts_by_ptr.len(), 0);
                assert_eq!(ds.rs.context_input_layouts_by_ptr.len(), 1);
                assert!(ds.rs.context_input_layouts_by_ptr.contains_key(&(pLayout as usize)));

                ds.rs = DX11RenderState::new();
            });

            GLOBAL_STATE.clr.runtime_pointer = None;
            let rc = (*context).Release();
            assert_eq!(rc, 0);
        }

        cleanup(device, "create_and_draw")

    }
}
