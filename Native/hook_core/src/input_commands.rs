
use global_state::ANIM_SNAP_STATE;
use shared_dx::types::DevicePointer;
use types::TexPtr;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
use fnv::FnvHashSet;

use std::ptr::null_mut;
use shared_dx::util::*;
use global_state::{hook_state_read, hook_state_write, LOADED_MODS};
use device_state::dev_state_write;
use crate::hook_device_d3d11::apply_device_hook;
use crate::hook_device_d3d11::query_and_set_runconf_in_globalstate;
use crate::hook_render::hook_set_texture;
use crate::global_state::MAX_STAGE;
use crate::hook_render::CLR_OK;
use crate::input;
use crate::mod_render;
use mod_load::AsyncLoadState;

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
    if let Some((_lck, gs)) = hook_state_write() {
        gs.making_selection = true;
        gs.active_texture_list = Some(Vec::with_capacity(5000));
        gs.active_texture_set = Some(FnvHashSet::with_capacity_and_hasher(
            5000,
            Default::default(),
        ));
    }

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
        let already_snapping = hook_state_read()
            .map(|(_lck, gs)| gs.is_snapping)
            .unwrap_or(false);
        if already_snapping {
            return;
        }

        let snap_conf = match SNAP_CONFIG.read() {
            Err(e) => {
                write_log_file(&format!("failed to lock snap config: {}", e));
                return;
            },
            Ok(c) => c
        };

        write_log_file(&format!("init snapshot mode: {}", snap_conf));
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
                expected_primverts,
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
                snap_on_count,
                // worldmat: std::mem::zeroed(),
                // viewmat: std::mem::zeroed(),
                // projmat: std::mem::zeroed(),
            });

            // TODO(perf): should prealloc the scratch arrays used to read from the device in set_vconsts()
            *ANIM_SNAP_STATE.get_mut() = Some(anim_state);
        }

        if let Some((_lck, gs)) = hook_state_write() {
            gs.is_snapping = true;
            gs.snap_start = SystemTime::now();
        }
    }
}

pub fn cmd_select_next_texture(device: DevicePointer) {
    let making_selection = hook_state_read()
        .map(|(_lck, gs)| gs.making_selection)
        .unwrap_or(false);
    if !making_selection {
        init_selection_mode(device)
            .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    if let Some((_lck, gs)) = hook_state_write() {
        let len = gs.active_texture_list.as_ref()
            .map(|list: &Vec<usize>| list.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        gs.curr_texture_index += 1;
        if gs.curr_texture_index >= len {
            gs.curr_texture_index = 0;
        }
    }
}
pub fn cmd_select_prev_texture(device: DevicePointer) {
    let making_selection = hook_state_read()
        .map(|(_lck, gs)| gs.making_selection)
        .unwrap_or(false);
    if !making_selection {
        init_selection_mode(device)
            .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    if let Some((_lck, gs)) = hook_state_write() {
        let len = gs.active_texture_list.as_ref()
            .map(|list: &Vec<usize>| list.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        gs.curr_texture_index = gs.curr_texture_index.wrapping_sub(1);
        if gs.curr_texture_index >= len {
            gs.curr_texture_index = len - 1;
        }
    }
}
fn cmd_clear_texture_lists(device: DevicePointer) {
    tryload_snap_config().map_err(|e| {
        write_log_file(&format!("failed to load snap config: {:?}", e))
    }).unwrap_or_default();

    hook_snapshot::reset();

    let (precopy, force_cpu_read) = hook_state_read()
        .map(|(_lck, gs)| (gs.run_conf.precopy_data, gs.run_conf.force_tex_cpu_read))
        .unwrap_or((false, false));
    if !precopy || !force_cpu_read {
        // Since they pressed the clear texture key that signals they intend to snapshot, so
        // enable precopy regardless of whatever is in the registry.
        // need to set it because apply_device_hook only does createbuffer if it is true
        if let Some((_lck, gs)) = hook_state_write() {
            gs.run_conf.precopy_data = true;
        }
        // query registry to get any additional changes in runconf
        unsafe { query_and_set_runconf_in_globalstate(false); }

        if let Some(true) = device.with_d3d11(|d3d11| unsafe {
            apply_device_hook(d3d11).map(|_| true).map_err(|e| {
                write_log_file(&format!("failed to reapply device hook: {:?}", e))
            }).unwrap_or(false)
        }) {
            write_log_file(&format!("==> precopy data now enabled; it was disabled, so you will need to reload game data for snapshots"));
        }

        // For DX9: log whether force_tex_cpu_read is enabled so the user knows
        // if the CreateTexture hook will redirect DEFAULT pool textures to MANAGED
        // (required for snapshotting most textures). This flag is only set via the
        // SnapForceTexCpuRead registry value; it is not auto-enabled here.
        if let DevicePointer::D3D9(_) = device {
            let force_cpu_read = hook_state_read()
                .map(|(_lck, gs)| gs.run_conf.force_tex_cpu_read)
                .unwrap_or(false);
            if force_cpu_read {
                write_log_file("==> DX9: force_tex_cpu_read is enabled; new textures will use MANAGED pool for snapshotting");
            } else {
                write_log_file("==> DX9: force_tex_cpu_read is disabled; set SnapForceTexCpuRead=1 in registry to enable MANAGED pool redirection for texture snapshotting if required");
            }
        }
    }
    if let Some((_lck, gs)) = hook_state_write() {
        if let Some(list) = gs.active_texture_list.as_mut() { list.clear() }
        if let Some(set) = gs.active_texture_set.as_mut() { set.clear() }
        gs.curr_texture_index = 0;
        for i in 0..MAX_STAGE {
            gs.selected_on_stage[i] = false;
        }
        gs.making_selection = false;
    }
}
pub fn cmd_toggle_show_mods() {
    if let Some((_lck, gs)) = hook_state_write() {
        gs.show_mods = !gs.show_mods;
    }
}
pub fn cmd_take_snapshot() {
    init_snapshot_mode();
}

pub fn is_loading_mods() -> bool {
    // Pull out the bits we need from interop_state under a brief read lock,
    // then drop the lock before calling the managed callback (which can
    // re-enter our hooks).
    let snap = hook_state_read().and_then(|(_lck, gs)| {
        gs.interop_state.as_ref().map(|is| (is.loading_mods, is.callbacks))
    });
    match snap {
        Some((true, _)) => true,
        Some((false, callbacks)) => {
            let loadstate = unsafe { (callbacks.GetLoadingState)() };
            loadstate == AsyncLoadState::InProgress as i32
        },
        None => false,
    }
}

pub fn cmd_clear_mods(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    if let Some((_lck, gs)) = hook_state_write() {
        if let Some(is) = gs.interop_state.as_mut() {
            write_log_file("clearing mods");
            is.loading_mods = false;
            is.done_loading_mods = true;
        } else {
            return;
        }
    } else {
        return;
    }
    unsafe {
        mod_load::clear_loaded_mods(device);
    }
}

fn cmd_reload_mods(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    cmd_clear_mods(device);
    if let Some((_lck, gs)) = hook_state_write() {
        if let Some(is) = gs.interop_state.as_mut() {
            write_log_file("reloading mods");
            is.loading_mods = false;
            is.done_loading_mods = false;
            // the actual reload will be handled in per-frame operations
        }
    }
}

fn cmd_reload_managed_dll(device: DevicePointer) {
    if is_loading_mods() {
        write_log_file("cannot reload now; mods are loading");
        return;
    }
    unsafe { mod_load::clear_loaded_mods(device) };
    // TODO: should check for active snapshotting and anything else that might be using the managed
    // code

    // Snapshot mm_root + run_context out under a read lock; reload_managed_dll
    // calls back into us (OnInitialized) and would deadlock under a write guard.
    let inputs = hook_state_read().and_then(|(_lck, gs)| {
        match gs.clr.runtime_pointer {
            Some(x) if x == CLR_OK => Some((gs.mm_root.clone(), gs.clr.run_context.clone())),
            _ => None,
        }
    });
    if let Some((mm_root, run_context)) = inputs {
        let res = reload_managed_dll(&mm_root, Some(&run_context));
        match res {
            Ok(_) => write_log_file("managed dll reloaded"),
            Err(e) => write_log_file(&format!("ERROR: reloading managed dll failed: {:?}", e))
        }
    }
}

fn select_next_variant() {
    // for any mods that have a variant, select the next one, wrapping around to first if needed.
    // this is currently pretty dumb, since it advances _all_ mods with variants.  if there
    // were a lot of variants of different sizes, it might be better to have multiple keybinds
    // to advance a particular size category, and then partition everything into one of those
    // buckets.  or maybe that means its time to put an imgui UI in here for this purpose.
    let lastframe = hook_state_read()
        .map(|(_lck, gs)| gs.metrics.total_frames)
        .unwrap_or(0);

    match LOADED_MODS.lock() {
        Ok(mut g) => {
            g.as_mut().map(|mstate| {
                mod_render::select_next_variant(mstate, lastframe);
                mod_prefs::save_variant_selections(&mstate.mods, &mstate.selected_variant);
            });
        }
        Err(e) => {
            write_log_file(&format!("select_next_variant: LOADED_MODS lock poisoned: {}", e));
        }
    }
}
fn select_prev_variant() {
    let lastframe = hook_state_read()
        .map(|(_lck, gs)| gs.metrics.total_frames)
        .unwrap_or(0);

    match LOADED_MODS.lock() {
        Ok(mut g) => {
            g.as_mut().map(|mstate| {
                mod_render::select_prev_variant(mstate, lastframe);
                mod_prefs::save_variant_selections(&mstate.mods, &mstate.selected_variant);
            });
        }
        Err(e) => {
            write_log_file(&format!("select_prev_variant: LOADED_MODS lock poisoned: {}", e));
        }
    }
}

fn setup_fkey_input(device: DevicePointer, inp: &mut input::Input) {
    write_log_file("using fkey input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!

    // Allow the handlers to take a copy of the device pointer in the closure.
    // This means that these handlers must be cleared when the device is destroyed,
    // (see purge_device_resources)
    // but lets us avoid passing a context argument through the input layer.
    inp.add_press_fn(input::DIK_F1, Box::new(move || cmd_reload_mods(device)));
    inp.add_press_fn(input::DIK_F2, Box::new(cmd_toggle_show_mods));
    inp.add_press_fn(
        input::DIK_F3,
        Box::new(move || cmd_select_next_texture(device)),
    );
    inp.add_press_fn(
        input::DIK_F4,
        Box::new(move || cmd_select_prev_texture(device)),
    );
    inp.add_press_fn(input::DIK_F6, Box::new(move || cmd_clear_texture_lists(device)));
    inp.add_press_fn(input::DIK_F7, Box::new(cmd_take_snapshot));
    inp.add_press_fn(input::DIK_NUMPAD8, Box::new(select_next_variant));
    inp.add_press_fn(input::DIK_NUMPAD9, Box::new(select_prev_variant));

    // Hot-reload: Ctrl+F10 reloads the managed DLL.  MMManaged.dll is now a thin shell
    // that dynamically loads MMManaged.Engine.dll (the engine implementation).  On reload,
    // the shell reads a fresh copy of the engine DLL from disk and swaps in its callbacks.
    inp.add_press_fn(input::DIK_F10, Box::new(move || cmd_reload_managed_dll(device)));
}

fn setup_punct_input(device: DevicePointer, inp: &mut input::Input) {
    write_log_file("using punct key input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
    inp.add_press_fn(input::DIK_BACKSLASH, Box::new(move || cmd_reload_mods(device)));
    inp.add_press_fn(input::DIK_RBRACKET, Box::new(cmd_toggle_show_mods));
    inp.add_press_fn(input::DIK_SEMICOLON, Box::new(move || cmd_clear_texture_lists(device)));
    inp.add_press_fn(
        input::DIK_COMMA,
        Box::new(move || cmd_select_next_texture(device)),
    );
    inp.add_press_fn(
        input::DIK_PERIOD,
        Box::new(move || cmd_select_prev_texture(device)),
    );
    inp.add_press_fn(input::DIK_SLASH, Box::new(cmd_take_snapshot));

    // Running out of punct!  oh well use these
    inp.add_press_fn(input::DIK_NUMPAD8, Box::new(select_next_variant));
    inp.add_press_fn(input::DIK_NUMPAD9, Box::new(select_prev_variant));

    // Hot-reload managed DLL (Ctrl+F10) - available in all input profiles
    inp.add_press_fn(input::DIK_F10, Box::new(move || cmd_reload_managed_dll(device)));

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
    // Copy the InputProfile bytes out of the locked HookState so we can drop
    // the read lock before doing the input setup work.
    let input_profile_bytes = hook_state_read()
        .and_then(|(_lck, gs)| gs.interop_state.as_ref().map(|is| is.conf_data.InputProfile));
    let inp_profile = match input_profile_bytes {
        None => return Err(HookError::DInputCreateFailed(String::from(
            "no interop state: was device created?",
        ))),
        Some(bytes) => {
            let carr_ptr = &bytes[0] as *const i8;
            unsafe { CStr::from_ptr(carr_ptr) }
                .to_str()
                .map(|s| s.to_owned())
                .map_err(HookError::CStrConvertFailed)?
        }
    };
    let lwr = inp_profile.to_lowercase();
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

        if let Some((_lck, ds)) = dev_state_write() {
            ds.d3d_resource_count += diff;
        }

        if let Some((_lck, gs)) = hook_state_write() {
            gs.selection_texture = Some(TexPtr::D3D9(tex));
        }
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
