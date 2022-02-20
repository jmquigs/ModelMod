
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use std::time::SystemTime;
use std::fmt;
use std::ptr::null_mut;
use fnv::FnvHashMap;
use fnv::FnvHashSet;

use types::interop;
use types::native_mod;
use types::d3dx;

use snaplib::anim_snap_state::AnimSnapState;

pub (crate) const MAX_STAGE: usize = 16;

pub struct FrameMetrics {
    pub dip_calls: u32,
    pub frames: u32,
    pub total_frames: u64,
    pub last_call_log: SystemTime,
    pub last_frame_log: SystemTime,
    pub last_fps: f64,
    pub last_fps_update: SystemTime,
    pub low_framerate: bool,
}

pub type LoadedModsMap = FnvHashMap<u32, Vec<native_mod::NativeModData>>;
pub type ModsByNameMap = FnvHashMap<String,u32>;
pub type SelectedVariantMap = FnvHashMap<u32, usize>;
pub fn new_fnv_map<A,B> (capacity:usize) -> FnvHashMap<A,B> {
    FnvHashMap::with_capacity_and_hasher(capacity, Default::default())
}

pub struct LoadedModState {
    pub mods: LoadedModsMap,
    pub mods_by_name: ModsByNameMap,
    pub selected_variant: SelectedVariantMap,
}
pub struct HookState {
    pub clr_pointer: Option<u64>,
    pub interop_state: Option<interop::InteropState>,
    //pub is_global: bool,
    pub loaded_mods: Option<LoadedModState>,
    /// List of mod names that should have the d3d resources loaded on the next frame.
    /// Mods are added to this by `hook_draw_indexed_primitive` when it discovers that is
    /// trying to render a mod that hasn't been loaded yet.
    pub load_on_next_frame: Option<FnvHashSet<String>>,
    // lists of pointers containing the set of textures in use during snapshotting.
    // these are simply compared against the selection texture, never dereferenced.
    pub active_texture_set: Option<FnvHashSet<*mut IDirect3DBaseTexture9>>,
    pub active_texture_list: Option<Vec<*mut IDirect3DBaseTexture9>>,
    pub making_selection: bool,
    pub in_dip: bool,
    pub in_hook_release: bool,
    pub in_beginend_scene: bool,
    pub show_mods: bool,
    pub mm_root: Option<String>,
    pub input: Option<input::Input>,
    pub selection_texture: *mut IDirect3DTexture9,
    pub selected_on_stage: [bool; MAX_STAGE],
    pub curr_texture_index: usize,
    pub is_snapping: bool,
    pub snap_start: SystemTime,
    pub d3dx_fn: Option<d3dx::D3DXFn>,
    pub device: Option<*mut IDirect3DDevice9>, // only valid during snapshots
    pub metrics: FrameMetrics,
    pub vertex_constants: Option<constant_tracking::ConstantGroup>,
    pub pixel_constants: Option<constant_tracking::ConstantGroup>,
    pub anim_snap_state: Option<AnimSnapState>,
}

impl HookState {
    pub fn in_any_hook_fn(&self) -> bool {
        self.in_dip || self.in_hook_release || self.in_beginend_scene
    }
}
impl fmt::Display for HookState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "HookState (thread: {:?})", // : d3d9: {:?}, device: {:?}",
            std::thread::current().id(),
            //self.hook_direct3d9.is_some(),
            //self.hook_direct3d9device.is_some()
        )
    }
}

lazy_static! {
    pub static ref GLOBAL_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

// TODO: maybe create read/write accessors for this
// TODO: actually the way global state is handled is super gross.  at a minimum it seems
// like it should be a behind a RW lock, and if I made it a pointer/box I could get rid of some
// of the option types that are only there due to Rust limitations on what can be used to
// init constants.
pub static mut GLOBAL_STATE: HookState = HookState {
    clr_pointer: None,
    interop_state: None,
    //is_global: true,
    load_on_next_frame: None,
    loaded_mods: None,
    active_texture_set: None,
    active_texture_list: None,
    making_selection: false,
    in_dip: false,
    in_hook_release: false,
    in_beginend_scene: false,
    show_mods: true,
    mm_root: None,
    input: None,
    selection_texture: null_mut(),
    selected_on_stage: [false; MAX_STAGE],
    curr_texture_index: 0,
    is_snapping: false,
    snap_start: std::time::UNIX_EPOCH,
    vertex_constants: None,
    pixel_constants: None,
    anim_snap_state: None,
    d3dx_fn: None,
    device: None,
    metrics: FrameMetrics {
        dip_calls: 0,
        frames: 0,
        total_frames: 0,
        last_call_log: std::time::UNIX_EPOCH,
        last_frame_log: std::time::UNIX_EPOCH,
        last_fps_update: std::time::UNIX_EPOCH,
        last_fps: 120.0,
        low_framerate: false,
    },
};

pub fn get_global_state_ptr() -> *mut HookState {
    let pstate: *mut HookState = unsafe { &mut GLOBAL_STATE };
    pstate
}
