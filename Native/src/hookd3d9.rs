use winapi::um::unknwnbase::IUnknown;

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::um::winnt::HRESULT;
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::winuser::{GetForegroundWindow, GetParent, GetAncestor};
use winapi::ctypes::c_void;
use winapi::um::wingdi::RGNDATA;

use fnv::FnvHashMap;
use fnv::FnvHashSet;

use util::*;
use util;
use dnclr::init_clr;
use interop::InteropState;
use interop::NativeModData;
use interop;
use input;

use std;
use std::fmt;
use std::time::SystemTime;
use std::ptr::null_mut;

pub type CreateDeviceFn =
    unsafe extern "system" fn(
        THIS: *mut IDirect3D9,
        Adapter: UINT,
        DeviceType: D3DDEVTYPE,
        hFocusWindow: HWND,
        BehaviorFlags: DWORD,
        pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
        ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
    ) -> HRESULT;
pub type DrawIndexedPrimitiveFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    arg1: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT;
pub type BeginSceneFn = unsafe extern "system" fn(THIS: *mut IDirect3DDevice9) -> HRESULT;
pub type IUnknownReleaseFn = unsafe extern "system" fn(THIS: *mut IUnknown) -> ULONG;
pub type PresentFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT;
pub type SetTextureFn = unsafe extern "system" fn(
    THIS: *mut IDirect3DDevice9, Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,) -> HRESULT;

pub struct HookDirect3D9 {
    pub real_create_device: CreateDeviceFn,
}

#[derive(Copy, Clone)]
pub struct HookDirect3D9Device {
    pub real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
    //pub real_begin_scene: BeginSceneFn,
    pub real_present: PresentFn,
    pub real_release: IUnknownReleaseFn,
    pub real_set_texture: SetTextureFn,
    pub ref_count: ULONG,
    pub dip_calls: u32,
    pub frames: u32,
    pub last_call_log: SystemTime,
    pub last_frame_log: SystemTime,
    pub last_fps: f64,
    pub last_fps_update: SystemTime,
    pub low_framerate: bool,
}

impl HookDirect3D9Device {
    pub fn new(
        real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
        //real_begin_scene: BeginSceneFn,
        real_present: PresentFn,
        real_release: IUnknownReleaseFn,
        real_set_texture: SetTextureFn,
    ) -> HookDirect3D9Device {
        HookDirect3D9Device {
            real_draw_indexed_primitive: real_draw_indexed_primitive,
            //real_begin_scene: real_begin_scene,
            real_release: real_release,
            real_present: real_present,
            real_set_texture: real_set_texture,
            dip_calls: 0,
            frames: 0,
            ref_count: 0,
            last_call_log: SystemTime::now(),
            last_frame_log: SystemTime::now(),
            last_fps_update: SystemTime::now(),
            last_fps: 120.0,
            low_framerate: false,
        }
    }
}

const MAX_STAGE:usize = 16;

pub struct HookState {
    pub hook_direct3d9: Option<HookDirect3D9>,
    pub hook_direct3d9device: Option<HookDirect3D9Device>,
    pub clr_pointer: Option<u64>,
    pub d3d_window: HWND,
    pub interop_state: Option<InteropState>,
    pub is_global: bool,
    pub loaded_mods: Option<FnvHashMap<u32, interop::NativeModData>>,
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
    // TODO: this should be tracked per device pointer.
    pub d3d_resource_count: u32,
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
            "HookState (thread: {:?}): d3d9: {:?}, device: {:?}",
            std::thread::current().id(),
            self.hook_direct3d9.is_some(),
            self.hook_direct3d9device.is_some()
        )
    }
}

lazy_static! {
    pub static ref GLOBAL_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
// TODO: maybe create read/write accessors for this
pub static mut GLOBAL_STATE: HookState = HookState {
    hook_direct3d9: None,
    hook_direct3d9device: None,
    clr_pointer: None,
    interop_state: None,
    d3d_window: null_mut(),
    is_global: true,
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
    selected_on_stage: [false;MAX_STAGE],
    curr_texture_index: 0,
    d3d_resource_count: 0,
};

enum AsyncLoadState {
    NotStarted = 51,
    Pending,
    InProgress,
    Complete,
}

fn get_current_texture() -> *mut IDirect3DBaseTexture9 {
    unsafe {
        let idx = GLOBAL_STATE.curr_texture_index;
        GLOBAL_STATE.active_texture_list.as_ref().map(|list| {
            if idx > list.len() {
                null_mut()
            } else {
                list[idx]
            }

        }).unwrap_or(null_mut())
    }
}

fn get_selected_texture_stage_() -> Option<DWORD> {
    unsafe {
        for i in 0..MAX_STAGE {
            if GLOBAL_STATE.selected_on_stage[i] {
                return Some(i as DWORD)
            }
        }
        None
    }
}

pub fn get_global_state_ptr() -> *mut HookState {
    let pstate: *mut HookState = unsafe { &mut GLOBAL_STATE };
    pstate
}

unsafe fn clear_loaded_mods(device: *mut IDirect3DDevice9) {
    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to clear mod data");
        return;
    }

    // get device ref count prior to adding everything
    (*device).AddRef();
    let pre_rc = (*device).Release();

    let mods = GLOBAL_STATE.loaded_mods.take();
    let mut cnt = 0;
    mods.map(|mods| {
        for (_key, nmd) in mods.into_iter() {
            cnt += 1;
            if nmd.vb != null_mut() {
                (*nmd.vb).Release();
            }
            if nmd.ib != null_mut() {
                (*nmd.ib).Release();
            }
            if nmd.decl != null_mut() {
                (*nmd.decl).Release();
            }
        }
    });

    (*device).AddRef();
    let post_rc = (*device).Release();
    let diff = pre_rc - post_rc;
    if (GLOBAL_STATE.d3d_resource_count as i64 - diff as i64) < 0  {
        write_log_file(&format!("DOH resource count would go below zero (curr: {}, removed {}),"
            ,GLOBAL_STATE.d3d_resource_count,diff));
    } else {
        GLOBAL_STATE.d3d_resource_count -= diff;
    }

    write_log_file(&format!("unloaded {} mods", cnt));
}

unsafe fn setup_mod_data(device: *mut IDirect3DDevice9, callbacks: interop::ManagedCallbacks) {
    clear_loaded_mods(device);

    let mod_count = (callbacks.GetModCount)();
    if mod_count <= 0 {
        return;
    }

    if device == null_mut() {
        return;
    }

    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to setup mod data");
        return;
    }

    // get device ref count prior to adding everything
    (*device).AddRef();
    let pre_rc = (*device).Release();

    let mut loaded_mods: FnvHashMap<u32, interop::NativeModData> =
        FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());

    for midx in 0..mod_count {
        let mdat: *mut interop::ModData = (callbacks.GetModData)(midx);

        if mdat == null_mut() {
            write_log_file(&format!("null mod at index {}", midx));
            continue;
        }

        let mod_type = (*mdat).numbers.mod_type;
        if mod_type != interop::ModType::GPUReplacement as i32
            && mod_type != interop::ModType::Deletion as i32
        {
            write_log_file(&format!(
                "Unsupported mod type: {}",
                (*mdat).numbers.mod_type
            ));
            continue;
        }

        let mut native_mod_data = interop::NativeModData {
            mod_data: (*mdat),
            vb_data: null_mut(),
            ib_data: null_mut(),
            decl_data: null_mut(),
            vb: null_mut(),
            ib: null_mut(),
            decl: null_mut(),
        };

        if (*mdat).numbers.mod_type == (interop::ModType::Deletion as i32) {
            let hash_code = NativeModData::mod_key(
                native_mod_data.mod_data.numbers.ref_vert_count as u32,
                native_mod_data.mod_data.numbers.ref_prim_count as u32,
            );

            loaded_mods.insert(hash_code, native_mod_data);
            // thats all we need to do for these.
            continue;
        }

        let decl_size = (*mdat).numbers.decl_size_bytes;
        // vertex declaration construct copies the vec bytes, so just keep a temp vector reference for the data
        let (decl_data, _decl_vec) = if decl_size > 0 {
            let mut decl_vec: Vec<u8> = Vec::with_capacity(decl_size as usize);
            let decl_data: *mut u8 = decl_vec.as_mut_ptr();
            (decl_data, Some(decl_vec))
        } else {
            (null_mut(), None)
        };

        let vb_size = (*mdat).numbers.prim_count * 3 * (*mdat).numbers.vert_size_bytes;
        let mut vb_data: *mut u8 = null_mut();

        // index buffers not currently supported
        let ib_size = 0; //mdat->indexCount * mdat->indexElemSizeBytes;
        let ib_data: *mut u8 = null_mut();

        // create vb
        let mut out_vb: *mut IDirect3DVertexBuffer9 = null_mut();
        let out_vb: *mut *mut IDirect3DVertexBuffer9 = &mut out_vb;
        let hr = (*device).CreateVertexBuffer(
            vb_size as UINT,
            D3DUSAGE_WRITEONLY,
            0,
            D3DPOOL_MANAGED,
            out_vb,
            null_mut(),
        );
        if hr != 0 {
            write_log_file(&format!(
                "failed to create vertex buffer for mod {}: HR {:x}",
                midx, hr
            ));
            return;
        }

        // TODO: // this->add(nModData.vb);

        let vb = *out_vb;

        // lock vb to obtain write buffer
        let hr = (*vb).Lock(0, 0, std::mem::transmute(&mut vb_data), 0);
        if hr != 0 {
            write_log_file(&format!("failed to lock vertex buffer: {:x}", hr));
            return;
        }

        // fill all data buckets with managed code
        let ret = (callbacks.FillModData)(
            midx,
            decl_data,
            decl_size,
            vb_data,
            vb_size,
            ib_data,
            ib_size,
        );

        let hr = (*vb).Unlock();
        if hr != 0 {
            write_log_file(&format!("failed to unlock vertex buffer: {:x}", hr));
            (*vb).Release();
            return;
        }

        if ret != 0 {
            write_log_file(&format!("failed to fill mod data: {}", ret));
            (*vb).Release();
            return;
        }

        native_mod_data.vb = vb;

        // create vertex declaration
        let mut out_decl: *mut IDirect3DVertexDeclaration9 = null_mut();
        let pp_out_decl: *mut *mut IDirect3DVertexDeclaration9 = &mut out_decl;
        let hr =
            (*device).CreateVertexDeclaration(decl_data as *const D3DVERTEXELEMENT9, pp_out_decl);
        if hr != 0 {
            write_log_file(&format!("failed to create vertex declaration: {}", hr));
            (*vb).Release();
            return;
        }
        if out_decl == null_mut() {
            write_log_file("vertex declaration is null");
            (*vb).Release();
            return;
        }
        native_mod_data.decl = out_decl;

        // TODO textures

        let mod_key = NativeModData::mod_key(
            native_mod_data.mod_data.numbers.ref_vert_count as u32,
            native_mod_data.mod_data.numbers.ref_prim_count as u32,
        );
        loaded_mods.insert(mod_key, native_mod_data);

        write_log_file(&format!(
            "allocated vb/decl for mod data {}: {:?}",
            midx,
            (*mdat).numbers
        ));
    }

    // get new ref count
    (*device).AddRef();
    let post_rc = (*device).Release();
    let diff = post_rc - pre_rc;
    GLOBAL_STATE.d3d_resource_count += diff;
    write_log_file(&format!("mod loading added {} to device {:x} ref count, new count: {}",
        diff, device as u64, GLOBAL_STATE.d3d_resource_count));

    GLOBAL_STATE.loaded_mods = Some(loaded_mods);
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
                                hookstate.clr_pointer = Some(1);
                                Ok(_x)
                            })
                            .map_err(|e| {
                                write_log_file(&format!("Error creating CLR: {:?}", e));
                                hookstate.clr_pointer = Some(666);
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

            unsafe { setup_mod_data(device, is.callbacks) };
        }
    });
    Ok(())
}

unsafe extern "system" fn hook_set_texture(THIS: *mut IDirect3DDevice9, Stage: DWORD,
    pTexture: *mut IDirect3DBaseTexture9,) -> HRESULT {
            let has_it = GLOBAL_STATE.active_texture_set.as_ref()
                .map(|set| set.contains(&pTexture)).unwrap_or(true);
            if !has_it {
                GLOBAL_STATE.active_texture_set.as_mut()
                    .map(|set| {
                        set.insert(pTexture);
                    });
                GLOBAL_STATE.active_texture_list.as_mut()
                    .map(|list| {
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

            (GLOBAL_STATE.hook_direct3d9device.unwrap().real_set_texture)(
            THIS,
            Stage,
            pTexture)
}

fn init_selection_mode(device:*mut IDirect3DDevice9) -> Result<()> {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    hookstate.making_selection = true;
    hookstate.active_texture_list = Some(Vec::with_capacity(5000));
    hookstate.active_texture_set =
        Some(FnvHashSet::with_capacity_and_hasher(5000, Default::default()));

    unsafe {
        // hot-patch the snapshot hook functions
        let vtbl: *mut IDirect3DDevice9Vtbl = std::mem::transmute((*device).lpVtbl);
        let vsize = std::mem::size_of::<IDirect3DDevice9Vtbl>();

        let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

        (*vtbl).SetTexture = hook_set_texture;

        protect_memory(vtbl as *mut c_void, vsize, old_prot)?;
    }
    Ok(())
}

fn cmd_select_next_texture(device:*mut IDirect3DDevice9) {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    if !hookstate.making_selection {
        init_selection_mode(device)
        .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    let len = hookstate.active_texture_list.as_mut().map(|list| {
        list.len()
    }).unwrap_or(0);

    if len == 0 {
        return;
    }

    hookstate.curr_texture_index += 1;
    if hookstate.curr_texture_index >= len {
        hookstate.curr_texture_index = 0;
    }
}
fn cmd_select_prev_texture(device:*mut IDirect3DDevice9) {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    if !hookstate.making_selection {
        init_selection_mode(device)
        .unwrap_or_else(|_e| write_log_file("woops couldn't init selection mode"));
    }

    let len = hookstate.active_texture_list.as_mut().map(|list| {
        list.len()
    }).unwrap_or(0);

    if len == 0 {
        return;
    }

    hookstate.curr_texture_index = hookstate.curr_texture_index.wrapping_sub(1);
    if hookstate.curr_texture_index >= len {
        hookstate.curr_texture_index = len - 1;
    }
}
fn cmd_toggle_show_mods() {
    let hookstate = unsafe { &mut GLOBAL_STATE };
    hookstate.show_mods = !hookstate.show_mods;
}

fn setup_fkey_input(device:*mut IDirect3DDevice9, inp: &mut input::Input) {
    write_log_file("using fkey input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
    // _fKeyMap[DIK_F1] = [&]() { this->loadMods(); };
    // _fKeyMap[DIK_F2] = [&]() { this->toggleShowModMesh(); };
    // _fKeyMap[DIK_F6] = [&]() { this->clearTextureLists(); };
    // _fKeyMap[DIK_F3] = [&]() { this->selectNextTexture(); };
    // _fKeyMap[DIK_F4] = [&]() { this->selectPrevTexture(); };
    // _fKeyMap[DIK_F7] = [&]() { this->requestSnap(); };
    // _fKeyMap[DIK_F10] = [&]() { this->loadEverything(); };

    // Allow the handlers to take a copy of the device pointer in the closure.
    // This means that these handlers must be cleared when the device is destroyed,
    // (see purge_device_resources)
    // but lets us avoid passing a context argument through the input layer.
    inp.add_press_fn(input::DIK_F2, Box::new(|| cmd_toggle_show_mods()));
    inp.add_press_fn(input::DIK_F3, Box::new(move || cmd_select_next_texture(device)));
    inp.add_press_fn(input::DIK_F4, Box::new(move || cmd_select_prev_texture(device)));
}

fn setup_punct_input(_device:*mut IDirect3DDevice9, _inp: &mut input::Input) {
    write_log_file("using punct key input layout");
    // If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
    // TODO: hook these up
    // _punctKeyMap[DIK_BACKSLASH] = [&]() { this->loadMods(); };
    // _punctKeyMap[DIK_RBRACKET] = [&]() { this->toggleShowModMesh(); };
    // _punctKeyMap[DIK_SEMICOLON] = [&]() { this->clearTextureLists(); };
    // _punctKeyMap[DIK_COMMA] = [&]() { this->selectNextTexture(); };
    // _punctKeyMap[DIK_PERIOD] = [&]() { this->selectPrevTexture(); };
    // _punctKeyMap[DIK_SLASH] = [&]() { this->requestSnap(); };
    // _punctKeyMap[DIK_MINUS] = [&]() { this->loadEverything(); };
}

fn setup_input(device:*mut IDirect3DDevice9, inp: &mut input::Input) -> Result<()> {
    use std::ffi::CStr;

    // Set key bindings.  Input also assumes that CONTROL modifier is required for these as well.
    // TODO: should push this out to conf file eventually so that they can be customized without rebuild
    let interop_state = unsafe { &GLOBAL_STATE.interop_state };
    interop_state
        .as_ref()
        .ok_or(HookError::DInputCreateFailed(String::from(
            "no interop state",
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
                setup_fkey_input(device,inp);
            } else if lwr.starts_with("punct") {
                setup_punct_input(device,inp);
            } else {
                write_log_file(&format!(
                    "input scheme unrecognized: {}, using FKeys",
                    inp_profile
                ));
                setup_fkey_input(device,inp);
            }
            Ok(())
        })
}

fn appwnd_is_foreground() -> bool {
    const GA_ROOTOWNER:UINT = 3;

    unsafe {
        let gs = &GLOBAL_STATE;
        if gs.d3d_window == null_mut() {
            return false;
        }
        let focus_wnd = GetForegroundWindow();
        let mut is_focused = focus_wnd == gs.d3d_window;
        if !is_focused {
            // check parent
            let par = GetParent(gs.d3d_window);
            is_focused = par == focus_wnd;
        }
        if !is_focused {
            // check root owner
            let own = GetAncestor(gs.d3d_window, GA_ROOTOWNER);
            is_focused = own == focus_wnd;
        }
        is_focused
    }
}

fn create_selection_texture(device:*mut IDirect3DDevice9) {
    unsafe {
        let width = 256;
        let height = 256;

        (*device).AddRef();
        let pre_rc = (*device).Release();

        let mut tex:*mut IDirect3DTexture9 = null_mut();
        let hr = (*device).CreateTexture(width, height, 1, 0, D3DFMT_A8R8G8B8, D3DPOOL_MANAGED,
            &mut tex, null_mut());
        if hr != 0 {
            write_log_file(&format!("failed to create selection texture: {:x}", hr));
            return;
        }

        // fill it with a lovely shade of green
        let mut rect:D3DLOCKED_RECT = std::mem::zeroed();
        let hr = (*tex).LockRect(0, &mut rect, null_mut(), D3DLOCK_DISCARD);
        if hr != 0 {
            write_log_file(&format!("failed to lock selection texture: {:x}", hr));
            (*tex).Release();
            return;
        }

        let dest:*mut u32 = std::mem::transmute(rect.pBits);
        for i in 0..width*height {
            let d:*mut u32 = dest.offset(i as isize);
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

        GLOBAL_STATE.d3d_resource_count += diff;

        GLOBAL_STATE.selection_texture = tex;
    }
}

// TODO: hook this up to device release at the proper time
unsafe fn purge_device_resources(device:*mut IDirect3DDevice9) {
    if device == null_mut() {
        write_log_file("WARNING: ignoring insane attempt to purge devices on a null device");
        return;
    }
    clear_loaded_mods(device);
    if GLOBAL_STATE.selection_texture != null_mut() {
        (*GLOBAL_STATE.selection_texture).Release();
        GLOBAL_STATE.selection_texture = null_mut();
    }
    GLOBAL_STATE.input.as_mut().map(|input| input.clear_handlers());
    GLOBAL_STATE.d3d_resource_count = 0;
}

pub unsafe extern "system" fn hook_present(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT {
    if GLOBAL_STATE.in_any_hook_fn() {
        return (GLOBAL_STATE.hook_direct3d9device.unwrap().real_present)(
            THIS,
            pSourceRect,
            pDestRect,
            hDestWindowOverride,
            pDirtyRegion,
        );
    }

    if let Err(e) = do_per_frame_operations(THIS) {
        write_log_file(&format!("unexpected error from do_per_scene_operations: {:?}", e));
        return (GLOBAL_STATE.hook_direct3d9device.unwrap().real_present)(
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

    let present_ret = GLOBAL_STATE
        .hook_direct3d9device
        .as_mut()
        .map_or(S_OK, |hookdevice| {
            hookdevice.frames += 1;
            if hookdevice.frames % 90 == 0 {
                // enforce min fps
                // NOTE: when low, it just sets a boolean flag to disable mod rendering,
                // but we could also use virtual protect to temporarily swap out the hook functions
                // (except for present)
                let now = SystemTime::now();
                let elapsed = now.duration_since(hookdevice.last_fps_update);
                if let Ok(d) = elapsed {
                    let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                    let fps = hookdevice.frames as f64 / secs;
                    let smooth_fps = 0.3 * fps + 0.7 * hookdevice.last_fps;
                    hookdevice.last_fps = smooth_fps;
                    let min_off = min_fps * 1.1;
                    if smooth_fps < min_fps && !hookdevice.low_framerate {
                        hookdevice.low_framerate = true;
                    }
                    // prevent oscillation: don't reactivate until 10% above mininum
                    else if hookdevice.low_framerate && smooth_fps > (min_off * 1.1) {
                        hookdevice.low_framerate = false;
                    }
                    // write_log_file(&format!(
                    //     "{} frames in {} secs ({} instant, {} smooth) (low: {})",
                    //     hookdevice.frames, secs, fps, smooth_fps, hookdevice.low_framerate
                    // ));
                    hookdevice.last_fps_update = now;
                    hookdevice.frames = 0;
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
        create_selection_texture(THIS);
    }

    if appwnd_is_foreground() {
        GLOBAL_STATE.input.as_mut().map(|inp| {
            if inp.get_press_fn_count() == 0 {
                setup_input(THIS, inp)
                    .unwrap_or_else(|e| write_log_file(&format!("input setup error: {:?}", e)));
            }
            inp.process()
                .unwrap_or_else(|e| write_log_file(&format!("input error: {:?}", e)));
        });
    }

    present_ret
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
    // TODO: hack to work around Release on device while in DIP
    if GLOBAL_STATE.in_hook_release {
        return (GLOBAL_STATE.hook_direct3d9device.unwrap().real_release)(THIS);
    }

    GLOBAL_STATE.in_hook_release = true;

    let r = GLOBAL_STATE
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

            let destroying = GLOBAL_STATE.d3d_resource_count > 0 &&
                hookdevice.ref_count == (GLOBAL_STATE.d3d_resource_count+1);
            if destroying {
                // purge my stuff
                write_log_file(&format!(
                    "device {:x} refcount is same as internal resource count ({}),
                    it is being destroyed: purging resources",
                    THIS as u64, GLOBAL_STATE.d3d_resource_count
                ));
                purge_device_resources(THIS as *mut IDirect3DDevice9);
                // Note, hookdevice.ref_count is wrong now since we bypassed
                // this function during unload (no re-entrancy).  however the count on the
                // device should be 1 if I did the math right, anyway the release below
                // will fix the count.
            }

            if destroying || (GLOBAL_STATE.d3d_resource_count == 0 && hookdevice.ref_count == 1) {
                // release again to trigger destruction of the device
                hookdevice.ref_count = (hookdevice.real_release)(THIS);
                write_log_file(&format!(
                    "device released: {:x}, refcount: {}",
                    THIS as u64, hookdevice.ref_count
                ));
                if hookdevice.ref_count != 0 {
                    write_log_file(&format!(
                        "WARNING: unexpected ref count of {} after supposedly final
                        device release, device probably leaked", hookdevice.ref_count));
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
    arg1: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT {
    let force_modding_off = false;

    profile_blocks!(hdip, hook_draw_indexed_primitive);

    profile_start!(hdip, hook_dip);

    // no re-entry please
    profile_start!(hdip, dip_check);
    if GLOBAL_STATE.in_dip {
        write_log_file(&format!("ERROR: i'm in DIP already!"));
        return S_OK;
    }
    profile_end!(hdip, dip_check);

    profile_start!(hdip, state_begin);

    let hookdevice = match GLOBAL_STATE.hook_direct3d9device {
        None => {
            write_log_file(&format!("DIP: No d3d9 device found"));
            return E_FAIL;
        }
        Some(ref mut hookdevice) => hookdevice,
    };
    profile_end!(hdip, state_begin);

    if hookdevice.low_framerate || !GLOBAL_STATE.show_mods || force_modding_off {
        return (hookdevice.real_draw_indexed_primitive)(
            THIS,
            arg1,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        );
    }

    // snapshotting
    let mut override_texture:*mut IDirect3DBaseTexture9 = null_mut();
    let mut sel_stage = 0;
    if GLOBAL_STATE.making_selection {
        get_selected_texture_stage_().map(|stage| {
            sel_stage = stage;
            override_texture = std::mem::transmute(GLOBAL_STATE.selection_texture);
        });
    }

    profile_start!(hdip, main_combinator);
    profile_start!(hdip, mod_key_prep);

    GLOBAL_STATE.in_dip = true;

    let mut drew_mod = false;

    // if there is a matching mod, render it
    let modded = GLOBAL_STATE
        .loaded_mods
        .as_ref()
        .and_then(|mods| {
            profile_end!(hdip, mod_key_prep);
            profile_start!(hdip, mod_key_lookup);
            let mod_key = NativeModData::mod_key(NumVertices, primCount);
            let r = mods.get(&mod_key);
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

            let mut save_texture:*mut IDirect3DBaseTexture9 = null_mut();
            if override_texture != null_mut() {
                (*THIS).GetTexture(sel_stage, &mut save_texture);
                (*THIS).SetTexture(sel_stage, override_texture);
            }
            (*THIS).DrawPrimitive(
                nmod.mod_data.numbers.prim_type as u32,
                0,
                nmod.mod_data.numbers.prim_count as u32,
            );
            drew_mod = true;

            // restore state
            (*THIS).SetVertexDeclaration(pDecl);
            (*THIS).SetStreamSource(0, pStreamVB, offsetBytes, stride);
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
        let mut save_texture:*mut IDirect3DBaseTexture9 = null_mut();
        if override_texture != null_mut() {
            (*THIS).GetTexture(sel_stage, &mut save_texture);
            (*THIS).SetTexture(sel_stage, override_texture);
        }
        let r = (hookdevice.real_draw_indexed_primitive)(
            THIS,
            arg1,
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
    hookdevice.dip_calls += 1;
    if hookdevice.dip_calls % 500_000 == 0 {
        let now = SystemTime::now();
        let elapsed = now.duration_since(hookdevice.last_call_log);
        match elapsed {
            Ok(d) => {
                let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                if secs >= 10.0 {
                    let dipsec = hookdevice.dip_calls as f64 / secs;

                    let epocht = now.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_secs(1))
                        .as_secs() * 1000;

                    write_log_file(&format!(
                        "{:?}: {} dip calls in {:.*} secs ({:.*} dips/sec (fps: {:.*}))",
                        epocht, hookdevice.dip_calls, 2, secs, 2, dipsec, 2, hookdevice.last_fps
                    ));
                    GLOBAL_STATE.active_texture_set.as_ref().map(|set| {
                        write_log_file(&format!("active texture set contains: {} textures", set.len()))
                    });
                    hookdevice.last_call_log = now;
                    hookdevice.dip_calls = 0;
                }
            }
            Err(e) => write_log_file(&format!("Error getting elapsed duration: {:?}", e)),
        }
    }
    profile_end!(hdip, statistics);

    GLOBAL_STATE.in_dip = false;
    profile_end!(hdip, hook_dip);

    profile_accum!(hdip);
    profile_summarize!(hdip);

    dresult
}

unsafe fn hook_device(
    device: *mut IDirect3DDevice9,
    _guard: &std::sync::MutexGuard<()>,
) -> Result<HookDirect3D9Device> {
    //write_log_file(&format!("gs hook_direct3d9device is some: {}", GLOBAL_STATE.hook_direct3d9device.is_some()));
    write_log_file(&format!("hooking new device: {:x}", device as u64));
    // Oddity: each device seems to have its own vtbl.  So need to hook each one of them.
    // but the direct3d9 instance seems to share a vtbl between different instances.  So need to only
    // hook those once.  I'm not sure why this is.
    let vtbl: *mut IDirect3DDevice9Vtbl = std::mem::transmute((*device).lpVtbl);
    write_log_file(&format!("device vtbl: {:x}", vtbl as u64));
    let vsize = std::mem::size_of::<IDirect3DDevice9Vtbl>();

    let real_draw_indexed_primitive = (*vtbl).DrawIndexedPrimitive;
    //let real_begin_scene = (*vtbl).BeginScene;
    let real_release = (*vtbl).parent.Release;
    let real_present = (*vtbl).Present;

    // remember these functions but don't hook them yet
    let real_set_texture = (*vtbl).SetTexture;

    let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

    (*vtbl).DrawIndexedPrimitive = hook_draw_indexed_primitive;
    //(*vtbl).BeginScene = hook_begin_scene;
    (*vtbl).Present = hook_present;
    (*vtbl).parent.Release = hook_release;

    protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

    // Inc ref count on the device
    (*device).AddRef();

    Ok(HookDirect3D9Device::new(
        real_draw_indexed_primitive,
        //real_begin_scene,
        real_present,
        real_release,
        real_set_texture
    ))
}

#[inline]
unsafe fn create_and_hook_device(
    THIS: *mut IDirect3D9,
    Adapter: UINT,
    DeviceType: D3DDEVTYPE,
    hFocusWindow: HWND,
    BehaviorFlags: DWORD,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
    ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
) -> Result<()> {
    let lock = GLOBAL_STATE_LOCK
        .lock()
        .map_err(|_err| HookError::GlobalLockError)?;

    GLOBAL_STATE
        .hook_direct3d9
        .as_mut()
        .ok_or(HookError::Direct3D9InstanceNotFound)
        .and_then(|hd3d9| {
            write_log_file(&format!("calling real create device"));
            if BehaviorFlags & D3DCREATE_MULTITHREADED == D3DCREATE_MULTITHREADED {
                write_log_file(&format!(
                    "Notice: device being created with D3DCREATE_MULTITHREADED"
                ));
            }
            let result = (hd3d9.real_create_device)(
                THIS,
                Adapter,
                DeviceType,
                hFocusWindow,
                BehaviorFlags,
                pPresentationParameters,
                ppReturnedDeviceInterface,
            );
            if result != S_OK {
                write_log_file(&format!("create device FAILED: {}", result));
                return Err(HookError::CreateDeviceFailed(result));
            }
            GLOBAL_STATE.d3d_window = hFocusWindow;
            hook_device(*ppReturnedDeviceInterface, &lock)
        })
        .and_then(|hook_d3d9device| {
            GLOBAL_STATE.hook_direct3d9device = Some(hook_d3d9device);
            write_log_file(&format!(
                "hooked device on thread {:?}",
                std::thread::current().id()
            ));
            Ok(())
        })
        .or_else(|err| {
            if ppReturnedDeviceInterface != null_mut() && *ppReturnedDeviceInterface != null_mut() {
                (*(*ppReturnedDeviceInterface)).Release();
            }
            Err(err)
        })
}

pub unsafe extern "system" fn hook_create_device(
    THIS: *mut IDirect3D9,
    Adapter: UINT,
    DeviceType: D3DDEVTYPE,
    hFocusWindow: HWND,
    BehaviorFlags: DWORD,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
    ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9,
) -> HRESULT {
    let res = create_and_hook_device(
        THIS,
        Adapter,
        DeviceType,
        hFocusWindow,
        BehaviorFlags,
        pPresentationParameters,
        ppReturnedDeviceInterface,
    );

    // create input, but don't fail everything if we can't (may be able to still use read-only mode)
    input::Input::new()
        .map(|inp| {
            GLOBAL_STATE.input = Some(inp);
        })
        .unwrap_or_else(|e| {
            write_log_file(&format!(
                "failed to create input; only playback from existing mods will be possible: {:?}",
                e
            ))
        });

    match res {
        Err(e) => {
            write_log_file(&format!("error creating/hooking device: {:?}", e));
            E_FAIL
        }
        Ok(_) => S_OK,
    }
}

type Direct3DCreate9Fn = unsafe extern "system" fn(sdk_ver: u32) -> *mut IDirect3D9;

#[allow(unused)]
#[no_mangle]
pub extern "system" fn Direct3DCreate9(SDKVersion: u32) -> *mut u64 {
    match create_d3d9(SDKVersion) {
        Ok(ptr) => ptr as *mut u64,
        Err(x) => {
            write_log_file(&format!("create_d3d failed: {:?}", x));
            std::ptr::null_mut()
        }
    }
}

pub fn create_d3d9(sdk_ver: u32) -> Result<*mut IDirect3D9> {
    let handle = util::load_lib("c:\\windows\\system32\\d3d9.dll")?; // Todo: use GetSystemDirectory
    let addr = util::get_proc_address(handle, "Direct3DCreate9")?;

    let make_it = || unsafe {
        let create: Direct3DCreate9Fn = std::mem::transmute(addr);

        let direct3d9 = (create)(sdk_ver);
        let direct3d9 = direct3d9 as *mut IDirect3D9;
        direct3d9
    };

    unsafe {
        let mm_root = match get_mm_conf_info() {
            Ok((true, Some(dir))) => dir,
            Ok((false, _)) => {
                write_log_file(&format!("ModelMod not initializing because it is not active (did you start it with the ModelMod launcher?)"));
                return Ok(make_it());
            }
            Ok((true, None)) => {
                write_log_file(&format!("ModelMod not initializing because install dir not found (did you start it with the ModelMod launcher?)"));
                return Ok(make_it());
            }
            Err(e) => {
                write_log_file(&format!(
                    "ModelMod not initializing due to conf error: {:?}",
                    e
                ));
                return Ok(make_it());
            }
        };

        // try to create log file using module name and root dir.  if it fails then just
        // let logging go to the temp dir file.
        get_module_name()
            .and_then(|mod_name| {
                use std::path::PathBuf;

                let stem = {
                    let mut pb = PathBuf::from(&mod_name);
                    let s = pb.file_stem()
                        .ok_or(HookError::ConfReadFailed("no stem".to_owned()))?;
                    let s = s.to_str()
                        .ok_or(HookError::ConfReadFailed("cant't make stem".to_owned()))?;
                    (*s).to_owned()
                };

                let file_name = format!("ModelMod.{}.log", stem);

                let mut tdir = mm_root.to_owned();
                tdir.push_str("\\Logs\\");
                let mut tname = tdir.to_owned();
                tname.push_str(&file_name);

                use std::io::Write;
                use std::fs::OpenOptions;
                // don't open append first time so that log is cleared.
                let mut f = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&tname)?;
                writeln!(f, "ModelMod initialized\r")?;

                // if that succeeded then we can set the file name now
                util::set_log_file_path(&tdir, &file_name)?;

                eprintln!("Log File: {}", tname);

                Ok(())
            })
            .map_err(|e| {
                write_log_file(&format!("error setting custom log file name: {:?}", e));
            })
            .unwrap_or(());

        let direct3d9 = make_it();
        write_log_file(&format!("created d3d: {:x}", direct3d9 as u64));

        // let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);
        // write_log_file(&format!("vtbl: {:x}", vtbl as u64));

        // don't hook more than once
        let _lock = GLOBAL_STATE_LOCK
            .lock()
            .map_err(|_err| HookError::D3D9HookFailed)?;

        if GLOBAL_STATE.hook_direct3d9.is_some() {
            return Ok(direct3d9);
        }

        GLOBAL_STATE.mm_root = Some(mm_root);

        // get pointer to original vtable
        let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);

        // save pointer to real function
        let real_create_device = (*vtbl).CreateDevice;
        // write_log_file(&format!(
        //     "hooking real create device, hookfn: {:?}, realfn: {:?} ",
        //     hook_create_device as u64, real_create_device as u64
        // ));

        // unprotect memory and slam the vtable
        let vsize = std::mem::size_of::<IDirect3D9Vtbl>();
        let old_prot = util::unprotect_memory(vtbl as *mut c_void, vsize)?;

        (*vtbl).CreateDevice = hook_create_device;

        util::protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

        // create hookstate
        let hd3d9 = HookDirect3D9 {
            real_create_device: real_create_device,
        };

        GLOBAL_STATE.hook_direct3d9 = Some(hd3d9);

        Ok(direct3d9)
    }
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
        use test_e2e;
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
    fn dip_call_time(b: &mut Bencher) {
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
