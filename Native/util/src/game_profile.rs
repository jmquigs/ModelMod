/// Early game-profile lookup from the Windows registry.
///
/// At hook time (device creation), the managed CLR has not yet been loaded, so
/// the F# `RegConfig.load` path is unavailable.  This module replicates the
/// profile-matching logic in pure Rust so that profile settings can influence
/// which functions are hooked.  `RegConfig.pickBestProfile` is the F# counterpart of
/// `pick_best_profile` below; the two must agree on which profile a given exe resolves to.
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

/// Minimum number of trailing path components (including the exe file name) that a profile
/// must share with the target before it is considered a match.  A value of 1 would match on
/// file name alone, which is too weak: many games ship a generically named exe.
const MIN_MATCH_COMPONENTS: usize = 2;

/// Split a path into components, ignoring separator style and empty parts.
/// The caller is responsible for case normalization.
fn path_components(p: &str) -> Vec<&str> {
    p.split(|c| c == '\\' || c == '/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Number of trailing components shared by both paths.
fn common_suffix_len(a: &[&str], b: &[&str]) -> usize {
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Choose the profile whose `ExePath` best matches `exe_path`.
///
/// `profiles` is a list of (profile key, ExePath) pairs in registry order.  An exact match
/// wins outright; failing that, the profile sharing the most trailing path components wins,
/// as long as it shares at least `MIN_MATCH_COMPONENTS`.  This lets a single profile serve a
/// game that is reachable by more than one path, which is normal under proton (the same
/// install is visible as both `Z:\` and a steam library drive).  Ties are broken by registry
/// order and logged, since they usually mean there are leftover duplicate profiles.
///
/// Splitting on components also makes the match insensitive to the `\\?\` prefix.
fn pick_best_profile(exe_path: &str, profiles: &[(String, String)]) -> Option<String> {
    let exe_lower = exe_path.trim().to_lowercase();
    if exe_lower.is_empty() {
        write_log_file("find_profile_for_exe: empty exe path, no profile lookup possible");
        return None;
    }
    let exe_comps = path_components(&exe_lower);

    // (score, key, path) for each profile that is close enough to consider.
    let mut candidates: Vec<(usize, String, String)> = Vec::new();

    for (key, prof_exe) in profiles {
        let prof_lower = prof_exe.trim().to_lowercase();
        if prof_lower.is_empty() {
            continue;
        }
        if prof_lower == exe_lower {
            write_log_file(&format!("find_profile_for_exe: exact match on {}", key));
            return Some(key.clone());
        }
        let score = common_suffix_len(&exe_comps, &path_components(&prof_lower));
        if score >= MIN_MATCH_COMPONENTS {
            candidates.push((score, key.clone(), prof_lower));
        }
    }

    // Sort is stable, so registry order breaks ties.
    candidates.sort_by_key(|c| std::cmp::Reverse(c.0));

    let best_score = match candidates.first() {
        None => {
            write_log_file(&format!(
                "find_profile_for_exe: no profile matches {:?}", exe_lower));
            return None;
        }
        Some(c) => c.0,
    };

    {
        let tied: Vec<&str> = candidates
            .iter()
            .take_while(|c| c.0 == best_score)
            .map(|c| c.2.as_str())
            .collect();
        if tied.len() > 1 {
            write_log_file(&format!(
                "find_profile_for_exe: ambiguous, {} profiles tie at {} component(s): {:?}; \
                 using the first.  consider removing the duplicates",
                tied.len(), best_score, tied));
        }
    }

    let (score, key, path) = candidates.remove(0);
    write_log_file(&format!(
        "find_profile_for_exe: {:?} matched profile {} ({:?}) on {} trailing component(s)",
        exe_lower, key, path, score));
    Some(key)
}

/// Find the profile registry path whose `ExePath` matches the current executable.
///
/// Returns the full registry path (e.g. `Software\ModelMod\Profiles\Profile0000`)
/// or `None` if no match is found.
unsafe fn find_profile_for_exe(exe_path: &str) -> Result<Option<String>> {
    let profiles_root = format!("{}\\Profiles", get_mm_reg_key());
    let subkeys = reg_enum_subkeys(&profiles_root)?;

    let mut profiles: Vec<(String, String)> = Vec::with_capacity(subkeys.len());
    let mut unreadable = 0;
    for key_name in &subkeys {
        let full_key = format!("{}\\{}", profiles_root, key_name);
        match reg_query_string(&full_key, "ExePath") {
            Ok(prof_exe) => profiles.push((full_key, prof_exe)),
            Err(_) => unreadable += 1,
        }
    }
    write_log_file(&format!(
        "find_profile_for_exe: {} profile(s) under {}, {} with no readable ExePath",
        profiles.len(), profiles_root, unreadable));

    Ok(pick_best_profile(exe_path, &profiles))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn profs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, p)| (k.to_string(), p.to_string())).collect()
    }

    #[test]
    fn exact_match_ignores_case_and_whitespace() {
        let p = profs(&[("P0", "  C:\\Games\\Foo\\Bin\\Game.exe  ")]);
        assert_eq!(pick_best_profile("c:\\games\\foo\\bin\\game.exe", &p), Some("P0".into()));
    }

    #[test]
    fn matches_same_install_through_different_drive_mappings() {
        // the profile was created against the Z: (unix root) view; the game is launched
        // through a steam library drive.
        let p = profs(&[("P0", "Z:\\home\\me\\SteamLibrary\\steamapps\\common\\Foo\\bin\\game.exe")]);
        assert_eq!(
            pick_best_profile("S:\\steamapps\\common\\Foo\\bin\\game.exe", &p),
            Some("P0".into()));
        // a hand-made mapping straight at the game dir still shares 3 components.
        assert_eq!(
            pick_best_profile("C:\\Foo\\bin\\game.exe", &p),
            Some("P0".into()));
    }

    #[test]
    fn separator_style_and_object_dir_prefix_are_ignored() {
        let p = profs(&[("P0", "S:/steamapps/common/Foo/bin/game.exe")]);
        assert_eq!(
            pick_best_profile("\\\\?\\S:\\steamapps\\common\\Foo\\bin\\game.exe", &p),
            Some("P0".into()));
    }

    #[test]
    fn file_name_alone_is_not_enough() {
        let p = profs(&[("P0", "C:\\Games\\Bar\\game.exe")]);
        assert_eq!(pick_best_profile("C:\\Games\\Foo\\game.exe", &p), None);
    }

    #[test]
    fn best_score_wins_over_a_weaker_match() {
        let p = profs(&[
            ("P0", "C:\\Games\\Bar\\bin\\game.exe"),
            ("P1", "Z:\\home\\me\\steamapps\\common\\Foo\\bin\\game.exe"),
        ]);
        assert_eq!(
            pick_best_profile("S:\\steamapps\\common\\Foo\\bin\\game.exe", &p),
            Some("P1".into()));
    }

    #[test]
    fn empty_paths_never_match() {
        let p = profs(&[("P0", ""), ("P1", "   ")]);
        assert_eq!(pick_best_profile("C:\\Games\\Foo\\game.exe", &p), None);
        assert_eq!(pick_best_profile("", &profs(&[("P0", "C:\\Games\\Foo\\game.exe")])), None);
    }

    #[test]
    fn ties_resolve_to_the_first_profile_in_registry_order() {
        let p = profs(&[
            ("P0", "S:\\steamapps\\common\\Foo\\bin\\game.exe"),
            ("P1", "Z:\\home\\me\\steamapps\\common\\Foo\\bin\\game.exe"),
        ]);
        // both share "common\Foo\bin\game.exe"; P0 wins on order, not on score.
        assert_eq!(
            pick_best_profile("C:\\steamapps\\common\\Foo\\bin\\game.exe", &p),
            Some("P0".into()));
    }
}
