use snaplib::snap_config::SnapConfig;
use util;
use std::{self, path::{PathBuf}};
use winapi::shared::minwindef::HINSTANCE;
use shared_dx::{error::*, util::write_log_file};

use winapi::um::winnt::LPCSTR;

use crate::SNAP_CONFIG;
type GetActivePlayerTransformFn = unsafe extern "system" fn() -> LPCSTR;

pub struct XDLLState {
    pub _handle: HINSTANCE,
    pub GetActivePlayerTransform: GetActivePlayerTransformFn,
}

impl XDLLState {
    pub fn get_player_transform(&self) -> Result<String> {
        unsafe {
            let xfrm = (self.GetActivePlayerTransform)();
            let ret = std::ffi::CStr::from_ptr(xfrm);
            let ret = ret.to_string_lossy();
            if ret.starts_with("error") {
                return Err(HookError::SnapshotFailed(format!("failed to get player transform: {:?}", ret)));
            }
            Ok(ret.into_owned())
        }
    }
}

pub static mut XDLLSTATE : Option<XDLLState> = None;

pub unsafe fn init_xdll() -> Result<()> {
    if XDLLSTATE.is_some() {
        return Ok(())
    }
    let snap_conf =
        match SNAP_CONFIG.read() {
            Err(e) => {
                write_log_file(&format!("failed to lock snap config: {}", e));
                SnapConfig::new()
            },
            Ok(c) => c.clone()
        };
    if snap_conf.extdll_path.trim().is_empty() {
        return Ok(())
    }
    let p = PathBuf::from(&snap_conf.extdll_path);
    if !p.exists() || !p.is_file() {
        return Err(HookError::SnapshotFailed(
            format!("error: snap extdll '{}' does not exist or is not a file: {:?}", &snap_conf.extdll_path, p)));
    }

    write_log_file(&format!("loading snap extdll: {}", snap_conf.extdll_path));
    let handle = util::load_lib(&snap_conf.extdll_path)?;
    //let handle = GetModuleHandleW(std::ptr::null_mut());
    write_log_file("loading snap extdll fns");
    // Note: leaks handle on error
    let ptransform:GetActivePlayerTransformFn = std::mem::transmute(util::get_proc_address(handle, "GetActivePlayerTransform")?);
    XDLLSTATE = Some(XDLLState{
        _handle: handle,
        GetActivePlayerTransform: ptransform
    });
    Ok(())
}

// We don't unload currently
#[allow(dead_code)]
unsafe fn unload_xdll() -> Result<()> {
    if !XDLLSTATE.is_some() {
        return Ok(())
    }
    let state = XDLLSTATE.take().unwrap();
    util::unload_lib(state._handle)?;
    Ok(())
}
