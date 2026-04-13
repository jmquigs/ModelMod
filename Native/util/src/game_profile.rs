/// Early game-profile lookup from the Windows registry.
///
/// At hook time (device creation), the managed CLR has not yet been loaded, so
/// the F# `RegConfig.load` path is unavailable.  This module replicates the
/// profile-matching logic in pure Rust so that profile settings can influence
/// which functions are hooked.
///
/// Registry layout (all under `HKCU\Software\ModelMod`):
///
/// ```text
/// Profiles\
///   Profile0000\
///     ExePath              REG_SZ    "C:\Games\foo.exe"
///     GameProfileReverseNormals        REG_DWORD
///     GameProfileUpdateTangents        REG_DWORD
///     GameProfileDataPathName          REG_SZ
///     ...
///   Profile0001\
///     ...
/// ```

use std::os::windows::ffi::OsStringExt;

use shared_dx::error::*;
use shared_dx::util::write_log_file;
use winapi::um::winnt::KEY_READ;

use crate::{reg_query_dword, reg_query_string, to_wide_str, get_module_name};

#[cfg(test)]
fn get_mm_reg_key() -> &'static str {
    "Software\\ModelModTEST"
}
#[cfg(not(test))]
fn get_mm_reg_key() -> &'static str {
    "Software\\ModelMod"
}

/// Settings read from the game profile in the registry.
#[derive(Debug, Clone)]
pub struct GameProfile {
    /// The registry path for this profile (e.g. `Software\ModelMod\Profiles\Profile0000`).
    /// Empty string if no profile was found.
    pub profile_key: String,
    pub reverse_normals: bool,
    pub update_tangent_space: bool,
    pub data_path_name: String,
    /// Whether to enable dx9 systemmem tracking for texture snapshots.  Default is false. 
    /// 
    /// At least one game (2026g1) creates textures in 
    /// the sysmem d3d pool, and then creates another for the device to use in the default d3d pool, 
    /// and copies the data from source to dest with UpdateTexture.
    /// The textures used for rendering are thus in the default pool and cannot be snapshotted.  
    /// 
    /// When this 
    /// setting is enabled, we track and keep references to the original textures so that we can snap from those instead.
    /// This also enables a garbage collector for the systemmem copies which introduces some performance hit (~10ms every 30secs on my 2015 desktop).
    /// The system textures will be kept for at least 5 minutes after creation.  Once disposed the textures involved may be 
    /// un-snapshotable, but sometimes loading a new level (to trigger the game to produce a fresh systemmem texture) works to refresh them.
    pub snap_use_sysmemtexturetracking: bool,
}

pub const EMPTY_GAME_PROFILE:GameProfile = GameProfile {
    profile_key: String::new(),
    reverse_normals: false,
    update_tangent_space: true,
    data_path_name: String::new(),
    snap_use_sysmemtexturetracking: false,
};

impl Default for GameProfile {
    fn default() -> Self {
        EMPTY_GAME_PROFILE
    }
}

/// Enumerate subkey names under `parent_path` (relative to HKCU).
///
/// Returns a sorted list of subkey names (e.g. `["Profile0000", "Profile0001"]`).
unsafe fn reg_enum_subkeys(parent_path: &str) -> Result<Vec<String>> {
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror::ERROR_SUCCESS;
    use winapi::um::winreg::*;

    let wide_path = to_wide_str(parent_path);
    let mut hkey: winapi::shared::minwindef::HKEY = std::ptr::null_mut();
    let res = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        wide_path.as_ptr(),
        0,
        KEY_READ,
        &mut hkey,
    );
    if res as DWORD != ERROR_SUCCESS {
        // Key doesn't exist — no profiles at all.
        return Ok(Vec::new());
    }

    let mut names: Vec<String> = Vec::new();
    let mut index: DWORD = 0;
    loop {
        let mut name_buf: [u16; 256] = [0; 256];
        let mut name_len: DWORD = 256;
        let res = RegEnumKeyExW(
            hkey,
            index,
            name_buf.as_mut_ptr(),
            &mut name_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if res as DWORD != ERROR_SUCCESS {
            break;
        }
        let name_slice = &name_buf[..name_len as usize];
        if let Ok(name) = std::ffi::OsString::from_wide(name_slice).into_string() {
            names.push(name);
        }
        index += 1;
    }

    RegCloseKey(hkey);
    names.sort();
    Ok(names)
}

/// Find the profile registry path whose `ExePath` matches the current executable.
///
/// Returns the full registry path (e.g. `Software\ModelMod\Profiles\Profile0000`)
/// or `None` if no match is found.
unsafe fn find_profile_for_exe(exe_path: &str) -> Result<Option<String>> {
    let profiles_root = format!("{}\\Profiles", get_mm_reg_key());
    let subkeys = reg_enum_subkeys(&profiles_root)?;

    let exe_lower = exe_path.trim().to_lowercase();

    for key_name in &subkeys {
        let full_key = format!("{}\\{}", profiles_root, key_name);
        if let Ok(prof_exe) = reg_query_string(&full_key, "ExePath") {
            if prof_exe.trim().to_lowercase() == exe_lower {
                return Ok(Some(full_key));
            }
        }
    }
    Ok(None)
}

/// Read a `GameProfile` from a specific profile registry path.
unsafe fn read_profile_from_key(profile_path: &str) -> GameProfile {
    let reverse_normals = reg_query_dword(profile_path, "GameProfileReverseNormals")
        .map(|v| v > 0)
        .unwrap_or(false);
    let update_tangent_space = reg_query_dword(profile_path, "GameProfileUpdateTangents")
        .map(|v| v > 0)
        .unwrap_or(true); // default is true, matching F# DefaultGameProfile
    let data_path_name = reg_query_string(profile_path, "GameProfileDataPathName")
        .unwrap_or_default();
    let snap_use_sysmemtexturetracking = reg_query_dword(profile_path, "GameProfileSnapUseSysmemTextureTracking")
        .map(|v| v > 0)
        .unwrap_or(false);

    GameProfile {
        profile_key: profile_path.to_owned(),
        reverse_normals,
        update_tangent_space,
        data_path_name,
        snap_use_sysmemtexturetracking
    }
}

/// Look up the game profile for the currently running executable.
///
/// This mirrors the logic in `MMManaged/RegConfig.fs :: load` — it enumerates
/// all profile subkeys under `HKCU\Software\ModelMod\Profiles`, finds one whose
/// `ExePath` matches the current process, and reads the GameProfile fields.
///
/// Returns `GameProfile::default()` if no matching profile is found or if any
/// error occurs.
pub fn load_for_current_exe() -> GameProfile {
    unsafe {
        let exe = match get_module_name() {
            Ok(e) => e,
            Err(e) => {
                write_log_file(&format!(
                    "game_profile: failed to get exe module name: {:?}", e
                ));
                return GameProfile::default();
            }
        };

        write_log_file(&format!("game_profile: looking up profile for exe: {}", exe));

        match find_profile_for_exe(&exe) {
            Ok(Some(key)) => {
                let profile = read_profile_from_key(&key);
                write_log_file(&format!(
                    "game_profile: found profile at {}: {:?}", key, profile
                ));
                profile
            }
            Ok(None) => {
                write_log_file("game_profile: no matching profile found, using defaults");
                GameProfile::default()
            }
            Err(e) => {
                write_log_file(&format!(
                    "game_profile: error searching profiles: {:?}", e
                ));
                GameProfile::default()
            }
        }
    }
}
