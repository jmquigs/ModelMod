pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use util;

use std;

use shared_dx9::util::*;
use shared_dx9::error::*;

//type GWToolboxVersionFn = unsafe extern "system" fn() -> LPCSTR;
pub use winapi::um::winnt::{LPCSTR};
type GetActivePlayerTransformFn = unsafe extern "system" fn() -> LPCSTR;


pub struct TBState {
    pub handle: shared_dx9::defs::HINSTANCE,
    pub GetActivePlayerTransform: GetActivePlayerTransformFn,
}

impl TBState {
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

pub static mut TBSTATE : Option<TBState> = None;

pub unsafe fn init_toolbox() -> Result<()> {
    if TBSTATE.is_some() {
        return Ok(())
    }

    write_log_file("loading toolbox");
    let handle = util::load_lib(r#"P:\GWToolboxpp\bin\Debug\GWToolboxdll.dll"#)?;
    //let handle = GetModuleHandleW(std::ptr::null_mut());
    write_log_file("loading gwtb func");
    // TODO: leaks handle on error
    let ptransform:GetActivePlayerTransformFn = std::mem::transmute(util::get_proc_address(handle, "GetActivePlayerTransform")?);
    TBSTATE = Some(TBState{
        handle: handle,
        GetActivePlayerTransform: ptransform
    });
    Ok(())
}

unsafe fn unload_toolbox() -> Result<()> {
    if !TBSTATE.is_some() {
        return Ok(())
    }
    let state = TBSTATE.take().unwrap();
    util::unload_lib(state.handle)?;
    Ok(())
}
