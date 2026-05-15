use shared_dx::types::{DeviceState, HookD3D11State, HookDeviceState};
use std::{ptr::null_mut, sync::{RwLock, RwLockWriteGuard, RwLockReadGuard}};
use shared_dx::util::write_log_file;

/// Newtype wrapping the raw `DeviceState` pointer so it can live inside a
/// `static RwLock`. The pointer itself is essentially set once at hook
/// install time and torn down only by tests, so the `Send`/`Sync` impls
/// reflect the actual access pattern: the `RwLock` controls concurrent
/// access to the pointee, while the pointer field is only mutated under
/// the write guard during init/cleanup.
pub struct DeviceStatePtr(pub *mut DeviceState);
unsafe impl Send for DeviceStatePtr {}
unsafe impl Sync for DeviceStatePtr {}

impl DeviceStatePtr {
    pub fn is_null(&self) -> bool { self.0.is_null() }
    pub fn as_ptr(&self) -> *mut DeviceState { self.0 }
}

/// Combined replacement for the previous `DEVICE_STATE` raw pointer and the
/// separate `DEVICE_STATE_LOCK` marker. The pointer is owned by the lock, so
/// every reader/writer goes through it.
///
/// In general MM is currently driven from a single render thread, but locking
/// here allows safe access from the deferred mod load thread and from tests.
/// Note: if you are using this and the log lock in a test, lock the log first ^_^
pub static DEVICE_STATE: RwLock<DeviceStatePtr> = RwLock::new(DeviceStatePtr(null_mut()));

/// Acquire a write guard on the device state, returning both the guard and a
/// `&mut DeviceState`. Returns `None` if the lock is poisoned or the pointer
/// is null. On poison this logs and proceeds in fail-safe mode rather than
/// crashing the host process.
pub fn dev_state_write<'a>() -> Option<(RwLockWriteGuard<'a, DeviceStatePtr>, &'a mut DeviceState)> {
    match DEVICE_STATE.write() {
        Ok(mut lock) => {
            if lock.0.is_null() {
                return None;
            }
            // SAFETY: the write guard provides exclusive access to the pointer
            // and (by convention in this module) to the pointee. The lifetime
            // is tied to the guard via the function signature.
            let ptr = lock.0;
            let r: &mut DeviceState = unsafe { &mut *ptr };
            // Suppress unused-mut: the binding must be `mut` so callers
            // (after destructuring) can mutate through the guard if needed.
            let _ = &mut lock;
            Some((lock, r))
        }
        Err(e) => {
            write_log_file(&format!("dev_state_write: lock poisoned: {}", e));
            None
        }
    }
}

/// Acquire a read guard on the device state, returning both the guard and a
/// `&DeviceState`. Returns `None` if the lock is poisoned or the pointer is
/// null. On poison this logs and proceeds in fail-safe mode.
pub fn dev_state_read<'a>() -> Option<(RwLockReadGuard<'a, DeviceStatePtr>, &'a DeviceState)> {
    match DEVICE_STATE.read() {
        Ok(lock) => {
            if lock.0.is_null() {
                return None;
            }
            // SAFETY: the read guard ensures no writer is active. We only
            // hand out a shared reference to the pointee.
            let r: &DeviceState = unsafe { &*lock.0 };
            Some((lock, r))
        }
        Err(e) => {
            write_log_file(&format!("dev_state_read: lock poisoned: {}", e));
            None
        }
    }
}

/// As `dev_state_write` but only returns the d3d11 state. Returns None if no
/// state is initialized, the current state is not d3d11, or the lock is
/// poisoned.
pub fn dev_state_d3d11_write<'a>() -> Option<(RwLockWriteGuard<'a, DeviceStatePtr>, &'a mut HookD3D11State)> {
    let (lock, ds) = dev_state_write()?;
    match ds.hook {
        Some(HookDeviceState::D3D11(ref mut h)) => Some((lock, h)),
        _ => None,
    }
}

/// As `dev_state_read` but only returns the d3d11 state, immutably. Returns
/// None if no state is initialized, the current state is not d3d11, or the
/// lock is poisoned.
///
/// Note: unlike the previous version of this function, this returns `&` not
/// `&mut`. Use `dev_state_d3d11_write` if mutation is needed.
pub fn dev_state_d3d11_read<'a>() -> Option<(RwLockReadGuard<'a, DeviceStatePtr>, &'a HookD3D11State)> {
    let (lock, ds) = dev_state_read()?;
    match ds.hook {
        Some(HookDeviceState::D3D11(ref h)) => Some((lock, h)),
        _ => None,
    }
}
