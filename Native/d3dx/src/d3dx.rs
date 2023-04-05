use shared_dx::error::*;
use shared_dx::defs_dx9::*;
use shared_dx::types::D3D11Tex;
use shared_dx::types::DevicePointer;

use global_state::{ GLOBAL_STATE };
use shared_dx::types::TexPtr;
use winapi::um::d3d11::ID3D11Device;
use winapi::um::d3d11::ID3D11Resource;
use std::ptr::null_mut;
use shared_dx::util::ReleaseOnDrop;

use types::d3dx::*;

/// For use when calling this from a lib that doesn't have shared dx or types.
pub fn deviceptr_from_d3d11(ptr:*mut ID3D11Device) -> Option<DevicePointer> {
    if ptr.is_null() {
        return None;
    }
    Some(DevicePointer::D3D11(ptr))
}

/// call load lib and store the lib in global state
pub fn load_and_set_in_gs(mm_root: &Option<String>, device: &DevicePointer) -> Result<()> {
    let d3dx_fn = load_lib(mm_root, device)?;
    unsafe { GLOBAL_STATE.d3dx_fn = Some(d3dx_fn); };
    Ok(())
}

pub fn load_lib(mm_root: &Option<String>, device: &DevicePointer) -> Result<D3DXFn> {
    let mm_root = mm_root.as_ref().ok_or(HookError::LoadLibFailed(
        "No MMRoot, can't load D3DX".to_owned(),
    ))?;
    let mut path = mm_root.to_owned();
    path.push('\\');
    path.push_str("TPLib");
    path.push('\\');

    match device {
        DevicePointer::D3D9(_device) => {
            if cfg!(target_pointer_width = "64") {
                path.push_str("D3DX9_43_x86_64.dll");
            } else {
                path.push_str("D3DX9_43_x86.dll");
            }

            let handle = util::load_lib(&path)?;

            unsafe {
                Ok(D3DXFn::DX9(D3DX9Fn {
                    D3DXSaveTextureToFileW: std::mem::transmute(util::get_proc_address(
                        handle,
                        "D3DXSaveTextureToFileW",
                    )?),
                    D3DXCreateTextureFromFileW: std::mem::transmute(util::get_proc_address(
                        handle,
                        "D3DXCreateTextureFromFileW",
                    )?),
                    D3DXDisassembleShader: std::mem::transmute(util::get_proc_address(
                        handle,
                        "D3DXDisassembleShader",
                    )?),
                }))
            }
        },
        DevicePointer::D3D11(_device) => {
            let base_names = vec!["d3dx11_43.dll", "d3dx11_42.dll"];
            // just try loading it first from the system
            let mut handle = base_names.iter().find_map(|base_name| {
                match util::load_lib(base_name) {
                    Ok(handle) => Some(handle),
                    Err(_) => None,
                }
            });
            let mut searched:Vec<String> = vec![];
            if handle.is_none() {
                // not found so look into tplib dir, try looking for arch specific folder
                // or file with an _arch suffix in tplib
                let arch = if cfg!(target_pointer_width = "64") {
                    "x86_64"
                } else {
                    "x86"
                };

                let path = path.clone();
                handle = base_names.iter().find_map(|base_name| {
                    let mut path = path.clone();
                    path.push_str(arch);
                    path.push('\\');
                    path.push_str(base_name);
                    searched.push(path.clone());
                    match util::load_lib(&path) {
                        Ok(handle) => Some(handle),
                        Err(_) => None,
                    }
                });

                // not found in folders so try again appending the arch to base name
                if handle.is_none(){
                    handle = base_names.iter().find_map(|base_name| {
                        let mut path = path.clone();
                        let base_name = base_name.replace(".dll", &format!("_{}.dll", arch));
                        path.push_str(&base_name);
                        searched.push(path.clone());
                        match util::load_lib(&path) {
                            Ok(handle) => Some(handle),
                            Err(_) => None,
                        }
                    });
                }
            }

            match handle {
                None => return Err(HookError::LoadLibFailed(format!("D3DX11 not found in system or {:?}", searched))),
                Some(handle) => {
                    unsafe {
                        Ok(D3DXFn::DX11(D3DX11Fn {
                            D3DX11SaveTextureToFileW: std::mem::transmute(util::get_proc_address(
                                handle,
                                "D3DX11SaveTextureToFileW",
                            )?),
                            D3DX11CreateTextureFromFileW: std::mem::transmute(util::get_proc_address(
                                handle,
                                "D3DX11CreateTextureFromFileW",
                            )?),
                            // D3DX11DisassembleShader: std::mem::transmute(util::get_proc_address(
                            //     handle,
                            //     "D3DX11DisassembleShader",
                            // )?),
                        }))
                    }
                }
            }
        },
    }
}

pub unsafe fn load_texture(device:DevicePointer, path:*const u16) -> Result<TexPtr> {
    let d3dx_fn = GLOBAL_STATE
        .d3dx_fn
        .as_ref()
        .ok_or(HookError::SnapshotFailed("d3dx not found".to_owned()))?;

    match (device,d3dx_fn) {
        (DevicePointer::D3D9(device), D3DXFn::DX9(d3dx_fn)) => {
            let mut tex: LPDIRECT3DTEXTURE9 = null_mut();
            let ptext: *mut LPDIRECT3DTEXTURE9 = &mut tex;
            let hr = (d3dx_fn.D3DXCreateTextureFromFileW)(device, path, ptext);
            if hr != 0 {
                return Err(HookError::SnapshotFailed("failed to create texture from path".to_owned()));
            }

            Ok(TexPtr::D3D9(tex))
        },
        (DevicePointer::D3D11(device), D3DXFn::DX11(d3dx_fn)) => {
            let mut tex: *mut ID3D11Resource = null_mut();
            let ptext: *mut *mut ID3D11Resource = &mut tex;
            let hr = (d3dx_fn.D3DX11CreateTextureFromFileW)(device, path, null_mut(), null_mut(), ptext, null_mut());
            if hr != 0 {
                return Err(HookError::SnapshotFailed("failed to create texture from path".to_owned()));
            }

            Ok(TexPtr::D3D11(D3D11Tex::Tex(tex)))
        },
        _ => Err(HookError::SnapshotFailed("d3dx device/fn mismatch".to_owned())),
    }
}

pub unsafe fn save_texture(idx: i32, path: *const u16) -> Result<()> {
    const D3DXIFF_DDS: i32 = 4;

    let d3dx_fn = GLOBAL_STATE
        .d3dx_fn
        .as_ref()
        .ok_or(HookError::SnapshotFailed("d3dx not found".to_owned()))?;

    let device_ptr = GLOBAL_STATE
        .device
        .as_ref()
        .ok_or(HookError::SnapshotFailed("device not found".to_owned()))?;

    match device_ptr {
        DevicePointer::D3D9(device) => {
            let mut tex: *mut IDirect3DBaseTexture9 = null_mut();

            let hr = (**device).GetTexture(idx as u32, &mut tex);
            if hr != 0 {
                return Err(HookError::SnapshotFailed(format!(
                    "failed to get texture on stage {} for snapshotting: {:x}",
                    idx, hr
                )));
            }
            let _tex_rod = ReleaseOnDrop::new(tex);
            if tex as usize == GLOBAL_STATE.selection_texture.as_ref().map(|t| t.as_usize()).unwrap_or(0) {
                return Err(HookError::SnapshotFailed(format!(
                    "not snapshotting texture on stage {} because it is the selection texture",
                    idx
                )));
            }

            match d3dx_fn {
                D3DXFn::DX9(d3dx_fn) => {
                    let hr = (d3dx_fn.D3DXSaveTextureToFileW)(path, D3DXIFF_DDS, tex, null_mut());
                    if hr != 0 {
                        return Err(HookError::SnapshotFailed(format!(
                            "failed to save snapshot texture on stage {}: {:x}",
                            idx, hr
                        )));
                    }
                    Ok(())
                },
                _ => Err(HookError::SnapshotFailed("d3dx fn not found".to_owned())),
            }
        },
        DevicePointer::D3D11(_device) => {
            return Err(HookError::SnapshotFailed("d3dx11 save texture not yet implemented".to_owned()));
        }
    }

}
