use winapi::um::libloaderapi::{LoadLibraryW, GetProcAddress};

use util::{write_log_file, load_lib, get_proc_address};
use util::Result;

pub fn init_clr() -> Result<()> {
    let h = load_lib("mscoree.dll")?;
    let clr_create_instance = get_proc_address(h, "CLRCreateInstance")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_init_clr() {
        let _r = init_clr()
        .map_err(|err| {
            assert!(false, "Expected Ok but got {:?}", err)
         });
    }
}

