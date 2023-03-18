
use shared_dx::types::DevicePointer;
use types::TexPtr;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use fnv::FnvHashSet;
use std;
use std::ptr::null_mut;
use shared_dx::util::*;
use global_state::GLOBAL_STATE;
use device_state::dev_state;
use crate::hook_render::hook_set_texture;
use crate::hook_render::MAX_STAGE;
use crate::hook_render::CLR_OK;
use crate::input;
use mod_load::AsyncLoadState;
use mod_load;
use dnclr::reload_managed_dll;

use shared_dx::error::*;
use util::*;
use winapi::ctypes::c_void;
use std::time::SystemTime;

use snaplib::anim_snap_state::AnimSnapState;
use snaplib::anim_snap_state::AnimConstants;
use hook_snapshot::SNAP_CONFIG;

use snaplib::snap_config::SnapConfig;
use std::collections::HashMap;
use std::collections::HashSet;
use shared_dx::types::DevicePointer::{D3D9, D3D11};

use shared_dx::error::Result;

pub fn init_selection_mode(device: DevicePointer) -> Result<()> {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    hookstate.making_selection = true;
    hookstate.active_texture_list = Some(Vec::with_capacity(5000));
    hookstate.active_texture_set = Some(FnvHashSet::with_capacity_and_hasher(
        5000,
        Default::default(),
    ));

    unsafe {
        // hot-patch the snapshot hook functions
        match device {
            D3D9(device) => {
                let vtbl: *mut IDirect3DDevice9Vtbl = std::mem::transmute((*device).lpVtbl);
                let vsize = std::mem::size_of::<IDirect3DDevice9Vtbl>();

                let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

                // TODO: should hook SetStreamSource so that we can tell what streams are in use
                (*vtbl).SetTexture = hook_set_texture;

                protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
            },
            D3D11(_device) => {
                // currently d3d11 just hooks what it needs from the start
                write_log_file("selection mode initialized")
            }
        }
    }
    Ok(())
}

pub fn init_snapshot_mode() {
    unsafe {
        if GLOBAL_STATE.is_snapping {
            return;
        }

        let snap_conf = match SNAP_CONFIG.read() {
            Err(e) => {
                write_log_file(&format!("failed to lock snap config: {}", e));
                return;
            },
            Ok(c) => c
        };

        if snap_conf.snap_anim {
            hook_snapshot::reset();

            let expected_primverts:HashSet<(UINT,UINT)> =
                match snap_conf.autosnap.as_ref() {
                    Some(hm) => hm.iter().map(|m| (m.prims,m.verts) ).collect(),
                    None => {
                        write_log_file("autosnap hashmap not populated, can't snap anim without it");
                        return;
                    },
                };

            let numcombos = expected_primverts.len();
            let mut anim_state = AnimSnapState {
                next_vconst_idx: 0,
                seen_all: false,
                expected_primverts: expected_primverts,
                seen_primverts: HashSet::new(),
                sequence_vconstants: Vec::new(),
                sequence_start_time: SystemTime::now(), // this will get overwritten when we actually start the constant sequences
                snap_dir: "".to_owned(),
                curr_frame: 0,
                start_frame: 0,
                capture_count_this_frame: HashMap::new(),
            };
            anim_state.seen_primverts.reserve(numcombos * 2);// double to avoid resize while snapping
            anim_state.capture_count_this_frame.reserve(numcombos * 2);
            // prealloc constant array; hopefully we won't exceed this
            let max_seq = snap_conf.max_const_sequences();
            let snap_on_count = snap_conf.snap_anim_on_count;
            anim_state.sequence_vconstants.resize_with(max_seq, || AnimConstants {
                snapped_at: std::time::SystemTime::UNIX_EPOCH,
                prim_count: 0,
                vert_count: 0,
                sequence: 0,
                constants: constant_tracking::ConstantGroup::new(),
                capture_count: 0,
                frame: 0,
                player_transform: Err(HookError::SnapshotFailed("".to_owned())),
                snap_on_count: snap_on_count,
                // worldmat: std::mem::zeroed(),
                // viewmat: std::mem::zeroed(),
                // projmat: std::mem::zeroed(),
            });

            // TODO(perf): should prealloc the scratch arrays used to read from the device in set_vconsts()
            GLOBAL_STATE.anim_snap_state = Some(anim_state);
        }

        GLOBAL_STATE.is_snapping = true;
        GLOBAL_STATE.snap_start = SystemTime::now();
    }
}

pub fn cmd_select_next_texture(device: DevicePointer) {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    if !hookstate.making_selection {
        init_selection_mode(device)
            .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    let len = hookstate
        .active_texture_list
        .as_mut()
        .map(|list| list.len())
        .unwrap_or(0);

    if len == 0 {
        return;
    }

    hookstate.curr_texture_index += 1;
    if hookstate.curr_texture_index >= len {
        hookstate.curr_texture_index = 0;
    }
}
pub fn cmd_select_prev_texture(device: DevicePointer) {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    if !hookstate.making_selection {
        init_selection_mode(device)
            .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    let len = hookstate
        .active_texture_list
        .as_mut()
        .map(|list| list.len())
        .unwrap_or(0);

    if len == 0 {
        return;
    }

    hookstate.curr_texture_index = hookstate.curr_texture_index.wrapping_sub(1);
    if hookstate.curr_texture_index >= len {
        hookstate.curr_texture_index = len - 1;
    }
}
fn cmd_clear_texture_lists(_device: DevicePointer) {
    tryload_snap_config().map_err(|e| {
        write_log_file(&format!("failed to load snap config: {:?}", e))
    }).unwrap_or_default();

    hook_snapshot::reset();

    unsafe {
        GLOBAL_STATE
            .active_texture_list
            .as_mut()
            .map(|list| list.clear());
        GLOBAL_STATE
            .active_texture_set
            .as_mut()
            .map(|list| list.clear());
        GLOBAL_STATE.curr_texture_index = 0;
        for i in 0..MAX_STAGE {
            GLOBAL_STATE.selected_on_stage[i] = false;
        }
        GLOBAL_STATE.making_selection = false;

        // TODO: this was an attempt to fix the issue with the selection
        // texture getting clobbered after alt-tab, but it didn't work.
        // for now I just use windowed mode in the affected game.  Doesn't happen with all games.
        // if GLOBAL_STATE.selection_texture != null_mut() {
        //     let mut tex: *mut IDirect3DTexture9 = GLOBAL_STATE.selection_texture;
        //     if tex != null_mut() {
        //         (*tex).Release();
        //     }
        //     GLOBAL_STATE.selection_texture = null_mut();
        //     create_selection_texture(device);
        // }
    }
}
pub fn cmd_toggle_show_mods() {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    hookstate.show_mods = !hookstate.show_mods;
}
pub fn cmd_take_snapshot() {
    init_snapshot_mode();
}

pub fn is_loading_mods() -> bool {
    let interop_state = unsafe { &mut GLOBAL_STATE.interop_state };
    let loading = interop_state.as_mut().map(|is| {
        if is.loading_mods {
            return true;
        }
        let loadstate = unsafe { (is.callbacks.GetLoadingState)() };
        if loadstate == AsyncLoadState::InProgress as i32 {
            return true;
        }
        false
    }).unwrap_or(false);
    loading
}

pub fn cmd_clear_mods(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    let interop_state = unsafe { &mut GLOBAL_STATE.interop_state };
    interop_state.as_mut().map(|is| {
        write_log_file("clearing mods");
        is.loading_mods = false;
        is.done_loading_mods = true;

        unsafe {
            mod_load::clear_loaded_mods(device);
        }
    });
}

fn cmd_reload_mods(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    cmd_clear_mods(device);
    let interop_state = unsafe { &mut GLOBAL_STATE.interop_state };
    interop_state.as_mut().map(|is| {
        write_log_file("reloading mods");
        is.loading_mods = false;
        is.done_loading_mods = false;

        // the actual reload will be handled in per-frame operations
    });
}

fn cmd_reload_managed_dll(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    unsafe { mod_load::clear_loaded_mods(device) };
    // TODO: should check for active snapshotting and anything else that might be using the managed
    // code
    let hookstate = unsafe { &mut GLOBAL_STATE };
    match hookstate.clr.runtime_pointer {
        Some(x) if x == CLR_OK => {
            let ctx = &hookstate.clr.run_context;
            let res = reload_managed_dll(&hookstate.mm_root, Some(ctx));
            match res {
                Ok(_) => write_log_file("managed dll reloaded"),
                Err(e) => write_log_file(&format!("ERROR: reloading managed dll failed: {:?}", e))
            }
        },
        _ => ()
    };
}

fn select_next_variant() {
    // for any mods that have a variant, select the next one, wrapping around to first if needed.
    // this is currently pretty dumb, since it advances _all_ mods with variants.  if there
    // were a lot of variants of different sizes, it might be better to have multiple keybinds
    // to advance a particular size category, and then partition everything into one of those
    // buckets.  or maybe that means its time to put an imgui UI in here for this purpose.
    let hookstate = unsafe { &mut GLOBAL_STATE };
    let lastframe = hookstate.metrics.total_frames;

    hookstate.loaded_mods.as_mut().map(|mstate| {
        for (mkey, nmdv) in mstate.mods.iter() {
            if nmdv.len() <= 1 {
                // most mods have no variants
                continue;
            }

            // don't change the selection if none have been rendered recently
            let foundrecent = nmdv.iter().find(|nmd| nmd.recently_rendered(lastframe));
            if foundrecent.is_none() {
                continue;
            }

            // get the current variant for this mod
            let sel_index_entry = mstate.selected_variant.entry(*mkey).or_insert(0);
            let mut sel_index = *sel_index_entry;
            let start = sel_index;
            // select next, skipping over child mods.  stop if we wrap to where we started
            sel_index += 1;
            loop {
                if sel_index >= nmdv.len() {
                    sel_index = 0;
                }
                if sel_index == start {
                    break;
                }
                if nmdv[sel_index].parent_mod_names.is_empty() {
                    // found one
                    write_log_file(&format!("selected next variant: {}", nmdv[sel_index].name));
                    *sel_index_entry = sel_index;
                    break;
                }
                // keep looking
                sel_index += 1;
            }
        }
    });
}

fn setup_fkey_input(device: DevicePointer, inp: &mut input::Input) {
    write_log_file("using fkey input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!

    // Allow the handlers to take a copy of the device pointer in the closure.
    // This means that these handlers must be cleared when the device is destroyed,
    // (see purge_device_resources)
    // but lets us avoid passing a context argument through the input layer.
    inp.add_press_fn(input::DIK_F1, Box::new(move || cmd_reload_mods(device)));
    inp.add_press_fn(input::DIK_F2, Box::new(|| cmd_toggle_show_mods()));
    inp.add_press_fn(
        input::DIK_F3,
        Box::new(move || cmd_select_next_texture(device)),
    );
    inp.add_press_fn(
        input::DIK_F4,
        Box::new(move || cmd_select_prev_texture(device)),
    );
    inp.add_press_fn(input::DIK_F6, Box::new(move || cmd_clear_texture_lists(device)));
    inp.add_press_fn(input::DIK_F7, Box::new(move || cmd_take_snapshot()));
    inp.add_press_fn(input::DIK_NUMPAD8, Box::new(move || select_next_variant()));
    inp.add_press_fn(input::DIK_NUMPAD9, Box::new(move || select_next_variant()));

    // Disabling this because its ineffective: the reload will complete without error, but
    // The old managed code will still be used.  The old C++ code
    // used a custom domain manager to support reloading, but I'd rather just move to the
    // CoreCLR rather than reimplement that.
    //inp.add_press_fn(input::DIK_F10, Box::new(move || cmd_reload_managed_dll(device)));
}

fn setup_punct_input(device: DevicePointer, inp: &mut input::Input) {
    write_log_file("using punct key input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
    inp.add_press_fn(input::DIK_BACKSLASH, Box::new(move || cmd_reload_mods(device)));
    inp.add_press_fn(input::DIK_RBRACKET, Box::new(|| cmd_toggle_show_mods()));
    inp.add_press_fn(input::DIK_SEMICOLON, Box::new(move || cmd_clear_texture_lists(device)));
    inp.add_press_fn(
        input::DIK_COMMA,
        Box::new(move || cmd_select_next_texture(device)),
    );
    inp.add_press_fn(
        input::DIK_PERIOD,
        Box::new(move || cmd_select_prev_texture(device)),
    );
    inp.add_press_fn(input::DIK_SLASH, Box::new(move || cmd_take_snapshot()));

    // Running out of punct!  oh well use these
    inp.add_press_fn(input::DIK_NUMPAD8, Box::new(move || select_next_variant()));
    inp.add_press_fn(input::DIK_NUMPAD9, Box::new(move || select_next_variant()));

    // _punctKeyMap[DIK_MINUS] = [&]() { this->loadEverything(); };
}

pub fn setup_input(device: DevicePointer, inp: &mut input::Input) -> Result<()> {
    use std::ffi::CStr;

    // if we fail to set it up repeatedly, don't spam log forever
    inp.setup_attempts += 1;
    if inp.setup_attempts == 10 {
        return Err(HookError::DInputCreateFailed(String::from(
            "too many calls to setup_input, further calls will be ignored",
        )));
    }
    if inp.setup_attempts > 10 {
        return Ok(())
        // return Err(HookError::DInputCreateFailed(format!(
        //     "ignoring setup call: {}", inp.setup_attempts
        // )));
    }

    // Set key bindings.  Input also assumes that CONTROL modifier is required for these as well.
    // TODO: should push this out to conf file eventually so that they can be customized without rebuild
    let interop_state = unsafe { &GLOBAL_STATE.interop_state };
    interop_state
        .as_ref()
        .ok_or(HookError::DInputCreateFailed(String::from(
            "no interop state: was device created?",
        )))
        .and_then(|is| {
            let carr_ptr = &is.conf_data.InputProfile[0] as *const i8;
            unsafe { CStr::from_ptr(carr_ptr) }
                .to_str()
                .map_err(|e| HookError::CStrConvertFailed(e))
        })
        .and_then(|inp_profile| {
            let lwr = inp_profile.to_owned().to_lowercase();
            if lwr.starts_with("fk") {
                setup_fkey_input(device, inp);
            } else if lwr.starts_with("punct") {
                setup_punct_input(device, inp);
            } else {
                write_log_file(&format!(
                    "input scheme unrecognized: {}, using FKeys",
                    inp_profile
                ));
                setup_fkey_input(device, inp);
            }
            Ok(())
        })
}

pub (crate) fn create_selection_texture_d3d9(device: *mut IDirect3DDevice9) {
    unsafe {
        let width = 256;
        let height = 256;

        (*device).AddRef();
        let pre_rc = (*device).Release();

        let mut tex: *mut IDirect3DTexture9 = null_mut();
        let hr = (*device).CreateTexture(
            width,
            height,
            1,
            0,
            D3DFMT_A8R8G8B8,
            D3DPOOL_MANAGED,
            &mut tex,
            null_mut(),
        );
        if hr != 0 {
            write_log_file(&format!("failed to create selection texture: {:x}", hr));
            return;
        }

        // fill it with a lovely shade of green
        let mut rect: D3DLOCKED_RECT = std::mem::zeroed();
        let hr = (*tex).LockRect(0, &mut rect, null_mut(), D3DLOCK_DISCARD);
        if hr != 0 {
            write_log_file(&format!("failed to lock selection texture: {:x}", hr));
            (*tex).Release();
            return;
        }

        let dest: *mut u32 = std::mem::transmute(rect.pBits);
        for i in 0..width * height {
            let d: *mut u32 = dest.offset(i as isize);
            *d = 0xFF00FF00;
        }
        let hr = (*tex).UnlockRect(0);
        if hr != 0 {
            write_log_file("failed to unlock selection texture");
            (*tex).Release();
            return;
        }
        write_log_file("created selection texture");

        (*device).AddRef();
        let post_rc = (*device).Release();
        let diff = post_rc - pre_rc;

        dev_state().d3d_resource_count += diff;

        GLOBAL_STATE.selection_texture = Some(TexPtr::D3D9(tex));
    }
}


fn tryload_snap_config() -> Result<()> {
    let (_,dir) = get_mm_conf_info()?;
    let dir = dir.ok_or_else(|| HookError::SnapshotFailed("no mm root dir".to_owned()))?;

    let sc = SnapConfig::load(&dir)?;
    let mut sclock = SNAP_CONFIG.write().map_err(|e| HookError::SnapshotFailed(format!("failed to lock snap config: {}", e)))?;
    *sclock = sc;
    drop(sclock);

    let sclock = SNAP_CONFIG.read().map_err(|e| HookError::SnapshotFailed(format!("failed to lock snap config: {}", e)))?;
    write_log_file(&format!("loaded snap config: {}", *sclock));
    Ok(())
}
