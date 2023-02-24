use crate::error::{HookError, Result};

lazy_static! {
    static ref LOG_FILE_NAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
}

pub fn set_log_file_path(path: &str, name: &str) -> Result<()> {
    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => Err(HookError::WinApiError(format!("lock error: {}", e))),
        Ok(mut fname) => {
            let mut p = path.to_owned();
            p.push_str(name);
            *fname = p;
            Ok(())
        }
    }
}

/// Return the log file path or "" if there was an error.  This function will temporarily lock
/// a global mutex protecting access to the variable.
pub fn get_log_file_path() -> String {
    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => {
            eprintln!(
                "ModelMod: derp, can't write log file due to lock error: {}",
                e
            );
            "".to_owned()
        }
        Ok(fname) => {
            (*fname).to_owned()
        }
    }
}

pub fn write_log_file(msg: &str) -> () {
    use std::env::temp_dir;
    use std::fs::OpenOptions;
    use std::io::Write;

    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => {
            eprintln!(
                "ModelMod: derp, can't write log file due to lock error: {}",
                e
            );
        }
        Ok(mut fname) => {
            if (*fname).is_empty() {
                let mut td = temp_dir();
                println!("no log path, writing log to {:?}", td);
                td.push("ModelMod.log");
                match td.as_path().to_str() {
                    None => {
                        eprintln!("ModelMod: error getting temp path");
                        return;
                    }
                    Some(p) => {
                        *fname = p.to_owned();
                    }
                }
            }

            let tid = std::thread::current().id();

            let w = || -> std::io::Result<()> {
                let mut f = OpenOptions::new().create(true).append(true).open(&*fname)?;
                writeln!(f, "{:?}: {}\r", tid, msg)?;
                Ok(())
            };

            w().unwrap_or_else(|e| eprintln!("ModelMod: log file write error: {}", e));
        }
    };
}

pub trait ReleaseDrop {
    fn OnDrop(&mut self) -> ();
}

pub struct ReleaseOnDrop<T: ReleaseDrop> {
    rd: T,
}

impl<T> ReleaseOnDrop<T>
where
    T: ReleaseDrop,
{
    pub fn new(rd: T) -> Self {
        ReleaseOnDrop { rd: rd }
    }
}

impl<T> std::ops::Drop for ReleaseOnDrop<T>
where
    T: ReleaseDrop,
{
    fn drop(&mut self) {
        self.rd.OnDrop();
    }
}

#[macro_export]
macro_rules! impl_release_drop {
    ($ptrtype:ident) => {
        impl crate::util::ReleaseDrop for *mut $ptrtype {
            fn OnDrop(&mut self) -> () {
                unsafe {
                    let ptr = *self;
                    if ptr != std::ptr::null_mut() {
                        (*ptr).Release();
                    }
                };
            }
        }
    };
}
