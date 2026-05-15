
use shared_dx::types::DevicePointer;
use shared_dx::util::write_log_file;
use types::TexPtr;
use util::game_profile::EMPTY_GAME_PROFILE;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use std::cell::UnsafeCell;
use std::ptr::null_mut;
use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use std::fmt;
use fnv::FnvHashMap;
use fnv::FnvHashSet;

use types::interop;
use types::native_mod;
use types::d3dx;

use snaplib::anim_snap_state::AnimSnapState;
use util::game_profile::GameProfile;

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

/// Resolved state of a vertex buffer's CRC32 entry in `vb_checksums`.
///
/// A VB that is not present in the map at all is implicitly "pending" —
/// the DX9 DIP hook will attempt to hash it the first time it sees it
/// bound, and insert either `Checksum` or `NotPossible` depending on
/// whether the Lock succeeded. DX11 hashes at create time using the
/// supplied initial data and goes straight to `Checksum`.
#[derive(Debug, Clone, Copy)]
pub enum VBChecksumStatus {
    /// Successfully computed CRC32 over the buffer's bytes.
    Checksum(u32),
    /// Buffer cannot be hashed (Lock failed). Don't retry.
    NotPossible,
}

impl VBChecksumStatus {
    /// Return the CRC if this entry has successfully been hashed.
    /// `NotPossible` returns `None`.
    pub fn checksum(&self) -> Option<u32> {
        match self {
            VBChecksumStatus::Checksum(c) => Some(*c),
            VBChecksumStatus::NotPossible => None,
        }
    }
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

// `LoadedModState` holds raw d3d resource pointers (textures, buffers) which
// are not `Send` by default. We need `Send` so that the global `Mutex` can be
// `Sync`, which lets multiple threads (render thread, deferred load thread)
// access it through the lock. The same `unsafe impl Send` pattern is already
// used for `LoadMsg` in `mod_load::load_thread`, where a cloned `NativeModData`
// is shipped across threads.
unsafe impl Send for LoadedModState {}

pub struct ClrState {
    pub runtime_pointer: Option<u64>,
    pub run_context: String,
}

pub struct RunConf {
    pub precopy_data: bool,
    pub force_tex_cpu_read: bool,
    /// Game profile data loaded from the profile found for this registry key
    /// (example: `Software\ModelMod\Profiles\Profile0000`), or empty if none was found.
    pub profile: GameProfile,
}
pub struct HookState {
    pub run_conf: RunConf,
    pub clr: ClrState,
    pub interop_state: Option<interop::InteropState>,
    //pub is_global: bool,
    /// List of mod names that should have the d3d resources loaded on the next frame.
    /// Mods are added to this by `hook_draw_indexed_primitive` when it discovers that is
    /// trying to render a mod that hasn't been loaded yet.
    pub load_on_next_frame: Option<FnvHashSet<String>>,
    // lists of pointers containing the set of textures in use during snapshotting.
    // these are simply compared against the selection texture, never dereferenced.
    pub active_texture_set: Option<FnvHashSet<usize>>,
    pub active_texture_list: Option<Vec<usize>>,
    pub making_selection: bool,
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
    /// Map of vertex-buffer pointer (as `usize`) to its checksum status.
    /// Checksums are generally computed at snapshot or on the first draw.
    /// When a checksum is specified in a mod it serves as an extra key 
    /// (other than prim,vert count) to control whether a mod is displayed.
    pub vb_checksums: Option<FnvHashMap<usize, VBChecksumStatus>>,
    /// Pointer (as `usize`) of the vertex buffer currently bound to stream 0.
    /// Written by the DX9 `SetStreamSource` hook and the DX11
    /// `IASetVertexBuffers` hook. `0` means no buffer is bound.
    pub bound_vertex_buffer: usize,
    /// Set of `(prim_count, vert_count)` pairs for which at least one loaded
    /// mod has a VB-checksum constraint. When `Some`, the DIP hooks will only
    /// compute a VB checksum for draws whose counts are in this set (or when
    /// snapshotting). This is populated after mod loading.
    pub vb_checksum_targets: Option<FnvHashSet<(u32, u32)>>,
}

impl HookState {
    pub fn new() -> Self {
        HookState {
            run_conf: RunConf {
                precopy_data: false,
                force_tex_cpu_read: false,
                profile: EMPTY_GAME_PROFILE,
            },
            clr: ClrState { runtime_pointer: None, run_context: String::new() },
            interop_state: None,
            load_on_next_frame: None,
            active_texture_set: None,
            active_texture_list: None,
            making_selection: false,
            show_mods: true,
            mm_root: None,
            input: None,
            selection_texture: None,
            selected_on_stage: [false; MAX_STAGE],
            curr_texture_index: 0,
            is_snapping: false,
            snap_start: std::time::UNIX_EPOCH,
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
            },
            vertex_constants: None,
            pixel_constants: None,
            last_snapshot_dir: None,
            vb_checksums: None,
            bound_vertex_buffer: 0,
            vb_checksum_targets: None,
        }
    }
}

impl Default for HookState {
    fn default() -> Self { Self::new() }
}

/// Re-entry guard flags. These live outside the locked HookState so that
/// the very first thing a hot-path hook does (the re-entry check) is
/// lock-free, and so a re-entrant call into our hooks cannot deadlock on
/// the global state lock.
pub static IN_DIP: AtomicBool = AtomicBool::new(false);
pub static IN_HOOK_RELEASE: AtomicBool = AtomicBool::new(false);
pub static IN_BEGINEND_SCENE: AtomicBool = AtomicBool::new(false);

pub fn in_any_hook_fn() -> bool {
    IN_DIP.load(Ordering::Acquire)
        || IN_HOOK_RELEASE.load(Ordering::Acquire)
        || IN_BEGINEND_SCENE.load(Ordering::Acquire)
}

/// RAII guard for an `AtomicBool` reentry flag. Calls `swap(true)` on
/// entry; if the flag was already `true` returns `None` (re-entry
/// detected). On drop, sets the flag back to `false`. This guarantees we
/// can't leak the flag on early-return paths.
pub struct ReentryGuard(&'static AtomicBool);

impl ReentryGuard {
    pub fn try_enter(flag: &'static AtomicBool) -> Option<Self> {
        if flag.swap(true, Ordering::Acquire) {
            None
        } else {
            Some(ReentryGuard(flag))
        }
    }
}

impl Drop for ReentryGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
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

/// Newtype wrapping the raw `HookState` pointer so it can live inside a
/// `static RwLock`. The pointer itself is set once at hook install time
/// (via `init_hook_state`) and never reassigned during the process
/// lifetime, so the `Send`/`Sync` impls reflect the actual access pattern:
/// the `RwLock` controls concurrent access to the pointee, while the
/// pointer field is only mutated under the write guard during init.
pub struct HookStatePtr(pub *mut HookState);
unsafe impl Send for HookStatePtr {}
unsafe impl Sync for HookStatePtr {}

impl HookStatePtr {
    pub fn is_null(&self) -> bool { self.0.is_null() }
}

/// Replaces the old `static mut GLOBAL_STATE` with a `RwLock` around a
/// heap-allocated `HookState`. Initialized lazily by `init_hook_state`,
/// which the hook install paths call in addition to `init_device_state_once`.
pub static HOOK_STATE: RwLock<HookStatePtr> = RwLock::new(HookStatePtr(null_mut()));

pub static mut ANIM_SNAP_STATE:UnsafeCell<Option<AnimSnapState>> = UnsafeCell::new(None);

/// Loaded mod database.
///
/// Wrapped in a `Mutex` because the render thread reads/mutates this on every
/// draw call while the deferred load thread writes back into it when a mod
/// finishes loading. Callers should keep the lock for as short a span as
/// possible — particularly on the DIP path.
pub static LOADED_MODS: Mutex<Option<LoadedModState>> = Mutex::new(None);

/// Allocate the global `HookState` and store its pointer in `HOOK_STATE`.
/// Idempotent: safe to call from multiple hook install paths. Subsequent
/// calls after the state has been allocated are no-ops, preserving any
/// runtime mutations made between init and re-init.
pub fn init_hook_state() {
    let mut guard = match HOOK_STATE.write() {
        Ok(g) => g,
        Err(e) => {
            write_log_file(&format!("init_hook_state: lock poisoned: {}", e));
            return;
        }
    };
    if !guard.0.is_null() {
        return;
    }
    let new_ptr = Box::into_raw(Box::new(HookState::new()));
    guard.0 = new_ptr;
    write_log_file(&format!("initted new hook state instance: {:x}", new_ptr as usize));
}

/// Acquire a write guard on the hook state. Returns `None` if the lock is
/// poisoned or the state pointer is null. On poison this logs and proceeds
/// in fail-safe mode rather than crashing the host process.
pub fn hook_state_write<'a>() -> Option<(RwLockWriteGuard<'a, HookStatePtr>, &'a mut HookState)> {
    match HOOK_STATE.write() {
        Ok(mut lock) => {
            if lock.0.is_null() {
                return None;
            }
            // SAFETY: the write guard provides exclusive access to the
            // pointer and (by convention in this module) to the pointee.
            // The lifetime is tied to the guard via the function signature.
            let ptr = lock.0;
            let r: &mut HookState = unsafe { &mut *ptr };
            let _ = &mut lock;
            Some((lock, r))
        }
        Err(e) => {
            write_log_file(&format!("hook_state_write: lock poisoned: {}", e));
            None
        }
    }
}

/// Acquire a read guard on the hook state. Returns `None` if the lock is
/// poisoned or the state pointer is null.
pub fn hook_state_read<'a>() -> Option<(RwLockReadGuard<'a, HookStatePtr>, &'a HookState)> {
    match HOOK_STATE.read() {
        Ok(lock) => {
            if lock.0.is_null() {
                return None;
            }
            // SAFETY: the read guard ensures no writer is active.
            let r: &HookState = unsafe { &*lock.0 };
            Some((lock, r))
        }
        Err(e) => {
            write_log_file(&format!("hook_state_read: lock poisoned: {}", e));
            None
        }
    }
}

/// Install the set of `(prim_count, vert_count)` pairs for which a loaded
/// mod has a VB-checksum constraint.
pub fn set_vb_checksum_targets(set: FnvHashSet<(u32, u32)>) {
    write_log_file(&format!(
        "set_vb_checksum_targets: installed {} target(s)",
        set.len()
    ));
    if let Some((_lck, gs)) = hook_state_write() {
        gs.vb_checksum_targets = Some(set);
    }
}

/// Returns true if the given
/// `(prim_count, vert_count)` pair matches a loaded mod's VB-checksum
/// constraint, false otherwise.
///
/// Convenience wrapper that acquires the global read lock; for hot-path
/// callers that already hold a `&HookState`, prefer
/// [`vb_checksum_target_matches_with`].
pub fn vb_checksum_target_matches(prim_count: u32, vert_count: u32) -> bool {
    match hook_state_read() {
        Some((_lck, gs)) => vb_checksum_target_matches_with(gs, prim_count, vert_count),
        None => false,
    }
}

/// Lock-free variant of [`vb_checksum_target_matches`] for callers that
/// already hold a `&HookState` (the DIP hot path).
pub fn vb_checksum_target_matches_with(gs: &HookState, prim_count: u32, vert_count: u32) -> bool {
    match gs.vb_checksum_targets.as_ref() {
        Some(set) => set.contains(&(prim_count, vert_count)),
        None => false,
    }
}
