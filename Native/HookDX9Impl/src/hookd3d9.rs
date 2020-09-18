use winapi::um::unknwnbase::IUnknown;

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::wingdi::RGNDATA;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use fnv::FnvHashMap;
use fnv::FnvHashSet;

use dnclr::{init_clr, reload_managed_dll};
use input;
use interop;
use interop::InteropState;
use interop::NativeModData;
use util;
use util::*;
use constant_tracking;
use mod_load;
use mod_load::AsyncLoadState;
use input_commands;
use shader_capture;
use d3dx;

use std;
use std::fmt;
use std::ptr::null_mut;
use std::time::SystemTime;

use shared_dx9::defs::*;
use shared_dx9::types::*;
use shared_dx9::util::*;
use shared_dx9::error::*;

pub (crate) const CLR_OK:u64 = 1;
pub (crate) const CLR_FAIL:u64 = 666;
pub (crate) const MAX_STAGE: usize = 16;

// Snapshotting currently stops after a certain amount of real time has passed from the start of
// the snap, specified by this constant.
// One might expect that just snapping everything drawn within a single begin/end scene combo is
// sufficient, but this often misses data,
// and sometimes fails to snapshot anything at all.  This may be because the game is using multiple
// begin/end combos, so maybe
// present->present would be more reliable (TODO: check this)
// Using a window makes it much more likely that something useful is captured, at the expense of
// some duplicates; even though
// some objects may still be missed.  Some investigation to make this more reliable would be useful.
static SNAP_MS: u32 = 250;

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

pub struct HookState {
    pub clr_pointer: Option<u64>,
    pub interop_state: Option<InteropState>,
    //pub is_global: bool,
    pub loaded_mods: Option<FnvHashMap<u32, Vec<interop::NativeModData>>>,
    pub mods_by_name: Option<FnvHashMap<String,u32>>,
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
pub static mut DEVICE_STATE: *mut DeviceState = null_mut();

pub fn dev_state() -> &'static mut DeviceState {
    unsafe {
        if DEVICE_STATE == null_mut() {
            write_log_file("accessing null device state pointer, this 'should never happen'.  we gonna crash boys");
            panic!("Aborting because I'm about to dereference a null device state pointer.");
        }
        &mut (*DEVICE_STATE)
    }
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
    loaded_mods: None,
    mods_by_name: None,
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

macro_rules! impl_release_drop {
    ($ptrtype:ident) => {
        impl ReleaseDrop for *mut $ptrtype {
            fn OnDrop(&mut self) -> () {
                unsafe {
                    let ptr = *self;
                    if ptr != null_mut() {
                        (*ptr).Release();
                    }
                };
            }
        }
    };
}

impl_release_drop!(IDirect3DBaseTexture9);
impl_release_drop!(IDirect3DVertexDeclaration9);
impl_release_drop!(IDirect3DIndexBuffer9);
impl_release_drop!(IDirect3DPixelShader9);
impl_release_drop!(IDirect3DVertexShader9);
impl_release_drop!(ID3DXBuffer);

fn snapshot_extra() -> bool {
    return constant_tracking::is_enabled() || shader_capture::is_enabled()
}

fn get_current_texture() -> *mut IDirect3DBaseTexture9 {
    unsafe {
        let idx = GLOBAL_STATE.curr_texture_index;
        GLOBAL_STATE
            .active_texture_list
            .as_ref()
            .map(|list| {
                if idx >= list.len() {
                    null_mut()
                } else {
                    list[idx]
                }
            })
            .unwrap_or(null_mut())
    }
}

fn get_selected_texture_stage() -> Option<DWORD> {
    unsafe {
        for i in 0..MAX_STAGE {
            if GLOBAL_STATE.selected_on_stage[i] {
                return Some(i as DWORD);
            }
        }
        None
    }
}

pub fn get_global_state_ptr() -> *mut HookState {
    let pstate: *mut HookState = unsafe { &mut GLOBAL_STATE };
    pstate
}

pub fn do_per_frame_operations(device: *mut IDirect3DDevice9) -> Result<()> {
    // init the clr if needed
    {
        let hookstate = unsafe { &mut GLOBAL_STATE };
        if hookstate.clr_pointer.is_none() {
            let lock = GLOBAL_STATE_LOCK.lock();
            match lock {
                Ok(_ignored) => {
                    if hookstate.clr_pointer.is_none() {
                        // store something in clr_pointer even if it create fails,
                        // so that we don't keep trying to create it.  clr_pointer is
                        // really just a bool right now, it remains to be
                        // seen whether storing anything related to clr in
                        // global state is actually useful.
                        write_log_file("creating CLR");
                        init_clr(&hookstate.mm_root)
                            .and_then(|_x| {
                                reload_managed_dll(&hookstate.mm_root)
                            })
                            .and_then(|_x| {
                                hookstate.clr_pointer = Some(CLR_OK);
                                Ok(_x)
                            })
                            .map_err(|e| {
                                write_log_file(&format!("Error creating CLR: {:?}", e));
                                hookstate.clr_pointer = Some(CLR_FAIL);
                                e
                            })?;
                    }
                }
                Err(e) => write_log_file(&format!("{:?} should never happen", e)),
            };
        }
    }
    // write_log_file(&format!("performing per-scene ops on thread {:?}",
    //         std::thread::current().id()));

    let interop_state = unsafe { &mut GLOBAL_STATE.interop_state };

    interop_state.as_mut().map(|is| {
        if !is.loading_mods && !is.done_loading_mods && is.conf_data.LoadModsOnStart {
            let loadstate = unsafe { (is.callbacks.GetLoadingState)() };
            if loadstate == AsyncLoadState::InProgress as i32 {
                is.loading_mods = true;
                is.done_loading_mods = false;
            } else if loadstate != AsyncLoadState::Pending as i32 {
                let r = unsafe { (is.callbacks.LoadModDB)() };
                if r == AsyncLoadState::Pending as i32 {
                    is.loading_mods = true;
                    is.done_loading_mods = false;
                }
                if r == AsyncLoadState::Complete as i32 {
                    is.loading_mods = false;
                    is.done_loading_mods = true;
                }
                write_log_file(&format!("mod db load returned: {}", r));
            }
        }

        if is.loading_mods
            && unsafe { (is.callbacks.GetLoadingState)() } == AsyncLoadState::Complete as i32
        {
            write_log_file("mod loading complete");
            is.loading_mods = false;
            is.done_loading_mods = true;

            unsafe { mod_load::setup_mod_data(device, is.callbacks) };
        }
    });
    Ok(())
}

pub (crate) unsafe extern "system" fn hook_set_texture(
    THIS: *mut IDirect3DDevice9,
    Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,
) -> HRESULT {
    let has_it = GLOBAL_STATE
        .active_texture_set
        .as_ref()
        .map(|set| set.contains(&pTexture))
        .unwrap_or(true);
    if !has_it {
        GLOBAL_STATE.active_texture_set.as_mut().map(|set| {
            set.insert(pTexture);
        });
        GLOBAL_STATE.active_texture_list.as_mut().map(|list| {
            list.push(pTexture);
        });
    }

    if Stage < MAX_STAGE as u32 {
        let curr = get_current_texture();
        if curr != null_mut() && pTexture == curr {
            GLOBAL_STATE.selected_on_stage[Stage as usize] = true;
        } else if GLOBAL_STATE.selected_on_stage[Stage as usize] {
            GLOBAL_STATE.selected_on_stage[Stage as usize] = false;
        }
    }

    (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_texture)(THIS, Stage, pTexture)
}


// TODO: hook this up to device release at the proper time
unsafe fn purge_device_resources(device: *mut IDirect3DDevice9) {
    if device == null_mut() {
        write_log_file("WARNING: ignoring insane attempt to purge devices on a null device");
        return;
    }
    mod_load::clear_loaded_mods(device);
    if GLOBAL_STATE.selection_texture != null_mut() {
        (*GLOBAL_STATE.selection_texture).Release();
        GLOBAL_STATE.selection_texture = null_mut();
    }
    GLOBAL_STATE
        .input
        .as_mut()
        .map(|input| input.clear_handlers());
    dev_state().d3d_resource_count = 0;
}

pub unsafe extern "system" fn hook_present(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT {
    //write_log_file("present");
    if GLOBAL_STATE.in_any_hook_fn() {
        return (dev_state().hook_direct3d9device.as_ref().unwrap().real_present)(
            THIS,
            pSourceRect,
            pDestRect,
            hDestWindowOverride,
            pDirtyRegion,
        );
    }

    if let Err(e) = do_per_frame_operations(THIS) {
        write_log_file(&format!(
            "unexpected error from do_per_scene_operations: {:?}",
            e
        ));
        return (dev_state().hook_direct3d9device.as_ref().unwrap().real_present)(
            THIS,
            pSourceRect,
            pDestRect,
            hDestWindowOverride,
            pDirtyRegion,
        );
    }

    let min_fps = GLOBAL_STATE
        .interop_state
        .map(|is| is.conf_data.MinimumFPS)
        .unwrap_or(0) as f64;

    let metrics = &mut GLOBAL_STATE.metrics;
    let present_ret = dev_state()
        .hook_direct3d9device
        .as_mut()
        .map_or(S_OK, |hookdevice| {
            metrics.frames += 1;
            metrics.total_frames += 1;
            if metrics.frames % 90 == 0 {
                // enforce min fps
                // NOTE: when low, it just sets a boolean flag to disable mod rendering,
                // but we could also use virtual protect to temporarily swap out the hook functions
                // (except for present)
                let now = SystemTime::now();
                let elapsed = now.duration_since(metrics.last_fps_update);
                if let Ok(d) = elapsed {
                    let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                    let fps = metrics.frames as f64 / secs;
                    let smooth_fps = 0.3 * fps + 0.7 * metrics.last_fps;
                    metrics.last_fps = smooth_fps;
                    let min_off = min_fps * 1.1;
                    if smooth_fps < min_fps && !metrics.low_framerate {
                        metrics.low_framerate = true;
                    }
                    // prevent oscillation: don't reactivate until 10% above mininum
                    else if metrics.low_framerate && smooth_fps > (min_off * 1.1) {
                        metrics.low_framerate = false;
                    }
                    // write_log_file(&format!(
                    //     "{} frames in {} secs ({} instant, {} smooth) (low: {})",
                    //     hookdevice.frames, secs, fps, smooth_fps, hookdevice.low_framerate
                    // ));
                    metrics.last_fps_update = now;
                    metrics.frames = 0;
                }
            }
            (hookdevice.real_present)(
                THIS,
                pSourceRect,
                pDestRect,
                hDestWindowOverride,
                pDirtyRegion,
            )
        });

    if GLOBAL_STATE.selection_texture == null_mut() {
        input_commands::create_selection_texture(THIS);
    }

    if util::appwnd_is_foreground(dev_state().d3d_window) {
        GLOBAL_STATE.input.as_mut().map(|inp| {
            if inp.get_press_fn_count() == 0 {
                input_commands::setup_input(THIS, inp)
                    .unwrap_or_else(|e| write_log_file(&format!("input setup error: {:?}", e)));
            }
            inp.process()
                .unwrap_or_else(|e| write_log_file(&format!("input error: {:?}", e)));
        });
    }

    if GLOBAL_STATE.is_snapping {
        let now = SystemTime::now();
        let max_dur = std::time::Duration::from_millis(SNAP_MS as u64);
        if now
            .duration_since(GLOBAL_STATE.snap_start)
            .unwrap_or(max_dur)
            >= max_dur
        {
            write_log_file("ending snapshot");
            GLOBAL_STATE.is_snapping = false;
        }
    }

    present_ret
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
    // TODO: hack to work around Release on device while in DIP
    if GLOBAL_STATE.in_hook_release {
        return (dev_state().hook_direct3d9device.as_ref().unwrap().real_release)(THIS);
    }

    GLOBAL_STATE.in_hook_release = true;

    let r = dev_state()
        .hook_direct3d9device
        .as_mut()
        .map_or(0xFFFFFFFF, |hookdevice| {
            hookdevice.ref_count = (hookdevice.real_release)(THIS);

            // if hookdevice.ref_count < 100 {
            //     write_log_file(&format!(
            //         "device {:x} refcount now {}",
            //         THIS as u64, hookdevice.ref_count
            //     ));
            // }

            // could just leak everything on device destroy.  but I know that will
            // come back to haunt me.  so make an effort to purge my stuff when the
            // resource count gets to the expected value, this way the device can be
            // properly disposed.

            let destroying = dev_state().d3d_resource_count > 0
                && hookdevice.ref_count == (dev_state().d3d_resource_count + 1);
            if destroying {
                // purge my stuff
                write_log_file(&format!(
                    "device {:x} refcount is same as internal resource count ({}),
                    it is being destroyed: purging resources",
                    THIS as u64, dev_state().d3d_resource_count
                ));
                purge_device_resources(THIS as *mut IDirect3DDevice9);
                // Note, hookdevice.ref_count is wrong now since we bypassed
                // this function during unload (no re-entrancy).  however the count on the
                // device should be 1 if I did the math right, anyway the release below
                // will fix the count.
            }

            if destroying || (dev_state().d3d_resource_count == 0 && hookdevice.ref_count == 1) {
                // release again to trigger destruction of the device
                hookdevice.ref_count = (hookdevice.real_release)(THIS);
                write_log_file(&format!(
                    "device released: {:x}, refcount: {}",
                    THIS as u64, hookdevice.ref_count
                ));
                if hookdevice.ref_count != 0 {
                    write_log_file(&format!(
                        "WARNING: unexpected ref count of {} after supposedly final
                        device release, device probably leaked",
                        hookdevice.ref_count
                    ));
                }
            }
            hookdevice.ref_count
        });
    GLOBAL_STATE.in_hook_release = false;
    r
}

// TODO: maybe remove if not needed
// pub unsafe extern "system" fn hook_begin_scene(THIS: *mut IDirect3DDevice9) -> HRESULT {
//     if GLOBAL_STATE.in_any_hook_fn() {
//         return (GLOBAL_STATE.hook_direct3d9device.unwrap().real_begin_scene)(THIS);
//     }
//     GLOBAL_STATE.in_beginend_scene = true;

//     if let Err(e) = do_per_frame_operations(THIS) {
//         write_log_file(&format!("unexpected error: {:?}", e));
//         return E_FAIL;
//     }

//     let r = GLOBAL_STATE
//         .hook_direct3d9device
//         .as_ref()
//         .map_or(E_FAIL, |hookdevice| (hookdevice.real_begin_scene)(THIS));

//     GLOBAL_STATE.in_beginend_scene = false;
//     r
// }

decl_profile_globals!(hdip);

pub unsafe extern "system" fn hook_draw_indexed_primitive(
    THIS: *mut IDirect3DDevice9,
    PrimitiveType: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT {
    let force_modding_off = false;

    profile_start!(hdip, hook_dip);

    // no re-entry please
    profile_start!(hdip, dip_check);
    if GLOBAL_STATE.in_dip {
        write_log_file(&format!("ERROR: i'm in DIP already!"));
        return S_OK;
    }
    profile_end!(hdip, dip_check);

    profile_start!(hdip, state_begin);

    let hookdevice = match dev_state().hook_direct3d9device {
        None => {
            write_log_file(&format!("DIP: No d3d9 device found"));
            return E_FAIL;
        }
        Some(ref mut hookdevice) => hookdevice,
    };
    profile_end!(hdip, state_begin);

    let mut metrics = &mut GLOBAL_STATE.metrics;

    if metrics.low_framerate || !GLOBAL_STATE.show_mods || force_modding_off {
        return (hookdevice.real_draw_indexed_primitive)(
            THIS,
            PrimitiveType,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        );
    }

    // snapshotting
    let (override_texture, sel_stage, this_is_selected) = {
        let default = (null_mut(), 0, false);
        if GLOBAL_STATE.making_selection {
            get_selected_texture_stage()
                .map(|stage| {
                    (
                        std::mem::transmute(GLOBAL_STATE.selection_texture),
                        stage,
                        true,
                    )
                })
                .unwrap_or(default)
        } else {
            default
        }
    };

    if this_is_selected && GLOBAL_STATE.is_snapping {
        write_log_file("Snap started");

        (*THIS).AddRef();
        let pre_rc = (*THIS).Release();

        GLOBAL_STATE.device = Some(THIS);

        if GLOBAL_STATE.d3dx_fn.is_none() {
            GLOBAL_STATE.d3dx_fn = d3dx::load_lib(&GLOBAL_STATE.mm_root)
                .map_err(|e| {
                    write_log_file(&format!(
                        "failed to load d3dx: texture snapping not available: {:?}",
                        e
                    ));
                    e
                })
                .ok();
        }

        // TODO: warn about active streams that are in use but not supported
        let mut blending_enabled: DWORD = 0;
        let hr = (*THIS).GetRenderState(D3DRS_INDEXEDVERTEXBLENDENABLE, &mut blending_enabled);
        if hr == 0 && blending_enabled > 0 {
            write_log_file("WARNING: vertex blending is enabled, this mesh may not be supported");
        }

        let mut ok = true;
        let mut vert_decl: *mut IDirect3DVertexDeclaration9 = null_mut();
        // sharpdx does not expose GetVertexDeclaration, so need to do it here
        let hr = (*THIS).GetVertexDeclaration(&mut vert_decl);

        if hr != 0 {
            write_log_file(&format!(
                "Error, can't get vertex declaration.
                Cannot snap; HR: {:x}",
                hr
            ));
        }
        let _vert_decl_rod = ReleaseOnDrop::new(vert_decl);

        ok = ok && hr == 0;
        let mut ib: *mut IDirect3DIndexBuffer9 = null_mut();
        let hr = (*THIS).GetIndices(&mut ib);
        if hr != 0 {
            write_log_file(&format!(
                "Error, can't get index buffer.  Cannot snap; HR: {:x}",
                hr
            ));
        }
        let _ib_rod = ReleaseOnDrop::new(ib);

        ok = ok && hr == 0;

        if ok {
            let mut sd = interop::SnapshotData {
                sd_size: std::mem::size_of::<interop::SnapshotData>() as u32,
                prim_type: PrimitiveType as i32,
                base_vertex_index: BaseVertexIndex,
                min_vertex_index: MinVertexIndex,
                num_vertices: NumVertices,
                start_index: startIndex,
                prim_count: primCount,
                vert_decl: vert_decl,
                index_buffer: ib,
            };
            write_log_file(&format!("snapshot data size is: {}", sd.sd_size));
            GLOBAL_STATE.interop_state.as_mut().map(|is| {
                let cb = is.callbacks;
                let res = (cb.TakeSnapshot)(THIS, &mut sd);
                if res == 0 && snapshot_extra() {
                    let sresult = *(cb.GetSnapshotResult)();
                    let dir = &sresult.directory[0..(sresult.directory_len as usize)];
                    let sprefix = &sresult.snap_file_prefix[0..(sresult.snap_file_prefix_len as usize)];

                    let dir = String::from_utf16(&dir).unwrap_or_else(|_| "".to_owned());
                    let sprefix = String::from_utf16(&sprefix).unwrap_or_else(|_| "".to_owned());
                    // write_log_file(&format!("snap save dir: {}", dir));
                    // write_log_file(&format!("snap prefix: {}", sprefix));
                    constant_tracking::take_snapshot(&dir, &sprefix);
                    shader_capture::take_snapshot(&dir, &sprefix);
                }
            });
        }
        (*THIS).AddRef();
        let post_rc = (*THIS).Release();
        if pre_rc != post_rc {
            write_log_file(&format!(
                "WARNING: device ref count before snapshot ({}) does not
             equal count after snapshot ({}), likely resources were leaked",
                pre_rc, post_rc
            ));
        }

        GLOBAL_STATE.device = None;
    }

    profile_start!(hdip, main_combinator);
    profile_start!(hdip, mod_key_prep);

    GLOBAL_STATE.in_dip = true;

    let mut drew_mod = false;

    // if there is a matching mod, render it
    let modded = 
        GLOBAL_STATE.loaded_mods.as_mut()
        .and_then(|mods| {
            profile_end!(hdip, mod_key_prep);
            profile_start!(hdip, mod_key_lookup);
            let mod_key = NativeModData::mod_key(NumVertices, primCount);
            let r = mods.get(&mod_key);
            // just get out of here if we didn't have a match
            if let None = r {
                profile_end!(hdip, mod_key_lookup);
                return None;
            }
            // found at least one mod.  do some more checks to see if each has a parent, and if the parent
            // is active.  count the active parents we find because if more than one is active, 
            // we have ambiguity and can't render any of them.
            let mut target_mod_index:usize = 0;
            let mut active_parent_name:&str = "";
            let r2 = r.and_then(|nmods| {
                let mut num_active_parents = 0;
                for (midx,nmod) in nmods.iter().enumerate() {
                    if !nmod.parent_mod_name.is_empty() {
                        GLOBAL_STATE.mods_by_name.as_ref() 
                            .and_then(|mbn| mbn.get(&nmod.parent_mod_name))
                            .and_then(|parmodkey| mods.get(parmodkey))
                            .map(|parent_mods| {
                                // count any active parents
                                for parent_mod in parent_mods.iter() {
                                    if num_active_parents > 1 {
                                        // fail, ambiguity
                                        break;
                                    }
                                    if parent_mod.recently_rendered(GLOBAL_STATE.metrics.total_frames) {
                                        // parent is active
                                        num_active_parents += 1;
                                        
                                        // if this parent is for the mod we are looking at, 
                                        // remember that mod index.  not that we'll slam this if we 
                                        // have multiple active parents for multiple mods, 
                                        // but we are screwed anyway in that case.
                                        if nmod.parent_mod_name == parent_mod.name {
                                            active_parent_name = &parent_mod.name;
                                            target_mod_index = midx;
                                        }
                                    }
                                }
                            });
                    } 
                }
                // return Some(()) if we found a valid one.
                // if multiple mods but only one parent, we're good
                if nmods.len() > 1 && num_active_parents == 1 {
                    // write_log_file(&format!("rend mod {} because just one active parent named '{}'", 
                    //     nmods[target_mod_index].name, active_parent_name));
                    Some(())
                }
                // if just one mod it doesn't have a parent, or if it does and there is just one parent,
                // also good.
                else if nmods.len() == 1 && (nmods[0].parent_mod_name.is_empty() || num_active_parents == 1) {
                    // write_log_file(&format!("rend mod {} because just one mod with parname '{}' or {} parents", 
                    // nmods[target_mod_index].name, nmods[0].parent_mod_name, num_active_parents));

                    Some(())
                } else {
                    None
                }
            });
            // return if we aren't rendering it.
            if let None = r2 {
                profile_end!(hdip, mod_key_lookup);
                return None;
            }
            // ok, we're rendering it, but it might be a parent mod too, so we have to set 
            // the last frame on it, which requires a mutable reference.  we couldn't use a 
            // mutable ref earlier, because we had to do two lookups on the hash table.
            // so we have to refetch as mutable, set the frame value and then (for safety)
            // refetch as immutable again so that we can pass that value on.  that's three
            // hash lookups guaranteed but fortunately we're only doing this for active mods.
            // we also can't be clever and return an immutable ref now if it isn't a parent, 
            // because we won't be able to even write the code that checks for the parent 
            // since it would require the get_mut call and thus a mutable and immutable ref 
            // would be active at the same time.
            // TODO: this bullshit could be avoided by using a refcell on the native mods.
            drop(r);
            drop(r2);
            mods.get_mut(&mod_key).map(|nmods| {
                if target_mod_index >= nmods.len() {
                    // error, spam the log i guess
                    write_log_file(&format!("selected target mod index {} exceeds number of mods {}", 
                        target_mod_index, nmods.len()));
                } else {
                    let nmod = &mut nmods[target_mod_index];
                    if nmod.is_parent {
                        nmod.last_frame_render = GLOBAL_STATE.metrics.total_frames;
                    }
                }
            });
            let r = mods.get(&mod_key).and_then(|nmods| {
                if target_mod_index < nmods.len() {
                    Some(&nmods[target_mod_index])
                } else { 
                    None
                }
            });
            profile_end!(hdip, mod_key_lookup);
            r
        })
        .and_then(|nmod| {
            if nmod.mod_data.numbers.mod_type == interop::ModType::Deletion as i32 {
                return Some(nmod.mod_data.numbers.mod_type);
            }
            profile_start!(hdip, mod_render);
            // save state
            let mut pDecl: *mut IDirect3DVertexDeclaration9 = null_mut();
            let ppDecl: *mut *mut IDirect3DVertexDeclaration9 = &mut pDecl;
            let hr = (*THIS).GetVertexDeclaration(ppDecl);
            if hr != 0 {
                write_log_file(&format!(
                    "failed to save vertex declaration when trying to render mod {} {}",
                    NumVertices, primCount
                ));
                return None;
            };

            let mut pStreamVB: *mut IDirect3DVertexBuffer9 = null_mut();
            let ppStreamVB: *mut *mut IDirect3DVertexBuffer9 = &mut pStreamVB;
            let mut offsetBytes: UINT = 0;
            let mut stride: UINT = 0;

            let hr = (*THIS).GetStreamSource(0, ppStreamVB, &mut offsetBytes, &mut stride);
            if hr != 0 {
                write_log_file(&format!(
                    "failed to save vertex data when trying to render mod {} {}",
                    NumVertices, primCount
                ));
                if pDecl != null_mut() {
                    (*pDecl).Release();
                }
                return None;
            }

            // Note: C++ code did not change StreamSourceFreq...may need it for some games.
            // draw override
            (*THIS).SetVertexDeclaration(nmod.decl);
            (*THIS).SetStreamSource(0, nmod.vb, 0, nmod.mod_data.numbers.vert_size_bytes as u32);

            // and set mod textures
            let mut save_tex:[Option<*mut IDirect3DBaseTexture9>; 4] = [None; 4];
            let mut _st_rods:Vec<ReleaseOnDrop<*mut IDirect3DBaseTexture9>> = vec![];
            for (i,tex) in nmod.textures.iter().enumerate() {
                if *tex != null_mut() {
                    //write_log_file(&format!("set override tex stage {} to {:x} for mod {}/{}", i, *tex as u64, NumVertices, primCount));
                    let mut save:*mut IDirect3DBaseTexture9 = null_mut();
                    (*THIS).GetTexture(i as u32, &mut save);
                    save_tex[i] = Some(save);
                    (*THIS).SetTexture(i as u32, *tex as *mut IDirect3DBaseTexture9);
                    _st_rods.push(ReleaseOnDrop::new(save));
                }
            }
            
            // set the override tex, which is the (usually) the selection tex.  this might overwrite
            // the mod tex tex we just set.
            let mut save_texture: *mut IDirect3DBaseTexture9 = null_mut();
            let _st_rod = {
                if override_texture != null_mut() {
                    (*THIS).GetTexture(sel_stage, &mut save_texture);
                    (*THIS).SetTexture(sel_stage, override_texture);
                    Some(ReleaseOnDrop::new(save_texture))
                } else {
                    None
                }
            };

            (*THIS).DrawPrimitive(
                nmod.mod_data.numbers.prim_type as u32,
                0,
                nmod.mod_data.numbers.prim_count as u32,
            );
            drew_mod = true;

            // restore state
            (*THIS).SetVertexDeclaration(pDecl);
            (*THIS).SetStreamSource(0, pStreamVB, offsetBytes, stride);
            // restore textures
            for (i,tex) in save_tex.iter().enumerate() {
                tex.map(|tex| {
                    (*THIS).SetTexture(i as u32, tex);
                });
            }
            if override_texture != null_mut() {
                (*THIS).SetTexture(sel_stage, save_texture);
            }
            (*pDecl).Release();
            (*pStreamVB).Release();
            profile_end!(hdip, mod_render);

            Some(nmod.mod_data.numbers.mod_type)
        });
    profile_end!(hdip, main_combinator);

    profile_start!(hdip, draw_input_check);
    // draw input if not modded or if mod is additive
    let draw_input = match modded {
        None => true,
        Some(mtype) if interop::ModType::CPUAdditive as i32 == mtype => true,
        Some(_) => false,
    };
    profile_end!(hdip, draw_input_check);

    profile_start!(hdip, real_dip);
    let dresult = if draw_input {
        let mut save_texture: *mut IDirect3DBaseTexture9 = null_mut();
        let _st_rod = {
            if override_texture != null_mut() {
                (*THIS).GetTexture(sel_stage, &mut save_texture);
                (*THIS).SetTexture(sel_stage, override_texture);
                Some(ReleaseOnDrop::new(save_texture))
            } else {
                None
            }
        };
        let r = (hookdevice.real_draw_indexed_primitive)(
            THIS,
            PrimitiveType,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        );
        if override_texture != null_mut() {
            (*THIS).SetTexture(sel_stage, save_texture);
        }
        r
    } else {
        S_OK
    };
    profile_end!(hdip, real_dip);

    profile_start!(hdip, statistics);
    // statistics
    metrics.dip_calls += 1;
    if metrics.dip_calls % 500_000 == 0 {
        let now = SystemTime::now();
        let elapsed = now.duration_since(metrics.last_call_log);
        match elapsed {
            Ok(d) => {
                let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                if secs >= 10.0 {
                    let dipsec = metrics.dip_calls as f64 / secs;

                    let epocht = now
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_secs(1))
                        .as_secs()
                        * 1000;

                    write_log_file(&format!(
                        "{:?}: {} dip calls in {:.*} secs ({:.*} dips/sec (fps: {:.*}))",
                        epocht, metrics.dip_calls, 2, secs, 2, dipsec, 2, metrics.last_fps
                    ));
                    GLOBAL_STATE.active_texture_set.as_ref().map(|set| {
                        write_log_file(&format!(
                            "active texture set contains: {} textures",
                            set.len()
                        ))
                    });
                    metrics.last_call_log = now;
                    metrics.dip_calls = 0;
                }
            }
            Err(e) => write_log_file(&format!("Error getting elapsed duration: {:?}", e)),
        }
    }
    profile_end!(hdip, statistics);

    GLOBAL_STATE.in_dip = false;
    profile_end!(hdip, hook_dip);

    profile_summarize!(hdip);

    dresult
}


#[cfg(test)]
mod tests {
    use super::*;

    extern crate test;

    use test::*;

    #[allow(unused)]
    pub unsafe extern "system" fn stub_draw_indexed_primitive(
        THIS: *mut IDirect3DDevice9,
        arg1: D3DPRIMITIVETYPE,
        BaseVertexIndex: INT,
        MinVertexIndex: UINT,
        NumVertices: UINT,
        startIndex: UINT,
        primCount: UINT,
    ) -> HRESULT {
        test::black_box(());
        S_OK
    }

    #[allow(unused)]
    pub unsafe extern "system" fn stub_begin_scene(THIS: *mut IDirect3DDevice9) -> HRESULT {
        test::black_box(());
        S_OK
    }

    #[allow(unused)]
    pub unsafe extern "system" fn stub_release(THIS: *mut IUnknown) -> ULONG {
        test::black_box(());
        0
    }

    #[allow(unused)]
    unsafe extern "system" fn stub_present(
        THIS: *mut IDirect3DDevice9,
        pSourceRect: *const RECT,
        pDestRect: *const RECT,
        hDestWindowOverride: HWND,
        pDirtyRegion: *const RGNDATA,
    ) -> HRESULT {
        test::black_box(());
        0
    }

    fn set_stub_device() {
        // let d3d9device = HookDirect3D9Device::new(
        //     stub_draw_indexed_primitive,
        //     stub_begin_scene,
        //     stub_present,
        //     stub_release);
        // set_hook_device(d3d9device);
    }

    #[test]
    fn can_create_d3d9() {
        //use test_e2e;
        // TODO: need to figure out why this behaves poorly WRT test_e2e.
        // is it a side effect of rust's threaded test framework or a system of issues.

        // let _lock = test_e2e::TEST_MUTEX.lock().unwrap();
        // let d3d9 = create_d3d9(32);
        // if let &Err(ref x) = &d3d9 {
        //     assert!(false, format!("unable to create d39: {:?}", x));
        // }
        // unsafe { d3d9.map(|d3d9| (*d3d9).Release()) };
        // let d3d9 = create_d3d9(32);
        // if let &Err(ref x) = &d3d9 {
        //     assert!(false, format!("unable to create d39: {:?}", x));
        // }
        // unsafe { d3d9.map(|d3d9| (*d3d9).Release()) };
        // println!("=============== exiting");
    }
    #[test]
    fn test_state_copy() {
        //set_stub_device();

        // TODO: re-enable when per-scene ops creates clr in global state
        // unsafe {
        //     let device = std::ptr::null_mut();
        //     hook_begin_scene(device);
        //     for _i in 0..10 {
        //         hook_draw_indexed_primitive(device, D3DPT_TRIANGLESTRIP, 0, 0, 0, 0, 0);
        //     }
        // };
    }

    #[bench]
    fn dip_call_time(_b: &mut Bencher) {
        //set_stub_device();

        // Core-i7-6700 3.4Ghz, 1.25 nightly 2018-01-13
        // 878600000 dip calls in 10.0006051 secs (87854683.91307643 dips/sec)
        // 111,695,214 ns/iter (+/- 2,909,577)
        // ~88K calls/millisecond

        // TODO: re-enable when per-scene ops creates clr in global state

        // let device = std::ptr::null_mut();
        // unsafe { hook_begin_scene(device) };
        // b.iter(|| {
        //     let range = 0..10_000_000;
        //     for _r in range {
        //         unsafe { hook_draw_indexed_primitive(device,
        //             D3DPT_TRIANGLESTRIP, 0, 0, 0, 0, 0) };
        //     }
        // });
    }
}
