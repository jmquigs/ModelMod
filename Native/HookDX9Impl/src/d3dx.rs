use shared_dx9::error::*;
use shared_dx9::defs::*;
use util;
use hookd3d9::{ GLOBAL_STATE };
use std::ptr::null_mut;
use util::ReleaseOnDrop;

pub struct D3DXFn {
    pub D3DXSaveTextureToFileW: D3DXSaveTextureToFileWFn,
    pub D3DXCreateTextureFromFileW: u64,
    pub D3DXDisassembleShader: D3DXDisassembleShaderFn,
}

pub fn load_lib(mm_root: &Option<String>) -> Result<D3DXFn> {
    // TODO: decide on where to load these from.
    let mm_root = mm_root.as_ref().ok_or(HookError::LoadLibFailed(
        "No MMRoot, can't load D3DX".to_owned(),
    ))?;
    let mut path = mm_root.to_owned();
    path.push_str("\\");
    path.push_str("TPLib");
    path.push_str("\\");
    if cfg!(target_pointer_width = "64") {
        path.push_str("D3DX9_43_x86_64.dll");
    } else {
        path.push_str("D3DX9_43_x86.dll");
    }

    let handle = util::load_lib(&path)?;

    unsafe {
        Ok(D3DXFn {
            D3DXSaveTextureToFileW: std::mem::transmute(util::get_proc_address(
                handle,
                "D3DXSaveTextureToFileW",
            )?),
            D3DXCreateTextureFromFileW: util::get_proc_address(
                handle,
                "D3DXCreateTextureFromFileW",
            )? as u64,
            D3DXDisassembleShader: std::mem::transmute(util::get_proc_address(
                handle,
                "D3DXDisassembleShader",
            )?),
        })
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

    let mut tex: *mut IDirect3DBaseTexture9 = null_mut();

    let hr = (*(*device_ptr)).GetTexture(idx as u32, &mut tex);
    if hr != 0 {
        return Err(HookError::SnapshotFailed(format!(
            "failed to get texture on stage {} for snapshotting: {:x}",
            idx, hr
        )));
    }
    let _tex_rod = ReleaseOnDrop::new(tex);
    if tex as u64 == GLOBAL_STATE.selection_texture as u64 {
        return Err(HookError::SnapshotFailed(format!(
            "not snapshotting texture on stage {} because it is the selection texture",
            idx
        )));
    }

    let hr = (d3dx_fn.D3DXSaveTextureToFileW)(path, D3DXIFF_DDS, tex, null_mut());
    if hr != 0 {
        return Err(HookError::SnapshotFailed(format!(
            "failed to save snapshot texture on stage {}: {:x}",
            idx, hr
        )));
    }

    Ok(())
}
