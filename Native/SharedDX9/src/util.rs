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
