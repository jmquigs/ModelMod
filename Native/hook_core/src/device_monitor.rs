/// Diagnostic module for debugging D3D9 hook failures on level load.
/// This is throwaway diagnostic code — not intended for long-term use.
///
/// Implements:
/// - Reset hook with parameter logging
/// - Enhanced Release logging
/// - Direct3DCreate9Ex detection
/// - Background device monitor thread (polls TestCooperativeLevel, refcount,
///   swap chain params, and queries for IDirect3DDevice9Ex)

use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use winapi::shared::d3d9::*;
use winapi::shared::d3d9types::*;
use winapi::shared::winerror::S_OK;
use winapi::um::winnt::HRESULT;
use winapi::um::unknwnbase::IUnknown;

use shared_dx::util::write_log_file;

// ── Globals for the monitor ────────────────────────────────────────────

/// Raw device pointer stashed at hook time so the monitor thread can poll it.
static MONITOR_DEVICE: AtomicUsize = AtomicUsize::new(0);
/// Flag to request the monitor thread to stop.
static MONITOR_STOP: AtomicBool = AtomicBool::new(false);
/// Track whether we've already spawned a monitor thread.
static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Track whether Direct3DCreate9Ex was ever called.
static CREATE9EX_CALLED: AtomicBool = AtomicBool::new(false);

/// Counter for total Reset calls observed.
static RESET_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Counter for total Release calls observed (sampled — only logged periodically).
static RELEASE_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

// ── Expected hook function addresses (set at hook time) ──────────────

/// Address of our hook_draw_indexed_primitive function, set when device is hooked.
static EXPECTED_DIP_HOOK: AtomicUsize = AtomicUsize::new(0);
/// Address of our hook_present function, set when device is hooked.
static EXPECTED_PRESENT_HOOK: AtomicUsize = AtomicUsize::new(0);
/// Address of our hook_reset function, set when device is hooked.
static EXPECTED_RESET_HOOK: AtomicUsize = AtomicUsize::new(0);
/// Address of our hook_release function, set when device is hooked.
static EXPECTED_RELEASE_HOOK: AtomicUsize = AtomicUsize::new(0);

/// Called from hook_d3d9_device after vtable patching to record the expected
/// hook function addresses so the monitor thread can verify them.
pub fn set_expected_hook_addrs(
    dip: usize,
    present: usize,
    reset: usize,
    release: usize,
) {
    EXPECTED_DIP_HOOK.store(dip, Ordering::Relaxed);
    EXPECTED_PRESENT_HOOK.store(present, Ordering::Relaxed);
    EXPECTED_RESET_HOOK.store(reset, Ordering::Relaxed);
    EXPECTED_RELEASE_HOOK.store(release, Ordering::Relaxed);
    write_log_file(&format!(
        "[DIAG] Stored expected hook addrs: DIP={:x}, Present={:x}, Reset={:x}, Release={:x}",
        dip, present, reset, release
    ));
}

// ── D3D9 error codes we care about ────────────────────────────────────

const D3DERR_DEVICELOST: HRESULT = -2005530520i32; // 0x88760868
const D3DERR_DEVICENOTRESET: HRESULT = -2005530519i32; // 0x88760869
const D3DERR_DRIVERINTERNALERROR: HRESULT = -2005530585i32; // 0x88760827

// IID for IDirect3DDevice9Ex: {B18B10CE-2649-405a-870F-95F777D4313A}
const IID_IDIRECT3DDEVICE9EX: winapi::shared::guiddef::GUID = winapi::shared::guiddef::GUID {
    Data1: 0xB18B10CE,
    Data2: 0x2649,
    Data3: 0x405a,
    Data4: [0x87, 0x0F, 0x95, 0xF7, 0x77, 0xD4, 0x31, 0x3A],
};

// ── Helper: describe an HRESULT from TestCooperativeLevel ──────────────

fn tcl_status_str(hr: HRESULT) -> &'static str {
    match hr {
        S_OK => "D3D_OK",
        D3DERR_DEVICELOST => "D3DERR_DEVICELOST",
        D3DERR_DEVICENOTRESET => "D3DERR_DEVICENOTRESET",
        D3DERR_DRIVERINTERNALERROR => "D3DERR_DRIVERINTERNALERROR",
        _ => "UNKNOWN",
    }
}

// ── Reset Hook ────────────────────────────────────────────────────────

pub unsafe extern "system" fn hook_reset(
    THIS: *mut IDirect3DDevice9,
    pPresentationParameters: *mut D3DPRESENT_PARAMETERS,
) -> HRESULT {
    let count = RESET_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    if !pPresentationParameters.is_null() {
        let pp = &*pPresentationParameters;
        write_log_file(&format!(
            "[DIAG] Reset #{} called on device {:x} — \
             BackBufferWidth={}, BackBufferHeight={}, BackBufferFormat={}, \
             BackBufferCount={}, Windowed={}, SwapEffect={}, \
             PresentationInterval={}, hDeviceWindow={:x}",
            count,
            THIS as usize,
            pp.BackBufferWidth,
            pp.BackBufferHeight,
            pp.BackBufferFormat,
            pp.BackBufferCount,
            pp.Windowed,
            pp.SwapEffect,
            pp.PresentationInterval,
            pp.hDeviceWindow as usize,
        ));
    } else {
        write_log_file(&format!(
            "[DIAG] Reset #{} called on device {:x} — pPresentationParameters is NULL",
            count, THIS as usize
        ));
    }

    // Call the real Reset
    use device_state::dev_state;
    use shared_dx::types::{HookDeviceState, HookD3D9State};
    let result = match dev_state().hook {
        Some(HookDeviceState::D3D9(HookD3D9State { d3d9: _, device: Some(ref dev) })) => {
            match dev.real_reset {
                Some(real_fn) => (real_fn)(THIS, pPresentationParameters),
                None => {
                    write_log_file("[DIAG] Reset hook: no real_reset stored!");
                    -1 // E_FAIL-ish
                }
            }
        },
        _ => {
            write_log_file("[DIAG] Reset hook: no device state!");
            -1
        }
    };

    write_log_file(&format!(
        "[DIAG] Reset #{} returned 0x{:08X} ({})",
        count,
        result as u32,
        if result == S_OK { "OK" } else { "FAILED" }
    ));

    result
}

// ── Enhanced Release logging ──────────────────────────────────────────

/// Call this from the existing hook_release to add diagnostic logging.
/// Returns the ref count string for the log (or does its own logging).
pub unsafe fn diag_log_release(this: *mut IUnknown, ref_count_after: u32) {
    let count = RELEASE_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    // Log every release when refcount is low (interesting), or periodically
    if ref_count_after < 10 || count % 500 == 0 {
        write_log_file(&format!(
            "[DIAG] Release on {:x}: refcount now {} (total release calls: {})",
            this as usize, ref_count_after, count
        ));
    }
}

// ── CreateDevice logging ──────────────────────────────────────────────

pub fn diag_log_create_device(device_ptr: usize, adapter: u32, device_type: u32, behavior_flags: u32) {
    write_log_file(&format!(
        "[DIAG] CreateDevice: device={:x}, adapter={}, deviceType={}, behaviorFlags=0x{:08X}, \
         multithreaded={}, thread={:?}",
        device_ptr,
        adapter,
        device_type,
        behavior_flags,
        (behavior_flags & 0x00000004) != 0, // D3DCREATE_MULTITHREADED
        std::thread::current().id(),
    ));
}

// ── Direct3DCreate9Ex hook ────────────────────────────────────────────

type Direct3DCreate9ExFn = unsafe extern "system" fn(
    sdk_version: u32,
    pp_d3d9ex: *mut *mut IDirect3D9,
) -> HRESULT;

/// Exported as a proxy DLL entry point. If the game calls Direct3DCreate9Ex
/// we intercept it here to log the fact, then forward to the real function.
#[no_mangle]
pub extern "system" fn Direct3DCreate9Ex(SDKVersion: u32, ppD3D: *mut *mut IDirect3D9) -> HRESULT {
    CREATE9EX_CALLED.store(true, Ordering::Relaxed);
    write_log_file(&format!(
        "[DIAG] *** Direct3DCreate9Ex called! SDKVersion={}, thread={:?} ***",
        SDKVersion,
        std::thread::current().id(),
    ));

    unsafe {
        // Load real d3d9.dll and get the real Direct3DCreate9Ex
        match crate::hook_device::load_d3d_lib("d3d9.dll") {
            Ok(handle) => {
                match util::get_proc_address(handle, "Direct3DCreate9Ex") {
                    Ok(addr) => {
                        let real_fn: Direct3DCreate9ExFn = std::mem::transmute(addr);
                        let hr = (real_fn)(SDKVersion, ppD3D);
                        write_log_file(&format!(
                            "[DIAG] Real Direct3DCreate9Ex returned 0x{:08X}",
                            hr as u32
                        ));
                        hr
                    }
                    Err(e) => {
                        write_log_file(&format!(
                            "[DIAG] Failed to get real Direct3DCreate9Ex: {:?}", e
                        ));
                        -1 // E_FAIL
                    }
                }
            }
            Err(e) => {
                write_log_file(&format!(
                    "[DIAG] Failed to load d3d9.dll for Ex path: {:?}", e
                ));
                -1
            }
        }
    }
}

// ── Device Monitor Thread ─────────────────────────────────────────────

/// Start the monitor thread for the given device pointer.
/// Safe to call multiple times — only one thread will run at a time.
pub fn start_device_monitor(device: *mut IDirect3DDevice9) {
    if device.is_null() {
        return;
    }

    // Stop any existing monitor
    MONITOR_STOP.store(true, Ordering::Relaxed);
    // Brief sleep to let old thread see the flag (best-effort)
    thread::sleep(Duration::from_millis(100));

    MONITOR_STOP.store(false, Ordering::Relaxed);
    MONITOR_DEVICE.store(device as usize, Ordering::Relaxed);

    if MONITOR_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        // AddRef to prevent the device from being destroyed while we hold a reference
        unsafe { (*device).AddRef(); }
        write_log_file(&format!(
            "[DIAG] Starting device monitor thread for device {:x}",
            device as usize
        ));

        match thread::Builder::new()
            .name("mm-device-monitor".into())
            .spawn(move || {
                device_monitor_loop();
            }) {
            Ok(_) => {},
            Err(e) => {
                write_log_file(&format!("[DIAG] Failed to spawn monitor thread: {:?}", e));
                MONITOR_RUNNING.store(false, Ordering::Relaxed);
            }
        }
    } else {
        // Already running — just update the device pointer (the loop will pick it up)
        write_log_file(&format!(
            "[DIAG] Monitor already running, updated device pointer to {:x}",
            device as usize
        ));
    }
}

/// Stop the monitor thread (call on shutdown / device destroy).
pub fn stop_device_monitor() {
    MONITOR_STOP.store(true, Ordering::Relaxed);
}

fn device_monitor_loop() {
    write_log_file("[DIAG] Monitor thread started");

    let mut iteration: u64 = 0;
    let mut last_tcl: HRESULT = S_OK;
    let mut last_refcount: u32 = 0;
    let mut last_width: u32 = 0;
    let mut last_height: u32 = 0;
    let mut ex_checked = false;

    loop {
        if MONITOR_STOP.load(Ordering::Relaxed) {
            break;
        }

        let dev_addr = MONITOR_DEVICE.load(Ordering::Relaxed);
        if dev_addr == 0 {
            thread::sleep(Duration::from_millis(1000));
            continue;
        }

        let device = dev_addr as *mut IDirect3DDevice9;
        iteration += 1;

        unsafe {
            // 1) TestCooperativeLevel
            let tcl = (*device).TestCooperativeLevel();
            if tcl != last_tcl || iteration % 20 == 1 {
                write_log_file(&format!(
                    "[DIAG] Monitor #{}: TestCooperativeLevel = 0x{:08X} ({})",
                    iteration, tcl as u32, tcl_status_str(tcl)
                ));
                last_tcl = tcl;
            }

            // 2) Reference count (AddRef/Release pair)
            let rc = {
                (*device).AddRef();
                (*device).Release()
            };
            // Subtract 1 for our monitor's own AddRef
            let effective_rc = if rc > 0 { rc - 1 } else { 0 };
            if effective_rc != last_refcount || iteration % 20 == 1 {
                write_log_file(&format!(
                    "[DIAG] Monitor #{}: Device refcount = {} (raw={}, minus our monitor ref)",
                    iteration, effective_rc, rc
                ));
                last_refcount = effective_rc;
            }

            // 3) GetSwapChain(0) -> GetPresentParameters to detect resolution changes
            let mut swap_chain: *mut IDirect3DSwapChain9 = null_mut();
            let hr = (*device).GetSwapChain(0, &mut swap_chain);
            if hr == S_OK && !swap_chain.is_null() {
                let mut pp: D3DPRESENT_PARAMETERS = std::mem::zeroed();
                let hr2 = (*swap_chain).GetPresentParameters(&mut pp);
                if hr2 == S_OK {
                    if pp.BackBufferWidth != last_width || pp.BackBufferHeight != last_height || iteration % 20 == 1 {
                        write_log_file(&format!(
                            "[DIAG] Monitor #{}: SwapChain PresentParams: {}x{}, fmt={}, windowed={}, swapEffect={}",
                            iteration, pp.BackBufferWidth, pp.BackBufferHeight,
                            pp.BackBufferFormat, pp.Windowed, pp.SwapEffect
                        ));
                        last_width = pp.BackBufferWidth;
                        last_height = pp.BackBufferHeight;
                    }
                } else {
                    write_log_file(&format!(
                        "[DIAG] Monitor #{}: GetPresentParameters failed: 0x{:08X}",
                        iteration, hr2 as u32
                    ));
                }
                (*swap_chain).Release();
            } else if iteration % 20 == 1 {
                write_log_file(&format!(
                    "[DIAG] Monitor #{}: GetSwapChain(0) failed: 0x{:08X}",
                    iteration, hr as u32
                ));
            }

            // 4) QueryInterface for IDirect3DDevice9Ex (only check occasionally)
            if !ex_checked || iteration % 60 == 1 {
                let mut ex_ptr: *mut IUnknown = null_mut();
                let hr_qi = (*(device as *mut IUnknown)).QueryInterface(
                    &IID_IDIRECT3DDEVICE9EX,
                    &mut ex_ptr as *mut *mut IUnknown as *mut *mut winapi::ctypes::c_void,
                );
                if hr_qi == S_OK && !ex_ptr.is_null() {
                    write_log_file(&format!(
                        "[DIAG] Monitor #{}: *** Device supports IDirect3DDevice9Ex! ptr={:x} ***",
                        iteration, ex_ptr as usize
                    ));
                    (*ex_ptr).Release();
                } else if !ex_checked {
                    write_log_file(&format!(
                        "[DIAG] Monitor #{}: Device does NOT support IDirect3DDevice9Ex (hr=0x{:08X})",
                        iteration, hr_qi as u32
                    ));
                }
                ex_checked = true;
            }

            // 5) Log whether Direct3DCreate9Ex was ever called
            if iteration == 1 || (iteration % 60 == 0 && CREATE9EX_CALLED.load(Ordering::Relaxed)) {
                write_log_file(&format!(
                    "[DIAG] Monitor #{}: Direct3DCreate9Ex ever called: {}",
                    iteration, CREATE9EX_CALLED.load(Ordering::Relaxed)
                ));
            }

            // 6) Check vtable — are our hook functions still installed?
            {
                let vtbl: *const IDirect3DDevice9Vtbl = (*device).lpVtbl;
                if !vtbl.is_null() {
                    let current_dip = (*vtbl).DrawIndexedPrimitive as usize;
                    let current_present = (*vtbl).Present as usize;
                    let current_reset = (*vtbl).Reset as usize;
                    let current_release = (*vtbl).parent.Release as usize;

                    let expected_dip = EXPECTED_DIP_HOOK.load(Ordering::Relaxed);
                    let expected_present = EXPECTED_PRESENT_HOOK.load(Ordering::Relaxed);
                    let expected_reset = EXPECTED_RESET_HOOK.load(Ordering::Relaxed);
                    let expected_release = EXPECTED_RELEASE_HOOK.load(Ordering::Relaxed);

                    let dip_ok = expected_dip == 0 || current_dip == expected_dip;
                    let present_ok = expected_present == 0 || current_present == expected_present;
                    let reset_ok = expected_reset == 0 || current_reset == expected_reset;
                    let release_ok = expected_release == 0 || current_release == expected_release;

                    if !dip_ok || !present_ok || !reset_ok || !release_ok {
                        write_log_file(&format!(
                            "[DIAG] Monitor #{}: *** VTABLE HOOKS MODIFIED! ***", iteration
                        ));
                        if !dip_ok {
                            write_log_file(&format!(
                                "[DIAG]   DrawIndexedPrimitive: expected {:x}, got {:x} — UNHOOKED!",
                                expected_dip, current_dip
                            ));
                        }
                        if !present_ok {
                            write_log_file(&format!(
                                "[DIAG]   Present: expected {:x}, got {:x} — UNHOOKED!",
                                expected_present, current_present
                            ));
                        }
                        if !reset_ok {
                            write_log_file(&format!(
                                "[DIAG]   Reset: expected {:x}, got {:x} — UNHOOKED!",
                                expected_reset, current_reset
                            ));
                        }
                        if !release_ok {
                            write_log_file(&format!(
                                "[DIAG]   Release: expected {:x}, got {:x} — UNHOOKED!",
                                expected_release, current_release
                            ));
                        }
                    } else if iteration % 20 == 1 {
                        write_log_file(&format!(
                            "[DIAG] Monitor #{}: Vtable hooks intact (DIP={:x}, Present={:x}, Reset={:x}, Release={:x})",
                            iteration, current_dip, current_present, current_reset, current_release
                        ));
                    }
                }
            }

            // Warn if refcount drops to just our ref (game abandoned device)
            // Our monitor holds 1 ref, the hook_d3d9_device AddRef holds another = 2
            if effective_rc <= 1 && effective_rc != 0 {
                write_log_file(&format!(
                    "[DIAG] Monitor #{}: WARNING — refcount is {} — game may have abandoned this device!",
                    iteration, effective_rc
                ));
            }
        }

        // Poll interval: 500ms as recommended (diagnostic only, minimize racing)
        thread::sleep(Duration::from_millis(500));
    }

    // Release our monitor's reference on exit
    let dev_addr = MONITOR_DEVICE.load(Ordering::Relaxed);
    if dev_addr != 0 {
        unsafe {
            let device = dev_addr as *mut IDirect3DDevice9;
            let rc = (*device).Release();
            write_log_file(&format!(
                "[DIAG] Monitor thread releasing device ref, refcount now: {}",
                rc
            ));
        }
    }

    MONITOR_RUNNING.store(false, Ordering::Relaxed);
    write_log_file("[DIAG] Monitor thread exiting");
}
