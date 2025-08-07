pub fn find_mm_root() -> anyhow::Result<String> {
    let mut current_dir = std::env::current_dir()?;

    loop {
        let tplib_dir = current_dir.join("TPLib");
        if tplib_dir.exists() && tplib_dir.is_dir() {
            if let Some(parent) = current_dir.parent() {
                return Ok(parent.to_string_lossy().into_owned());
            } else {
                break;
            }
        }

        if !current_dir.pop() {
            break;
        }
    }

    Err(anyhow::anyhow!("Could not find the MM root directory."))
}
