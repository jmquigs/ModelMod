use winapi::um::unknwnbase::IUnknown;

pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::um::winnt::HRESULT;
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::ctypes::c_void;
use winapi::um::wingdi::RGNDATA;

use fnv::FnvHashMap;

use util::*;
use util;
use dnclr::init_clr;
use interop::InteropState;
use interop::NativeModData;
use interop;

use std;
use std::fmt;
use std::cell::RefCell;
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

pub struct HookDirect3D9 {
    pub real_create_device: CreateDeviceFn,
}

#[derive(Copy, Clone)]
pub struct HookDirect3D9Device {
    pub real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
    pub real_begin_scene: BeginSceneFn,
    pub real_present: PresentFn,
    pub real_release: IUnknownReleaseFn,
    pub ref_count: ULONG,
    pub dip_calls: u32,
    pub frames: u32,
    pub last_call_log: SystemTime,
    pub last_frame_log: SystemTime,
}

impl HookDirect3D9Device {
    pub fn new(
        real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
        real_begin_scene: BeginSceneFn,
        real_present: PresentFn,
        real_release: IUnknownReleaseFn,
    ) -> HookDirect3D9Device {
        HookDirect3D9Device {
            real_draw_indexed_primitive: real_draw_indexed_primitive,
            real_begin_scene: real_begin_scene,
            real_release: real_release,
            real_present: real_present,
            dip_calls: 0,
            frames: 0,
            ref_count: 0,
            last_call_log: SystemTime::now(),
            last_frame_log: SystemTime::now(),
        }
    }
}

// TODO: maybe don't need TLS variant
pub struct HookState {
    pub hook_direct3d9: Option<HookDirect3D9>,
    pub hook_direct3d9device: Option<HookDirect3D9Device>,
    pub clr_pointer: Option<u64>,
    pub interop_state: Option<InteropState>,
    pub is_global: bool,
    pub loaded_mods: Option<FnvHashMap<i32, interop::NativeModData>>,
}

impl HookState {
    pub fn new() -> HookState {
        let tid = std::thread::current().id();
        write_log_file(&format!("local hookstate created on thread {:?}", tid));
        HookState {
            hook_direct3d9: None,
            hook_direct3d9device: None,
            clr_pointer: None,
            interop_state: None,
            is_global: false,
            loaded_mods: None,
        }
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

const fn new_global_hookstate() -> HookState {
    HookState {
        hook_direct3d9: None,
        hook_direct3d9device: None,
        clr_pointer: None,
        interop_state: None,
        is_global: true,
        loaded_mods: None,
    }
}

// global state is copied into TLS as needed.  Prefer TLS to avoid locking on
// global state.
lazy_static! {
    pub static ref GLOBAL_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
pub static mut GLOBAL_STATE: HookState = new_global_hookstate();

thread_local! {
    static STATE: RefCell<HookState> = RefCell::new(HookState::new());
}

enum AsyncLoadState {
    NotStarted = 51,
    Pending,
    InProgress,
    Complete,
}

pub fn get_global_state_ptr() -> *mut HookState {
    let pstate: *mut HookState = unsafe { &mut GLOBAL_STATE };
    pstate
}

#[inline]
fn copy_state_to_tls() -> Result<()> {
    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();

        if state.hook_direct3d9device.is_none() {
            GLOBAL_STATE_LOCK
                .lock()
                .map(|_ignored| {
                    let hookstate = unsafe { &mut GLOBAL_STATE };
                    match (*hookstate).hook_direct3d9device {
                        Some(ref mut hookdevice) => {
                            write_log_file(&format!(
                                "writing global device state into TLS on thread {:?}",
                                std::thread::current().id()
                            ));

                            (*state).hook_direct3d9device = Some(*hookdevice);
                        }
                        None => write_log_file(&format!("no hook device in global state")),
                    };
                })
                .map_err(|_err| HookError::GlobalStateCopyFailed)?;
        }

        if state.interop_state.is_none() {
            GLOBAL_STATE_LOCK
                .lock()
                .as_mut()
                .map(|_ignored| {
                    let hookstate = unsafe { &mut GLOBAL_STATE };
                    match (*hookstate).interop_state {
                        Some(ref mut interop_state) => {
                            write_log_file(&format!(
                                "writing global interop state into TLS on thread {:?}",
                                std::thread::current().id()
                            ));

                            (*state).interop_state = Some(*interop_state);
                        }
                        None => (),
                    };
                })
                .map_err(|_err| HookError::GlobalStateCopyFailed)?;
        }

        Ok(())
    })
}

unsafe fn clear_loaded_mods() {
    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to clear mod data");
        return;
    }

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
    write_log_file(&format!("unloaded {} mods", cnt));
}

unsafe fn setup_mod_data(device: *mut IDirect3DDevice9, callbacks: interop::ManagedCallbacks) {
    clear_loaded_mods();

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

    let mut loaded_mods: FnvHashMap<i32, interop::NativeModData> =
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
            let hash_code = NativeModData::hash_code(
                native_mod_data.mod_data.numbers.ref_vert_count,
                native_mod_data.mod_data.numbers.ref_prim_count,
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

        // TODO:
        // this->add(nModData.vb);

        let vb = *out_vb;

        let hr = (*vb).Lock(0, 0, std::mem::transmute(&mut vb_data), 0);
        if hr != 0 {
            write_log_file(&format!("failed to lock vertex buffer: {:x}", hr));
            return;
        }

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
            return;
        }

        if ret != 0 {
            write_log_file(&format!("failed to fill mod data: {}", ret));
            return;
        }

        native_mod_data.vb = vb;

        let hash_code = NativeModData::hash_code(
            native_mod_data.mod_data.numbers.ref_vert_count,
            native_mod_data.mod_data.numbers.ref_prim_count,
        );
        loaded_mods.insert(hash_code, native_mod_data);

        println!("allocated vb for mod data {}: {:?}", midx, (*mdat).numbers);
    }

    GLOBAL_STATE.loaded_mods = Some(loaded_mods);
}

pub fn do_per_scene_operations(device: *mut IDirect3DDevice9) -> Result<()> {
    copy_state_to_tls()?;

    // init the clr if needed
    {
        let hookstate = unsafe { &mut GLOBAL_STATE };
        if hookstate.clr_pointer.is_none() {
            let lock = GLOBAL_STATE_LOCK.lock();
            match lock {
                Ok(_ignored) => {
                    if hookstate.clr_pointer.is_none() {
                        write_log_file("creating clr");
                        if let Ok(_p) = init_clr() {
                            hookstate.clr_pointer = Some(1);
                        } else {
                            hookstate.clr_pointer = Some(666);
                        }
                    }
                }
                Err(e) => write_log_file(&format!("{:?} should never happen", e)),
            };
        }
    }
    // write_log_file(&format!("performing per-scene ops on thread {:?}",
    //         std::thread::current().id()));

    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();

        state.interop_state.as_mut().map(|is| {
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
    })
}

pub unsafe extern "system" fn hook_present(
    THIS: *mut IDirect3DDevice9,
    pSourceRect: *const RECT,
    pDestRect: *const RECT,
    hDestWindowOverride: HWND,
    pDirtyRegion: *const RGNDATA,
) -> HRESULT {
    // if let Err(e) = copy_state_to_tls() {
    //     write_log_file(&format!("unexpected error: {:?}", e));
    //     return E_FAIL;
    // }

    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();

        state
            .hook_direct3d9device
            .as_mut()
            .map_or(S_OK, |hookdevice| {
                hookdevice.frames += 1;
                if hookdevice.frames % 30 == 0 {
                    let now = SystemTime::now();
                    let elapsed = now.duration_since(hookdevice.last_frame_log);
                    match elapsed {
                        Ok(d) => {
                            let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
                            if secs >= 10.0 {
                                let fps = hookdevice.frames as f64 / secs;

                                let epocht = now.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or(std::time::Duration::from_secs(1))
                                    .as_secs() * 1000;

                                write_log_file(&format!(
                                    "{:?}: {} frames in {} secs ({} fps)",
                                    epocht, hookdevice.frames, secs, fps
                                ));
                                hookdevice.last_frame_log = now;
                                hookdevice.frames = 0;
                            }
                        }
                        Err(e) => {
                            write_log_file(&format!("Error getting elapsed duration: {:?}", e))
                        }
                    }
                }
                (hookdevice.real_present)(
                    THIS,
                    pSourceRect,
                    pDestRect,
                    hDestWindowOverride,
                    pDirtyRegion,
                )
            })
    })
}

pub unsafe extern "system" fn hook_release(THIS: *mut IUnknown) -> ULONG {
    if let Err(e) = copy_state_to_tls() {
        write_log_file(&format!("unexpected error: {:?}", e));
        return 0xFFFFFFFF; // TODO: check docs, may be wrong "error" value
    }
    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();

        state
            .hook_direct3d9device
            .as_mut()
            .map_or(0xFFFFFFFF, |hookdevice| {
                hookdevice.ref_count = (hookdevice.real_release)(THIS);

                if hookdevice.ref_count == 1 {
                    // I am the last reference, unload any device-dependant state
                    // TODO: this doesn't work.  the d3d objects in mods will prevent the ref count from going to zero.
                    // will need to keep a count of the number of objects I have created that are dependant on the device,
                    // and release them all (and the device) when the ref count equals that count.
                    clear_loaded_mods();

                    // release again to trigger destruction of the device
                    hookdevice.ref_count = (hookdevice.real_release)(THIS);
                    write_log_file(&format!(
                        "device released: {:x}, refcount: {}",
                        THIS as u64, hookdevice.ref_count
                    ));
                    //write_log_file(&format!("device may be destroyed: {}", THIS as u64));
                }
                //else { write_log_file(&format!("curr device ref count: {}", hookdevice.ref_count )) }
                hookdevice.ref_count
            })
    })
}

pub unsafe extern "system" fn hook_begin_scene(THIS: *mut IDirect3DDevice9) -> HRESULT {
    if let Err(e) = do_per_scene_operations(THIS) {
        write_log_file(&format!("unexpected error: {:?}", e));
        return E_FAIL;
    }

    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();
        state
            .hook_direct3d9device
            .as_ref()
            .map_or(E_FAIL, |hookdevice| (hookdevice.real_begin_scene)(THIS))
    })
}

pub unsafe extern "system" fn hook_draw_indexed_primitive(
    THIS: *mut IDirect3DDevice9,
    arg1: D3DPRIMITIVETYPE,
    BaseVertexIndex: INT,
    MinVertexIndex: UINT,
    NumVertices: UINT,
    startIndex: UINT,
    primCount: UINT,
) -> HRESULT {
    STATE.with(|state| {
        let ref mut state = *state.borrow_mut();

        let hookdevice = match state.hook_direct3d9device {
            None => {
                write_log_file(&format!("No state in DIP"));
                return E_FAIL;
            } // beginscene must do global->tls copy
            Some(ref mut hookdevice) => hookdevice,
        };
        hookdevice.dip_calls += 1;
        if hookdevice.dip_calls % 200_000 == 0 {
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
                            "{:?}: {} dip calls in {} secs ({} dips/sec)",
                            epocht, hookdevice.dip_calls, secs, dipsec
                        ));
                        hookdevice.last_call_log = now;
                        hookdevice.dip_calls = 0;
                    }
                }
                Err(e) => write_log_file(&format!("Error getting elapsed duration: {:?}", e)),
            }
        }

        (hookdevice.real_draw_indexed_primitive)(
            THIS,
            arg1,
            BaseVertexIndex,
            MinVertexIndex,
            NumVertices,
            startIndex,
            primCount,
        )
    })
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
    let real_begin_scene = (*vtbl).BeginScene;
    let real_release = (*vtbl).parent.Release;
    let real_present = (*vtbl).Present;

    let old_prot = unprotect_memory(vtbl as *mut c_void, vsize)?;

    (*vtbl).DrawIndexedPrimitive = hook_draw_indexed_primitive;
    (*vtbl).BeginScene = hook_begin_scene;
    (*vtbl).Present = hook_present;
    (*vtbl).parent.Release = hook_release;

    protect_memory(vtbl as *mut c_void, vsize, old_prot)?;

    // Inc ref count on the device
    (*device).AddRef();

    Ok(HookDirect3D9Device::new(
        real_draw_indexed_primitive,
        real_begin_scene,
        real_present,
        real_release,
    ))
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
    let lock = GLOBAL_STATE_LOCK.lock();
    match lock {
        Err(e) => {
            write_log_file(&format!("global lock error: {}", e));
            E_FAIL
        }
        Ok(guard) => {
            write_log_file(&format!("hook_create_device called"));
            match GLOBAL_STATE.hook_direct3d9 {
                None => {
                    write_log_file(&format!("no hook_direct3d9"));
                    E_FAIL
                }
                Some(ref hd3d9) => {
                    write_log_file(&format!("calling real create device"));
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
                        return result;
                    }
                    match hook_device(*ppReturnedDeviceInterface, &guard) {
                        Err(e) => {
                            write_log_file(&format!("error hooking device: {:?}", e));
                            // return device anyway, since failing just because the hook failed is very rude.
                            S_OK
                        }
                        Ok(hook_d3d9device) => {
                            GLOBAL_STATE.hook_direct3d9device = Some(hook_d3d9device);
                            write_log_file(&format!(
                                "hooked device on thread {:?}",
                                std::thread::current().id()
                            ));
                            S_OK
                        }
                    }
                }
            }
        }
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

    unsafe {
        let create: Direct3DCreate9Fn = std::mem::transmute(addr);

        let direct3d9 = (create)(sdk_ver);
        let direct3d9 = direct3d9 as *mut IDirect3D9;
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

        // get pointer to original vtable
        let vtbl: *mut IDirect3D9Vtbl = std::mem::transmute((*direct3d9).lpVtbl);

        // save pointer to real function
        let real_create_device = (*vtbl).CreateDevice;
        println!(
            "============= hooking rcd, hookfn: {:?}, realfn: {:?} ",
            hook_create_device as u64, real_create_device as u64
        );

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
