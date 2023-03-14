use std::{time::SystemTime, cell::{RefCell, RefMut}, fmt::Debug};

use shared_dx::util::write_log_file;

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub enum DebugModeCalledFns {
    Hook_DeviceCreateInputLayoutFn,
    Hook_ContextRelease,
    Hook_ContextVSSetConstantBuffers,
    Hook_ContextDrawIndexed,
    Hook_ContextIASetVertexBuffers,
    Hook_ContextIASetInputLayout,
    Hook_ContextIASetPrimitiveTopology,
    Last = 99
}
/// DebugMode is a special mode that is enabled by creating a file in the MMRoot called `DebugMode.txt`.
/// See `check_init` for details.  When in this mode, MM will slow its init process and perform extra
/// logging about what is going on.  Its intended to be used to help track down crashes on end
/// user machines.  This structure holds all the state for debug mode and is stored in a thread
/// local to minimize the chance that it can cause a crash on its own.  Running with all settings
/// enabled (the default) does incur a performance penalty.  But as of this writing (3/14/2023)
/// the perf penalty when this mode is disabled is small, even though this code is always compiled in.
pub struct DebugMode {
    enabled: bool,
    /// True if MM should unprotect memory before hooking.
    protect_mem: bool,
    /// True if MM should defer rehooking until some time has passed, enabled progressively for
    /// each of the available functions.
    defer_rehook: bool,
    /// True if MM should defer draw hooking until some time has passed.  Deferring this also
    /// defers CLR init, since that is triggered from draw.
    defer_draw_hook: bool,

    draw_hooked: bool,
    call_counts: [u64; 100],
    start_time: Option<SystemTime>,
    rehook_enabled: [bool; 100],
}

thread_local! {
    pub static DEBUG_MODE: RefCell<DebugMode> = RefCell::new(DebugMode {
        enabled: false,
        protect_mem: true,
        defer_rehook: true,
        defer_draw_hook: true,

        call_counts: [0; 100],
        start_time: None,
        draw_hooked: false,
        rehook_enabled: [false; 100],
    });
}

/// Return seconds since debug mode was initialized.
pub fn seconds_since_start(dm:&RefMut<DebugMode>) -> u64 {
    if let Some(start_time) = dm.start_time {
        let since_start = SystemTime::now().duration_since(start_time).unwrap_or_else(|_| std::time::Duration::from_millis(0));
        since_start.as_secs() as u64
    } else {
        0
    }
}

/// Check if debug mode is enabled and Init it if so.  This check is done by looking for
/// `DebugMode.txt` in the mm root.  If it exists, debug mode is enabled.  The file can additionally
/// contain key=value pairs to set debug mode options.
/// It is expected that MMLaunch will create this file when the user starts from that app.
pub fn check_init(mmroot:&str) {
    let mut dmfile = mmroot.to_string();
    dmfile.push_str("\\DebugMode.txt");
    let path = std::path::Path::new(&dmfile);
    if path.exists() {
        DEBUG_MODE.with(|dm| {
            let mut dm = dm.borrow_mut();
            dm.enabled = true;
            dm.start_time = Some(SystemTime::now());
            write_log_file(&format!("DebugMode: enabled, init will be slower. remove/rename {} to disable it", dmfile));
            match std::fs::read_to_string(path) {
                Ok(lines) => {
                    for line in lines.lines() {
                        if line.trim().starts_with("#") {
                            continue;
                        }
                        let mut parts = line.split_terminator('=');
                        let key = parts.next().unwrap_or("").trim();
                        let val = parts.next().unwrap_or("").trim();
                        if key.to_lowercase() == "protect_mem" {
                            dm.protect_mem = val.to_lowercase() == "true" || val.to_lowercase() == "1";
                        }
                        else if key.to_lowercase() == "defer_rehook" {
                            dm.defer_rehook = val.to_lowercase() == "true" || val.to_lowercase() == "1";
                        }
                        else if key.to_lowercase() == "defer_draw_hook" {
                            dm.defer_draw_hook = val.to_lowercase() == "true" || val.to_lowercase() == "1";
                        }
                    }
                    write_log_file(&format!("DebugMode: protect_mem={}", dm.protect_mem));
                    write_log_file(&format!("DebugMode: defer_rehook={}", dm.defer_rehook));
                    write_log_file(&format!("DebugMode: defer_draw_hook={}", dm.defer_draw_hook));

                    if !dm.defer_rehook {
                        for i in 0..dm.rehook_enabled.len() {
                            dm.rehook_enabled[i] = true;
                        }
                    }
                },
                Err(e) => {
                    write_log_file(&format!("DebugMode: error reading {}: {}", dmfile, e));
                }
            }
        });
    }
}

#[inline]
/// Record the fact that particular hook function was called.  When debug mode is enabled,
/// logs the first time its called and the 1000th time.
pub fn note_called(fn_id: DebugModeCalledFns) {
    DEBUG_MODE.with(|dm| {
        let mut dm = dm.borrow_mut();
        if dm.enabled {
            dm.call_counts[fn_id as usize] += 1;

            if dm.call_counts[fn_id as usize] == 1 {
                write_log_file(&format!("DebugMode: 1st call to {:?}", fn_id));
            }
            else if dm.call_counts[fn_id as usize] == 2 {
                write_log_file(&format!("DebugMode: 2nd call to {:?}", fn_id));
            }
            else if dm.call_counts[fn_id as usize] == 1000 {
                write_log_file(&format!("DebugMode: 1000th call to {:?}", fn_id));
            }
        }
    });
}

#[inline]
/// Returns `true` if the draw function should be hooked.  Normally `true`, but if we're in debug mode
/// and defer_draw_hook is true, then we'll only hook after a certain amount of time has passed.
/// Until that time this function will return `false`.
pub fn draw_hook_enabled() -> bool {
    DEBUG_MODE.with(|dm| {
        let mut dm = dm.borrow_mut();
        if !dm.enabled || !dm.defer_draw_hook || dm.draw_hooked {
            return true;
        }

        let secs_since_start = seconds_since_start(&dm);
        let ok_to_hook = secs_since_start > 35;
        if ok_to_hook && !dm.draw_hooked {
            dm.draw_hooked = true;
            write_log_file(&format!("DebugMode: draw hook enabled"));
        }
        ok_to_hook
    })
}

/// Returns `true` if rehooking is enabled.  Normally this is `true`, but if we're in debug mode and
/// defer_rehook is true, then we'll only rehook after a certain amount of time has passed.
/// This time is based on the name of the calling function.  Rehooking is enabled for different
/// functions at different times to isolate problems with particular functions.
pub fn rehook_enabled(fn_id: DebugModeCalledFns) -> bool {
    DEBUG_MODE.with(|dm| {
        let mut dm = dm.borrow_mut();
        if !dm.enabled {
            return true;
        }

        let mut fn_enabled = dm.rehook_enabled[fn_id as usize];
        if !fn_enabled {
            let secs_since_start = seconds_since_start(&dm);
            fn_enabled = match fn_id {
                DebugModeCalledFns::Hook_DeviceCreateInputLayoutFn if secs_since_start > 30 => true,
                DebugModeCalledFns::Hook_ContextIASetVertexBuffers if secs_since_start > 40 => true,
                DebugModeCalledFns::Hook_ContextIASetInputLayout if secs_since_start > 50 => true,
                DebugModeCalledFns::Hook_ContextIASetPrimitiveTopology if secs_since_start > 50 => true,
                DebugModeCalledFns::Hook_ContextVSSetConstantBuffers if secs_since_start > 90 => true,
                DebugModeCalledFns::Hook_ContextDrawIndexed if secs_since_start > 120 => true,
                _ => false,
            };
            if fn_enabled {
                write_log_file(&format!("DebugMode: rehook enabled for {:?}", fn_id));
                dm.rehook_enabled[fn_id as usize] = true;
            }
        }

        return fn_enabled;
    })
}

#[inline]
/// Returns true if memory should be protected during hook.  Normally `false` as this shouldn't
/// be necessary.  But if running in debug mode, and protect_mem is true, then this will
/// return `true`.
pub fn protect_mem() -> bool {
    DEBUG_MODE.with(|dm| {
        let dm = dm.borrow();
        dm.enabled && dm.protect_mem
    })
}