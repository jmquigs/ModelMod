use std;
use winapi;

#[derive(Debug,Clone)]
pub enum HookError {
    ProtectFailed,
    GlobalStateCopyFailed,
}

pub type Result<T> = std::result::Result<T, HookError>;

pub fn write_log_file(format:String) -> () {
    use std::io::Write;
    use std::fs::OpenOptions;

    let w = || -> std::io::Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open("D:\\Temp\\rd3dlog.txt")?;
        writeln!(f, "{}\r", format)?;
        Ok(())
    };

    w().unwrap_or_else(|e| eprintln!("oops can't write log file: {}", e));
}

pub unsafe fn protect_memory(target: *mut winapi::ctypes::c_void, size:usize, protection:u32) -> Result<u32> {
    let process = winapi::um::processthreadsapi::GetCurrentProcess();
    let mut old_protection = winapi::um::winnt::PAGE_READWRITE;    
    if winapi::um::memoryapi::VirtualProtectEx(process, 
            target as *mut winapi::ctypes::c_void, 
            size, 
            protection, 
            (&mut old_protection) as *mut u32) > 0 {
                Ok(old_protection)
    } else {
        Err(HookError::ProtectFailed)
    }    
}

pub unsafe fn unprotect_memory(target: *mut winapi::ctypes::c_void, size:usize) -> Result<u32> {
    protect_memory(target, size, winapi::um::winnt::PAGE_READWRITE)
}