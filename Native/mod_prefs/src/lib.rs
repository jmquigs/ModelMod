//! Per-game user preferences stored in `%LOCALAPPDATA%\ModelMod\<gamename>.prefs.yaml`.
//!
//! Currently tracks which variant index is selected for each (ref prim, ref vert)
//! geometry that has multiple non-parented mods. The file format is intentionally
//! extensible (a versioned YAML document with sections) so additional preferences
//! can be added later without breaking older files.
//!
//! This lives in its own crate to keep `serde`/`serde_yaml` out of the dependency
//! graph of `mod_load` (a central crate whose incremental rebuild time is
//! sensitive to proc-macro-heavy deps).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use global_state::{LoadedModsMap, SelectedVariantMap};
use shared_dx::util::write_log_file;
use types::native_mod::NativeModData;

const PREFS_VERSION: u32 = 1;
const PREFS_SUBDIR: &str = "ModelMod";
const PREFS_EXT: &str = "prefs.yaml";

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub struct VariantPref {
    pub ref_prim_count: u32,
    pub ref_vert_count: u32,
    pub index: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub struct ModPrefs {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub variants: Vec<VariantPref>,
}

fn default_version() -> u32 { PREFS_VERSION }

impl ModPrefs {
    pub fn new() -> Self {
        Self { version: PREFS_VERSION, variants: Vec::new() }
    }
}

impl Default for ModPrefs {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
// this locate's this crate's "target", not the workspace, but is otherwise ok for test purposes
fn prefs_dir_base() -> Option<PathBuf> {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("tmp");
    // rust names test thread after current test
    let test_name = std::thread::current()
        .name()
        .unwrap_or("unknown_test")
        .replace("::", "_");

// FIX: 安全检查 — 防止目录穿越
let path = {}.canonicalize().map_err(|_| Error::InvalidPath)?;
if !path.starts_with(&base_dir) {
    return Err(Error::PathTraversalDetected);
}

    let fnopath = file!().replace("\\", "_").replace("/", "_").replace("..", "_");
    path.push(format!("test_run_{}_{}", fnopath, test_name)); // Uses the line number as a simple unique ID
    path.push(PREFS_SUBDIR);
    Some(path)
}

#[cfg(not(test))]
fn prefs_dir_base() -> Option<PathBuf> {
    match std::env::var("LOCALAPPDATA") {
        Ok(v) if !v.is_empty() => {
            let mut pb = PathBuf::from(v);
            pb.push(PREFS_SUBDIR);
            Some(pb)
        },
        _ => {
            None
        }
    }
}

fn prefs_dir() -> Option<PathBuf> {
    match prefs_dir_base() {
        Some(x) => Some(x),
        _ => {
            write_log_file("prefs: LOCALAPPDATA environment variable not set; variant prefs disabled");
            None
        }
    }
}

fn prefs_file_path() -> Option<PathBuf> {
    let stem = match util::get_module_name_base() {
        Ok(s) => s,
        Err(e) => {
            write_log_file(&format!("prefs: cannot determine game name: {:?}", e));
            return None;
        }
    };
    let mut dir = prefs_dir()?;
    dir.push(format!("{}.{}", stem, PREFS_EXT));
    Some(dir)
}

fn read_prefs() -> Option<ModPrefs> {
    let pb = prefs_file_path()?;
    if !pb.is_file() {
        return None;
    }

    let file = match std::fs::File::open(&pb) {
        Ok(f) => f,
        Err(e) => {
            write_log_file(&format!("prefs: failed to open {:?}: {:?}", pb, e));
            return None;
        }
    };
    let reader = std::io::BufReader::new(file);
    match serde_yaml::from_reader::<_, ModPrefs>(reader) {
        Ok(p) => {
            write_log_file(&format!("prefs: loaded from {:?} (version {}, {} variants)",
                pb, p.version, p.variants.len()));
            Some(p)
        },
        Err(e) => {
            write_log_file(&format!("prefs: failed to parse {:?}: {}", pb, e));
            None
        }
    }
}

fn write_prefs(prefs: &ModPrefs) {
    let pb = match prefs_file_path() {
        Some(p) => p,
        None => return,
    };
    if let Some(parent) = pb.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            write_log_file(&format!("prefs: failed to create dir {:?}: {:?}", parent, e));
            return;
        }
    }
    let yaml = match serde_yaml::to_string(prefs) {
        Ok(s) => s,
        Err(e) => {
            write_log_file(&format!("prefs: serialization failed: {}", e));
            return;
        }
    };
    if let Err(e) = std::fs::write(&pb, yaml) {
        write_log_file(&format!("prefs: failed to write {:?}: {:?}", pb, e));
    }
}

/// Apply `prefs` entries to `selected_variant` for each entry whose index is in
/// range for the currently loaded mods. Entries that refer to ref geometry with
/// no loaded mods, or whose index is out of range, are silently dropped
/// (effectively resetting to 0). Returns `(applied, total)` counts.
fn apply_variants(prefs: &ModPrefs, mods: &LoadedModsMap,
                  selected_variant: &mut SelectedVariantMap) -> (usize, usize) {
    let mut applied = 0;
    for entry in &prefs.variants {
        let mod_key = NativeModData::mod_key(entry.ref_vert_count, entry.ref_prim_count);
        match mods.get(&mod_key) {
            Some(nmdv) if entry.index < nmdv.len() => {
                selected_variant.insert(mod_key, entry.index);
                applied += 1;
            },
            Some(nmdv) => {
                write_log_file(&format!(
                    "prefs: variant pref for ref ({}p, {}v) index {} out of range (list size {}), resetting to 0",
                    entry.ref_prim_count, entry.ref_vert_count, entry.index, nmdv.len()));
            },
            None => {
                // no loaded mods for that ref geom; just skip
            }
        }
    }
    (applied, prefs.variants.len())
}

/// Build a `ModPrefs` from the current non-zero `selected_variant` entries.
/// Zero indices are omitted (default selection does not need to be persisted).
/// Entries whose ref geometry has no loaded mods are skipped. The resulting
/// variants list is sorted by `(ref_prim_count, ref_vert_count)` for stable
/// on-disk output.
fn build_prefs(mods: &LoadedModsMap, selected_variant: &SelectedVariantMap) -> ModPrefs {
    let mut prefs = ModPrefs::new();
    for (mod_key, index) in selected_variant.iter() {
        if *index == 0 {
            // default; don't persist
            continue;
        }
        if let Some(nmdv) = mods.get(mod_key) {
            if let Some(first) = nmdv.first() {
                prefs.variants.push(VariantPref {
                    ref_prim_count: first.mod_data.numbers.ref_prim_count as u32,
                    ref_vert_count: first.mod_data.numbers.ref_vert_count as u32,
                    index: *index,
                });
            }
        }
    }
    prefs.variants.sort_by_key(|v| (v.ref_prim_count, v.ref_vert_count));
    prefs
}

/// Read the prefs file (if any) and populate `selected_variant` for each entry
/// whose index is in range for the currently loaded mods. Entries that refer to
/// ref geometry with no loaded mods, or whose index is out of range, are silently
/// dropped (effectively resetting to 0).
pub fn load_and_apply_variants(mods: &LoadedModsMap, selected_variant: &mut SelectedVariantMap) {
    let prefs = match read_prefs() {
        Some(p) => p,
        None => return,
    };
    let (applied, total) = apply_variants(&prefs, mods, selected_variant);
    write_log_file(&format!("prefs: applied {} of {} saved variant selections", applied, total));
}

/// Serialize the current non-zero `selected_variant` entries to the prefs file.
pub fn save_variant_selections(mods: &LoadedModsMap, selected_variant: &SelectedVariantMap) {
    let prefs = build_prefs(mods, selected_variant);
    write_prefs(&prefs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_state::new_fnv_map;

    fn new_mod(ref_prims: i32, ref_verts: i32) -> NativeModData {
        let mut m = NativeModData::new();
        m.mod_data.numbers.ref_prim_count = ref_prims;
        m.mod_data.numbers.ref_vert_count = ref_verts;
        m
    }

    fn add_mods(mmap: &mut LoadedModsMap, ref_prims: i32, ref_verts: i32, count: usize) {
        let mk = NativeModData::mod_key(ref_verts as u32, ref_prims as u32);
        let v = mmap.entry(mk).or_insert_with(|| vec![]);
        for _ in 0..count {
            v.push(new_mod(ref_prims, ref_verts));
        }
    }

    #[test]
    fn new_and_default_use_current_version() {
        let p = ModPrefs::new();
        assert_eq!(p.version, PREFS_VERSION);
        assert!(p.variants.is_empty());

        let d = ModPrefs::default();
        assert_eq!(d.version, PREFS_VERSION);
        assert!(d.variants.is_empty());
    }

    #[test]
    fn yaml_roundtrip_preserves_entries() {
        let mut p = ModPrefs::new();
        p.variants.push(VariantPref { ref_prim_count: 10, ref_vert_count: 20, index: 3 });
        p.variants.push(VariantPref { ref_prim_count: 11, ref_vert_count: 21, index: 1 });

        let yaml = serde_yaml::to_string(&p).expect("serialize");
        let p2: ModPrefs = serde_yaml::from_str(&yaml).expect("deserialize");

        assert_eq!(p2.version, p.version);
        assert_eq!(p2.variants.len(), p.variants.len());
        for (a, b) in p.variants.iter().zip(p2.variants.iter()) {
            assert_eq!(a.ref_prim_count, b.ref_prim_count);
            assert_eq!(a.ref_vert_count, b.ref_vert_count);
            assert_eq!(a.index, b.index);
        }
    }

    #[test]
    fn deserialize_uses_defaults_for_missing_fields() {
        // Empty document -> all defaults.
        let p: ModPrefs = serde_yaml::from_str("{}").unwrap();
        assert_eq!(p.version, PREFS_VERSION);
        assert!(p.variants.is_empty());

        // Only `variants` provided -> default version.
        let p: ModPrefs = serde_yaml::from_str("variants: []").unwrap();
        assert_eq!(p.version, PREFS_VERSION);

        // Only `version` provided -> default (empty) variants, unchanged version.
        let p: ModPrefs = serde_yaml::from_str("version: 99").unwrap();
        assert_eq!(p.version, 99);
        assert!(p.variants.is_empty());
    }

    #[test]
    fn apply_variants_sets_selection_when_in_range() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 7, 13, 3); // 3 variants for ref (7p,13v)

        let mut prefs = ModPrefs::new();
        prefs.variants.push(VariantPref { ref_prim_count: 7, ref_vert_count: 13, index: 2 });

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let (applied, total) = apply_variants(&prefs, &mods, &mut sel);

        assert_eq!(applied, 1);
        assert_eq!(total, 1);
        let mk = NativeModData::mod_key(13, 7);
        assert_eq!(sel.get(&mk), Some(&2));

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn apply_variants_skips_out_of_range_index() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 7, 13, 2); // only 2 variants

        let mut prefs = ModPrefs::new();
        prefs.variants.push(VariantPref { ref_prim_count: 7, ref_vert_count: 13, index: 5 });

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let (applied, total) = apply_variants(&prefs, &mods, &mut sel);

        assert_eq!(applied, 0);
        assert_eq!(total, 1);
        assert!(sel.is_empty(), "out-of-range index must not be applied");

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn apply_variants_skips_unknown_ref_geometry() {
        let mods: LoadedModsMap = new_fnv_map(4); // no mods loaded

        let mut prefs = ModPrefs::new();
        prefs.variants.push(VariantPref { ref_prim_count: 1, ref_vert_count: 2, index: 0 });

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let (applied, total) = apply_variants(&prefs, &mods, &mut sel);

        assert_eq!(applied, 0);
        assert_eq!(total, 1);
        assert!(sel.is_empty());

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn apply_variants_handles_mixed_entries() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 7, 13, 3);  // ok
        add_mods(&mut mods, 8, 14, 2);  // entry below will be out of range

        let mut prefs = ModPrefs::new();
        prefs.variants.push(VariantPref { ref_prim_count: 7, ref_vert_count: 13, index: 1 }); // in range
        prefs.variants.push(VariantPref { ref_prim_count: 8, ref_vert_count: 14, index: 9 }); // out of range
        prefs.variants.push(VariantPref { ref_prim_count: 99, ref_vert_count: 99, index: 0 }); // unknown

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let (applied, total) = apply_variants(&prefs, &mods, &mut sel);

        assert_eq!(applied, 1);
        assert_eq!(total, 3);
        let good = NativeModData::mod_key(13, 7);
        assert_eq!(sel.get(&good), Some(&1));
        assert_eq!(sel.len(), 1);

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn build_prefs_omits_zero_indices() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 5, 10, 2);

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let mk = NativeModData::mod_key(10, 5);
        sel.insert(mk, 0); // default; should not be persisted

        let prefs = build_prefs(&mods, &sel);
        assert!(prefs.variants.is_empty());
        assert_eq!(prefs.version, PREFS_VERSION);

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn build_prefs_records_nonzero_index() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 5, 10, 3);

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        let mk = NativeModData::mod_key(10, 5);
        sel.insert(mk, 2);

        let prefs = build_prefs(&mods, &sel);
        assert_eq!(prefs.variants.len(), 1);
        let v = &prefs.variants[0];
        assert_eq!(v.ref_prim_count, 5);
        assert_eq!(v.ref_vert_count, 10);
        assert_eq!(v.index, 2);

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn build_prefs_sorts_by_ref_counts() {
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 10, 5, 2);
        add_mods(&mut mods, 5, 50, 2);
        add_mods(&mut mods, 5, 10, 2);

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        sel.insert(NativeModData::mod_key(5, 10), 1);
        sel.insert(NativeModData::mod_key(50, 5), 1);
        sel.insert(NativeModData::mod_key(10, 5), 1);

        let prefs = build_prefs(&mods, &sel);
        let keys: Vec<(u32, u32)> = prefs.variants.iter()
            .map(|v| (v.ref_prim_count, v.ref_vert_count))
            .collect();
        assert_eq!(keys, vec![(5, 10), (5, 50), (10, 5)]);

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn build_prefs_skips_selection_with_no_loaded_mods() {
        let mods: LoadedModsMap = new_fnv_map(4); // empty

        let mut sel: SelectedVariantMap = new_fnv_map(4);
        sel.insert(NativeModData::mod_key(1, 2), 1);

        let prefs = build_prefs(&mods, &sel);
        assert!(prefs.variants.is_empty());

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));
    }

    #[test]
    fn build_then_apply_round_trip() {
        // Create a loaded-mods map and a selection, build prefs, then re-apply
        // to a fresh selection map and verify we land on the same state.
        let mut mods: LoadedModsMap = new_fnv_map(4);
        add_mods(&mut mods, 5, 10, 3);
        add_mods(&mut mods, 20, 40, 4);

        let mut sel_src: SelectedVariantMap = new_fnv_map(4);
        sel_src.insert(NativeModData::mod_key(10, 5), 2);
        sel_src.insert(NativeModData::mod_key(40, 20), 3);

        let prefs = build_prefs(&mods, &sel_src);

        // Round-trip through YAML to also cover the serde layer.
        let yaml = serde_yaml::to_string(&prefs).unwrap();
        let prefs2: ModPrefs = serde_yaml::from_str(&yaml).unwrap();

        let mut sel_dst: SelectedVariantMap = new_fnv_map(4);
        let (applied, total) = apply_variants(&prefs2, &mods, &mut sel_dst);
        assert_eq!(applied, 2);
        assert_eq!(total, 2);

        assert_eq!(sel_dst.get(&NativeModData::mod_key(10, 5)), Some(&2));
        assert_eq!(sel_dst.get(&NativeModData::mod_key(40, 20)), Some(&3));

        write_prefs(&prefs);
        let res = read_prefs();
        assert_eq!(res, Some(prefs));

        write_prefs(&prefs2);
        let res = read_prefs();
        assert_eq!(res, Some(prefs2));
    }
}
