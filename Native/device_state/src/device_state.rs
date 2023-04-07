use shared_dx::types::{DeviceState, HookD3D11State, HookDeviceState};
use std::{ptr::null_mut, sync::{RwLockWriteGuard, RwLockReadGuard}};
use shared_dx::util::write_log_file;

// At some point need to make this private and just use accessor functions (especially with locks)
pub static mut DEVICE_STATE: *mut DeviceState = null_mut();

/// This is primarily used by tests at the moment that need the device state.  In general MM
/// is not currently thread safe, as all supported games use only a single render
/// thread, but that might change in the future.  The perf overhead of locking inside
/// draw primitive is probably too great but a thread local might be viable, however I tried
/// that before and didn't like it, but maybe I was just wrong (prev commit is in
/// dc46643366ba6f44306ee79448afd73aec5038aa ).
/// Note: if you are using this and the log lock in a test, lock the log first ^_^
pub static mut DEVICE_STATE_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

pub fn dev_state() -> &'static mut DeviceState {
    unsafe {
        if DEVICE_STATE == null_mut() {
            write_log_file("accessing null device state pointer, this 'should never happen'.  we gonna crash boys");
            panic!("Aborting because I'm about to dereference a null device state pointer.");
        }
        &mut (*DEVICE_STATE)
    }
}

/// As `dev_state()` but only returns the d3d11 state.  No locking.  Returns None if no state
/// or current state is not d3d11.
pub unsafe fn dev_state_d3d11_nolock<'a>() -> Option<&'a mut HookD3D11State> {
    match dev_state().hook {
        Some(HookDeviceState::D3D11(ref mut h)) => {
            Some(h)
        },
        _ => None
    }
}

// TODO11 benchmark this and use it where needed
// As `dev_state()` but also returns only the d3d11 state and locks.
pub unsafe fn dev_state_d3d11_write<'a>() -> Option<(RwLockWriteGuard<'a, ()>, &'a mut HookD3D11State)> {
    match DEVICE_STATE_LOCK.write() {
        Ok(lock) => {
            match dev_state().hook {
                Some(HookDeviceState::D3D11(ref mut h)) => {
                    Some((lock,h))
                },
                _ => None
            }
        },
        Err(_) => {
            write_log_file(&format!("dev_state_d3d11_write: failed to get lock"));
            None
        }
    }
}

pub unsafe fn dev_state_d3d11_read<'a>() -> Option<(RwLockReadGuard<'a, ()>, &'a mut HookD3D11State)> {
    match DEVICE_STATE_LOCK.read() {
        Ok(lock) => {
            match dev_state().hook {
                Some(HookDeviceState::D3D11(ref mut h)) => {
                    Some((lock,h))
                },
                _ => None
            }
        },
        Err(_) => {
            write_log_file(&format!("dev_state_d3d11_write: failed to get lock"));
            None
        }
    }
}
