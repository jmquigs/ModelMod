use shared_dx::defs::LPDIRECT3DDEVICE9;
use shared_dx::error::Result;
use shared_dx::error::HookError;
use snaplib::anim_frame::AnimFrame;

use util;

const PLUGIN_VER:u32 = 1;

#[repr(C)] pub struct FrameCaptureState { private: [u8; 0] }

#[repr(C)]
#[derive(Debug)]
pub enum PluginError {
    FailedToCaptureState(String),
    FailedToProcessState(String),
}

impl std::convert::From<PluginError> for HookError {
    fn from(error: PluginError) -> Self {
        HookError::SnapshotPluginError(format!("{:?}", error))
    }
}

pub type GetVersionFn = extern "C" fn () -> u32;
pub type InitFn = extern "C" fn () -> std::result::Result<(), PluginError>;
pub type AnimFrameCaptureFn = extern "C" fn (device: LPDIRECT3DDEVICE9) -> std::result::Result<*mut FrameCaptureState, PluginError>;
pub type AnimFrameProcessFn = extern "C" fn (cap_state: *mut FrameCaptureState, frame: *mut AnimFrame, fsize:u32) -> std::result::Result<(), PluginError>;

pub struct SnapPlugin {
    capture_fn:AnimFrameCaptureFn,
    process_fn:AnimFrameProcessFn,
}
impl SnapPlugin {
    fn anim_frame_capture(&mut self, device: LPDIRECT3DDEVICE9) -> Result<*mut FrameCaptureState> {
        let res = unsafe { (self.capture_fn)(device) }?;
        Ok(res)
    }
    fn anim_frame_process(&mut self, cap_state: *mut FrameCaptureState, frame: &mut AnimFrame) -> Result<()> {
        let fsize:u32 = std::mem::size_of::<AnimFrame>() as u32;
        let res = unsafe { (self.process_fn)(cap_state, frame, fsize) } ?;
        Ok(res)
    }
}
pub fn load(path:&str) -> Result<SnapPlugin> {
    let h = util::load_lib(path)?;

    let load = || {
        unsafe {
            let getver:GetVersionFn = std::mem::transmute(util::get_proc_address(h, "get_version")?);
            let ver = getver();
            if ver != PLUGIN_VER {
                return Err(HookError::SnapshotFailed(format!("Can't load plugin {}: has old version {}, we need {}", path, ver, PLUGIN_VER)));
            }
            // all functions must be available before we call init
            let capture_fn:AnimFrameCaptureFn = std::mem::transmute(util::get_proc_address(h, "anim_frame_capture")?);
            let process_fn:AnimFrameProcessFn = std::mem::transmute(util::get_proc_address(h, "anim_frame_process")?);

            let init:InitFn = std::mem::transmute(util::get_proc_address(h, "init")?);
            init()?;
            Ok(SnapPlugin{
                capture_fn,
                process_fn,
            })
        }
    };

    let res = load();
    if res.is_err() {
        util::unload_lib(h)?;
    }
    res
}