
use shared_dx::types::DevicePointer;
use shared_dx::util::write_log_file;
use types::TexPtr;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ptr::addr_of_mut;
use std::sync::atomic::AtomicI64;
use std::time::SystemTime;
use std::fmt;
use fnv::FnvHashMap;
use fnv::FnvHashSet;

use types::interop;
use types::native_mod;
use types::d3dx;

use snaplib::anim_snap_state::AnimSnapState;

pub const MAX_STAGE: usize = 40;

/// Enable this to dump out a file containing metrics for primitives every
/// few seconds.  The file is `rendered_last_frame.txt` and is stored in the
/// same directory as logs.  It contains one line for each primitive count,
/// vertex count pair that was observed in the frame prior to the dump.
/// This is intended as a tool to roughly observe the frequence and count
/// of primitives in a given frame.  Since it costs a bit of performance and
/// I don't normally use it, it is off by default.
pub const METRICS_TRACK_PRIMS: bool = false;
/// Similar to METRICS_TRACK_PRIMS, but instead this dumps out a list of the recently
/// rendered mods and their types during the periodic logging dump (every few seconds).
pub const METRICS_TRACK_MOD_PRIMS: bool = false;


#[derive(Debug)]
pub enum RenderedPrimType {
    PrimVertCount(u32,u32),
    PrimCountVertSizeAndVBs(u32,u32,Vec<(u32,u32,u32)>)
}

pub struct FrameMetrics {
    pub dip_calls: u32,
    pub frames: u32,
    pub total_frames: u64,
    pub last_call_log: SystemTime,
    pub last_frame_log: SystemTime,
    pub last_fps: f64,
    pub last_fps_update: SystemTime,
    pub low_framerate: bool,
    pub rendered_prims: Vec<RenderedPrimType>,
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

pub struct ClrState {
    pub runtime_pointer: Option<u64>,
    pub run_context: String,
}

pub struct RunConf {
    pub precopy_data: bool,
    pub force_tex_cpu_read: bool,
    /// Registry path of the matched game profile (e.g.
    /// `Software\ModelMod\Profiles\Profile0000`), or empty if none was found.
    /// Populated at device-creation time by the early profile lookup in
    /// `util::game_profile`.
    pub profile_key: String,
}
pub struct HookState {
    pub run_conf: RunConf,
    pub clr: ClrState,
    pub interop_state: Option<interop::InteropState>,
    //pub is_global: bool,
    pub loaded_mods: Option<LoadedModState>,
    /// List of mod names that should have the d3d resources loaded on the next frame.
    /// Mods are added to this by `hook_draw_indexed_primitive` when it discovers that is
    /// trying to render a mod that hasn't been loaded yet.
    pub load_on_next_frame: Option<FnvHashSet<String>>,
    // lists of pointers containing the set of textures in use during snapshotting.
    // these are simply compared against the selection texture, never dereferenced.
    pub active_texture_set: Option<FnvHashSet<usize>>,
    pub active_texture_list: Option<Vec<usize>>,
    /// DX9 only: mapping of destination texture pointer (usize) to source texture
    /// pointer (usize), as observed in calls to IDirect3DDevice9::UpdateTexture.
    /// Used during snapshotting to find a lockable (SYSTEMMEM/MANAGED) source
    /// texture for a given DEFAULT-pool destination texture. The map is populated
    /// by `hook_update_texture` and read by `d3dx::save_texture`. The most recent
    /// mapping for a given destination wins. Entries are only removed when the
    /// GC pass (`dx9_update_texture_gc`) discovers a source texture has been
    /// fully released (refcount reached zero).
    pub dx9_update_texture_map: Option<FnvHashMap<usize, usize>>,
    /// DX9 only: set of source texture pointers (as usize) on which we currently
    /// own a single AddRef'd reference (taken in `hook_update_texture`). Used to
    /// deduplicate AddRefs so that no matter how many times a given source shows
    /// up in an UpdateTexture call, we only hold one extra ref on it. The
    /// `dx9_update_texture_gc` pass periodically releases these references.
    pub dx9_update_texture_tracked_srcs: Option<FnvHashSet<usize>>,
    /// DX9 only: ordered queue of (source pointer, time-of-AddRef) entries for
    /// the GC pass. Each entry corresponds to one owned ref in
    /// `dx9_update_texture_tracked_srcs`. Front is oldest. Consumed by
    /// `dx9_update_texture_gc`.
    pub dx9_update_texture_deque: Option<VecDeque<(usize, SystemTime)>>,
    /// DX9 only: the last time the GC pass was run (from `hook_present`).
    pub dx9_update_texture_last_gc: SystemTime,
    pub making_selection: bool,
    pub in_dip: bool,
    pub in_hook_release: bool,
    pub in_beginend_scene: bool,
    pub show_mods: bool,
    pub mm_root: Option<String>,
    pub input: Option<input::Input>,
    pub selection_texture: Option<TexPtr>,
    pub selected_on_stage: [bool; MAX_STAGE],
    pub curr_texture_index: usize,
    pub is_snapping: bool,
    pub snap_start: SystemTime,
    pub d3dx_fn: Option<d3dx::D3DXFn>,
    pub device: Option<DevicePointer>,
    pub metrics: FrameMetrics,
    pub vertex_constants: Option<constant_tracking::ConstantGroup>,
    pub pixel_constants: Option<constant_tracking::ConstantGroup>,
    pub last_snapshot_dir: Option<String>,
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

    pub static ref GS_PTR_REFS:AtomicI64 = AtomicI64::new(0);
}

// TODO: maybe create read/write accessors for this
// TODO: actually the way global state is handled is super gross.  at a minimum it seems
// like it should be a behind a RW lock, and if I made it a pointer/box I could get rid of some
// of the option types that are only there due to Rust limitations on what can be used to
// init constants.
pub static mut GLOBAL_STATE: HookState = HookState {
    run_conf: RunConf {
        precopy_data: false,
        force_tex_cpu_read: false,
        profile_key: String::new(),
    },
    clr: { ClrState { runtime_pointer: None, run_context: String::new() } },
    interop_state: None,
    //is_global: true,
    load_on_next_frame: None,
    loaded_mods: None,
    active_texture_set: None,
    active_texture_list: None,
    dx9_update_texture_map: None,
    dx9_update_texture_tracked_srcs: None,
    dx9_update_texture_deque: None,
    dx9_update_texture_last_gc: std::time::UNIX_EPOCH,
    making_selection: false,
    in_dip: false,
    in_hook_release: false,
    in_beginend_scene: false,
    show_mods: true,
    mm_root: None,
    input: None,
    selection_texture: None,
    selected_on_stage: [false; MAX_STAGE],
    curr_texture_index: 0,
    is_snapping: false,
    snap_start: std::time::UNIX_EPOCH,
    vertex_constants: None,
    pixel_constants: None,
    last_snapshot_dir: None,
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
        rendered_prims: vec![],
    }
};
pub static mut ANIM_SNAP_STATE:UnsafeCell<Option<AnimSnapState>> = UnsafeCell::new(None);

const TRACK_GS_PTR:bool = true;

/// Container structure providing access to the global state pointer.
/// The intention is to let me log if there is ever more than once access at a time.
/// Obviously this can be defeated by copying the pointer but at least if I inadvertently 
/// try to do this I'll get some warning in the log.
/// This is possibly more important now because I may have UB in the code where I was 
/// creating &mut's to this and created more than one at a time, which the compiler now 
/// reports is UB and will be an error in the future.
pub struct GSPointerRef<'a> {
    pub gsp: *mut HookState,
    marker: PhantomData<&'a i32>,
}

impl<'a> GSPointerRef<'a> {
    pub fn new() -> GSPointerRef<'a> {
        if TRACK_GS_PTR {
            let cnt = GS_PTR_REFS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if cnt > 0 {
                // let the log throttler deal with this if it gets spammed
                write_log_file(&format!("Warning: GSPointerRef exceeded zero, possible concurrent reference to global state: (old value: {})", cnt));
            }
        }
        GSPointerRef {
            gsp: addr_of_mut!(GLOBAL_STATE),
            marker: PhantomData,
        }
    }

}

impl<'a> Drop for GSPointerRef<'a> {
    fn drop(&mut self) {
        if TRACK_GS_PTR {
            let cnt = GS_PTR_REFS.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if cnt <= 0 {
                write_log_file(&format!("Warning: GSPointerRef is now negative (old value: {})", cnt));
            }
        }
    }
}

pub fn get_global_state_ptr<'a>() -> GSPointerRef<'a> {
    GSPointerRef::new()
}
