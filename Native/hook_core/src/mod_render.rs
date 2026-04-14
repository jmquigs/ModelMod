use std::collections::HashMap;

use fnv::FnvHashMap;
use global_state::{LoadedModState, MAX_CHECKSUM_STAGES};
use types::native_mod::NativeModData;
use shared_dx::util::*;

/// A snapshot of the currently bound textures used by `select` to filter
/// candidate mods that carry texture-checksum constraints.  Keys for
/// `checksums` are resource pointers (DX9 `IDirect3DBaseTexture9*`, DX11
/// `ID3D11Texture2D*`).  When `checksums` is `None` or a particular bound
/// pointer has no entry, we treat that slot as "checksum unknown", which
/// fails any constraint on that slot.
pub struct BoundTextures<'a> {
    pub per_stage: [usize; MAX_CHECKSUM_STAGES],
    pub checksums: Option<&'a FnvHashMap<usize, u32>>,
}

impl<'a> BoundTextures<'a> {
    pub fn empty() -> Self {
        Self { per_stage: [0usize; MAX_CHECKSUM_STAGES], checksums: None }
    }
}

/// True iff the mod either has no texture-checksum constraint, or every
/// slot it constrains is satisfied by the currently bound textures.
fn checksum_constraint_matches(nmod: &NativeModData, bound: &BoundTextures) -> bool {
    let mask = nmod.mod_data.tex_checksum_mask;
    if mask == 0 {
        return true;
    }
    for i in 0..MAX_CHECKSUM_STAGES {
        if (mask >> i) & 1 == 0 {
            continue;
        }
        let expected = nmod.mod_data.tex_checksums[i];
        let ptr = bound.per_stage[i];
        let actual = bound
            .checksums
            .and_then(|m| m.get(&ptr).copied());
        match actual {
            Some(a) if a == expected => {}
            _ => return false,
        }
    }
    true
}

#[inline]
fn has_checksum_constraint(nmod: &NativeModData) -> bool {
    nmod.mod_data.tex_checksum_mask != 0
}

fn find_parent<'a>(name:&str, mvec:&'a mut Vec<NativeModData>) -> Option<&'a mut NativeModData> {
    for p in mvec.iter_mut() {
        if p.name == name {
            return Some(p)
        }
    }
    return None
}

/// Run a function on each parent mod of `nmod`.  May run it zero times if there are no parents.
fn iter_parent_mods<'a, F>(nmod:&NativeModData, mstate: &'a LoadedModState, f:&mut F) -> ()
where F: FnMut(&'a NativeModData) -> ()
{
    if nmod.parent_mod_names.is_empty() {
        return;
    }
    nmod.parent_mod_names.iter().for_each(|pmod| {
        mstate.mods_by_name.get(pmod)
            .and_then(|parmodkey| mstate.mods.get(parmodkey))
            .and_then(|parent_mods| {
                parent_mods.iter().find(|p| p.name == *pmod)
            }).map(|pmod| {
                f(pmod)
            });
    });
}

/// Return a vector of references to any parent mods that the target mod has, or an empty vec
/// if there are none.  Caller should check `nmod.parent_mod_names.is_empty()` before calling this
/// to avoid an unnecessary vector allocation in the empty case.
/// If you just want to run a function on each parent mod, use iter_parent_mods instead, which
/// avoids allocating any vecs.
fn lookup_parent_mods<'a>(nmod:&NativeModData, mstate: &'a LoadedModState) -> Vec<&'a NativeModData> {
    let mut res = vec![];
    if nmod.parent_mod_names.is_empty() {
        return res;
    }
    iter_parent_mods(nmod, mstate, &mut |pmod| {
        res.push(pmod);
    });
    res
}

#[macro_export]
macro_rules! debug_spam {
    ($v:expr) => {

        if (crate::ENABLE_DEBUG_SPAM) {
            if (crate::DEBUG_SPAM_TO_STDERR) {
                eprintln!("{}", $v())
            } else {
                write_log_file(&$v());
            }
        }
    };
}

#[inline(always)]
/// Returns true if a mod is available that matches the given primitive and vertex counts.
/// This is the first part of the work done by `select` below, and is intended to speed up
/// hot paths (since this check is small and can be inlined).
pub fn preselect(mstate: &mut LoadedModState, prim_count:u32, vert_count:u32) -> bool {
    let mod_key = NativeModData::mod_key(vert_count, prim_count);
    mstate.mods.get(&mod_key).is_some()
}

/// Return values for `select` below; `as_slice` can be used on this to handle them the same way.
/// For vast majority of mods especially older mods this will be One.  Some newer mods use 2 or more 
/// for the same ref, the Many case is used for those. 
/// In the Many case, one mod has no parent (and thus is the primary mod or variant), the other(s) use the first 
/// as the parent so that they only render when parent is active.  This is the only way to get a 
/// (aggregate) mod that has two different materials/textures for the same ref,
/// since the mmobj does not support that.
pub enum SelectedMod<'a> {
    One(&'a NativeModData),
    Many(Vec<&'a NativeModData>)
}

impl<'a> SelectedMod<'a> {
    pub fn as_slice(&self) -> &[&'a NativeModData] {
        match self {
            SelectedMod::One(item)   => std::slice::from_ref(item),
            SelectedMod::Many(list)  => list.as_slice(),
        }
    }
}

impl<'a> std::fmt::Debug for SelectedMod<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectedMod::One(mod_data) => {
                write!(f, "SelectedMod::One {{ name: {} }}", mod_data.name)
            },
            SelectedMod::Many(mod_list) => {
                write!(
                    f,
                    "SelectedMod::Many [ {} ]",
                    mod_list
                        .iter()
                        .enumerate()
                        .map(|(index, mod_data)| format!("{{ index: {}, name: {} }}", index, mod_data.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        }
    }
}

/// Select a mod for rendering, if any.
///
/// The mod state is &mut because we may need to update the last frame rendered for any
/// parent mods we find.
///
/// Perf note: since checking for a mod is needed for everything drawn by the game, it is better to
/// call `preselect` first to determine if this function even needs to be called.  `select` does
/// early out as soon as it knows there is no mod, but still incurs a bit of extra cost.
pub fn select<'a>(mstate: &'a mut LoadedModState, prim_count:u32, vert_count:u32, current_frame_num:u64, bound: &BoundTextures) -> Option<SelectedMod<'a>> {
    let mod_key = NativeModData::mod_key(vert_count, prim_count);
    let r = mstate.mods.get(&mod_key);
    // just get out of here if we didn't have a match
    r?;

    // Compute which mod indices are allowed by texture-checksum constraints.
    // Priority: if any constrained mod matches the currently bound textures,
    // restrict selection to the matching constrained mods. Otherwise fall
    // back to unconstrained mods (mods with `tex_checksum_mask == 0`). A
    // constrained mod whose constraint doesn't match is never selected.
    let (allowed, num_allowed): (Vec<bool>, usize) = {
        let nmods = r.unwrap();
        let mut has_match = false;
        let mut cm: Vec<bool> = Vec::with_capacity(nmods.len());
        for nmod in nmods.iter() {
            let matched = has_checksum_constraint(nmod)
                && checksum_constraint_matches(nmod, bound);
            if matched { has_match = true; }
            cm.push(matched);
        }
        let v: Vec<bool> = if has_match {
            cm
        } else {
            nmods.iter().map(|nmod| !has_checksum_constraint(nmod)).collect()
        };
        let n = v.iter().filter(|&&b| b).count();
        (v, n)
    };
    if num_allowed == 0 {
        debug_spam!(|| format!("no allowed mods after checksum filter for {}p/{}v",
            prim_count, vert_count));
        return None;
    }

    // found at least one mod.  do some more checks to see if each has a parent, and if the parent
    // is active.  count the active parents we find because if more than one is active,
    // we have ambiguity and can't render any of them.
    // Initialize target to the first allowed index so the n==1 and default-fallback
    // paths refer to an allowed mod rather than nmods[0].
    let mut target_mod_index:usize = allowed.iter().position(|&b| b).unwrap_or(0);
    let mut parent_in_mod_list = false;
    let r2 = r.and_then(|nmods| {
        let mut num_active_parents = 0;
        let num_mods = num_allowed;
        let mut observed_noparent_mods: HashMap<String,usize> = HashMap::new(); // aka the top level variants

        debug_spam!(|| format!("checking {} allowed mods (of {}) for {}p/{}v",
            num_mods, nmods.len(), prim_count, vert_count));
        for (midx,nmod) in nmods.iter().enumerate() {
            if !allowed[midx] { continue; }
            if nmod.parent_mod_names.is_empty() {
                if num_mods > 1 {
                    observed_noparent_mods.insert(nmod.name.clone(), midx);
                }
                debug_spam!(|| format!("no parents for {} (num mods {})", nmod.name, num_mods));
                continue;
            }
            debug_spam!(|| format!("check parents for {} (nummods: {}, parents: {:?})", nmod.name, num_mods, nmod.parent_mod_names));

            iter_parent_mods(nmod, mstate, &mut |parent:&NativeModData| {
                if parent.recently_rendered(current_frame_num) {
                    if num_mods > 1 {
                        if let Some(pidx) = observed_noparent_mods.get(&parent.name) {
                            // the parent is in this mod list, set target_mod_idx to it
                            target_mod_index = *pidx;
                            parent_in_mod_list = true;
                        }
                    }

                    if !parent_in_mod_list {
                        // parent not in this list so this child mod is the one we want to render
                        target_mod_index = midx;
                    }

                    num_active_parents += 1;
                    debug_spam!(|| format!(" par {} of mod {} is active, num active: {}", parent.name, nmod.name, num_active_parents));
                } else {
                    debug_spam!(|| format!(" par {} is not active (mod {})", parent.name, nmod.name));
                }
            });
        }

        // return Some(()) if we found a valid one.
        match num_mods {
            0 => None,

            // multiple mods but only one parent, and the parent is outside of this list, so this is a
            // child mod of an active parent with a different ref. that
            // takes precedence over whatever other variants are here.
            n if n > 1 && num_active_parents == 1 && !parent_in_mod_list => {
                debug_spam!(|| format!("rend mod {} because just one active parent named '{}' and parent outside this list",
                    nmods[target_mod_index].name, "unknown"));
                Some(())
            },
            // just one mod (allowed), it doesn't have a parent, or if it does and there is just one parent
            n if n == 1 && (nmods[target_mod_index].parent_mod_names.is_empty() || num_active_parents == 1) => {
                Some(())
            },
            // more than one mod, 0 or >1 active parents, so if we have a selected variant
            // index, use that index
            n if n > 1 => { //&& mstate.selected_variant.contains_key(&mod_key)
                let tmic = target_mod_index;
                let sel_index = mstate.selected_variant.get(&mod_key).unwrap_or(&tmic);
                debug_spam!(|| format!("var sel index: {}, max: {}", sel_index, n));
                if *sel_index < nmods.len() && allowed.get(*sel_index).copied().unwrap_or(false) {
                    // currently child mods can't be variants - this avoids messy cases with
                    // one or more children whose parents may or may not have rendered recently.
                    nmods.get(*sel_index).and_then(|nmod| {
                        if !nmod.parent_mod_names.is_empty() {
                            None
                        } else {
                            target_mod_index = *sel_index;
                            Some(())
                        }
                    })
                } else {
                    None
                }
            }
            _ => None
        }
    });
    // return if we aren't rendering it.
    r2?;

    // ok, we're rendering it, so need to update last render frame on it,
    // which requires a mutable reference.  we couldn't use a
    // mutable ref earlier, because we had to do two simultaneous lookups on the hash table.
    // so we have to refetch as mutable, set the frame value and then (for safety)
    // refetch as immutable again so that we can pass that value on.  that's three
    // hash lookups guaranteed but fortunately we're only doing this for active mods.

    // second pass (mut borrow)
    // - grab the variant’s name
    // - walk the same list and bump `last_frame_render` on
    // the variant and every mod that names it as a parent
    let mut num_selected = 0;

    let variant_name = {
        if let Some(nmods_mut) = mstate
            .mods
            .get_mut(&mod_key) {

            // gpt-o3 says not to worry about this allocation, and its a pain to do it any other way
            // due to BC ^_^
            let vname = nmods_mut[target_mod_index].name.clone();

            for nmod in nmods_mut.iter_mut() {
                if nmod.name == vname
                    || nmod
                        .parent_mod_names
                        .iter()
                        .any(|p| p == &vname)
                {
                    nmod.last_frame_render = current_frame_num;
                    num_selected += 1;
                }
            }
            vname
        } else {
            String::new()
        }
    }; // mutable borrow ends here

    // now determine the final selection result, which is usually just one mod
    let selection = if let Some(nmods) = mstate.mods.get(&mod_key) {
        // special case the most common result to avoid another linear search and vec allocation
        if num_selected == 1 {
            debug_spam!(|| format!("returning one mod (variant: {})", variant_name));
            Some(SelectedMod::One(&nmods[target_mod_index]))
        } else {

            let vec:Vec<&NativeModData> = nmods
                .iter()
                .filter(|m| {
                    m.name == variant_name
                        || m.parent_mod_names
                            .iter()
                            .any(|p| p == &variant_name)
                })
                .collect();
            debug_spam!(|| format!("returning {} mods (orig: {}) (variant: {})", vec.len(), num_selected, variant_name));
            Some(SelectedMod::Many(vec))
        }
    } else {
        None
    };
    selection
}

pub fn select_next_variant(mstate: &mut LoadedModState, lastframe:u64) {
    for (mkey, nmdv) in mstate.mods.iter() {
        if nmdv.len() <= 1 {
            // most mods have no variants
            continue;
        }

        // don't change the selection if none have been rendered recently
        let foundrecent = nmdv.iter().find(|nmd| nmd.recently_rendered(lastframe));
        if foundrecent.is_none() {
            continue;
        }

        // get the current variant for this mod
        let sel_index_entry = mstate.selected_variant.entry(*mkey).or_insert(0);
        let mut sel_index = *sel_index_entry;
        let start = sel_index;
        // select next, skipping over child mods.  stop if we wrap to where we started
        sel_index += 1;
        loop {
            if sel_index >= nmdv.len() {
                sel_index = 0;
            }
            if sel_index == start {
                break;
            }
            if nmdv[sel_index].parent_mod_names.is_empty() {
                // found one
                write_log_file(&format!("selected next variant: {} => {}", nmdv[sel_index].name, sel_index));
                *sel_index_entry = sel_index;
                break;
            }
            // keep looking
            sel_index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_state::new_fnv_map;
    use global_state::{LoadedModState,LoadedModsMap,ModsByNameMap};
    use mod_load::sort_mods;
    use types::native_mod::{NativeModData, MAX_RECENT_RENDER_PARENT_THRESH};

    fn new_mod(name:&str, prims:i32, verts:i32) -> NativeModData {
        let mut m = NativeModData::new();
        m.mod_data.numbers.prim_count = prims;
        m.mod_data.numbers.vert_count = verts;
        m.name = name.to_owned();
        m
    }
    fn add_mod(mmap:&mut LoadedModsMap, nmod:NativeModData) {
        let mk = NativeModData::mod_key(
            nmod.mod_data.numbers.vert_count as u32,
            nmod.mod_data.numbers.prim_count as u32);

        let mvec = mmap.entry(mk).or_insert_with(|| vec![]);
        mvec.push(nmod);
        sort_mods(mmap);
    }
    fn new_state(mmap:LoadedModsMap) -> LoadedModState {
        let mut mods_by_name:ModsByNameMap  = new_fnv_map(mmap.len());
        use std::collections::HashSet;
        let mut parent_mods:HashSet<String> = HashSet::new();

        let mut mmap = mmap;
        for (mk,nmdv) in mmap.iter_mut() {
            for nmod in nmdv.iter_mut() {
                // by convention mod names in internal structures are lowercased
                nmod.name = nmod.name.to_lowercase();
                mods_by_name.insert(nmod.name.to_owned(), *mk);
                nmod.parent_mod_names.iter_mut().for_each(|pmod| {
                    *pmod = pmod.to_lowercase();
                    parent_mods.insert(pmod.to_owned());
                });
            }
        }
        // mark parents
        for parent in parent_mods {
            let pmk = mods_by_name.get(&parent).expect(&format!("no parent: {}", parent));
            let pmods = mmap.get_mut(&pmk).expect("no parent mods");
            let pmod = find_parent(&parent, pmods).expect("no parent");
            pmod.is_parent = true;
        }
        LoadedModState {
            mods: mmap,
            mods_by_name: mods_by_name,
            selected_variant: global_state::new_fnv_map(16),
        }
    }

    fn get_parent<'a>(mstate:&'a mut LoadedModState, pname:&str) -> &'a mut NativeModData {
        let pname = pname.to_lowercase();
        let pkey = mstate.mods_by_name.get(&pname.to_owned()).expect(&format!("no parent: {}", pname));
        let pmods = mstate.mods.get_mut(pkey).expect("no parent");
        let pmod = find_parent(&pname, pmods).expect("no parent");
        pmod
    }
    #[test]
    fn test_select_basic() {
        let mut modmap:LoadedModsMap = new_fnv_map(10);

        add_mod(&mut modmap, new_mod("Mod1", 100, 200));
        add_mod(&mut modmap, new_mod("Mod2", 101, 201));
        let mut mstate = new_state(modmap);
        let r = select(&mut mstate, 99, 100, 1, &BoundTextures::empty());
        assert!(r.is_none());
        let r = select(&mut mstate, 100, 202, 1, &BoundTextures::empty());
        assert!(r.is_none());
        let r = select(&mut mstate, 100, 200, 1, &BoundTextures::empty());
        match r.expect("no mod found") {
            SelectedMod::One(mod_data) => assert_eq!(mod_data.name, "mod1".to_string()),
            _ => panic!("Expected SelectedMod::One"),
        }
        let r = select(&mut mstate, 101, 201, 1, &BoundTextures::empty());
        match r.expect("no mod found") {
            SelectedMod::One(mod_data) => assert_eq!(mod_data.name, "mod2".to_string()),
            _ => panic!("Expected SelectedMod::One"),
        }
    }

    #[test]
    fn test_select_parent() {
        let mut modmap:LoadedModsMap = new_fnv_map(10);

        // add two parents.  Note the parents have different geometry since we aren't
        // testing variations here (and if they had the same geometry,
        // both would be eligible for render, which is an error)
        add_mod(&mut modmap, new_mod("Mod1P", 100, 200));
        add_mod(&mut modmap, new_mod("Mod4P", 99, 200));
        let mut child = new_mod("Mod2C", 101, 201);
        child.parent_mod_names.push("Mod1P".to_string());
        add_mod(&mut modmap, child);
        // add another child for a different parent
        let mut child = new_mod("Mod3C", 101, 201);
        child.parent_mod_names.push("Mod4P".to_string());
        add_mod(&mut modmap, child);
        let mut mstate = new_state(modmap);
        // when both parents have rendered recently, trying to select either child will be None
        // since both are eligible and we can't pick one
        // (note, variations doesn't apply here, because variations
        // should select root parent mods, not children).
        let r = select(&mut mstate, 101, 201, 1, &BoundTextures::empty());
        assert!(r.is_none());
        // update so that we have just one recent parent
        let pmod = get_parent(&mut mstate, "Mod1P");
        let frame = MAX_RECENT_RENDER_PARENT_THRESH + 10; // make sure all mods are out of recent window
        pmod.last_frame_render = frame;
        // trying to select child when one parent has rendered recently should find it
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "mod2c");
        // and should not when parent hasn't been rendered
        let frame = frame + MAX_RECENT_RENDER_PARENT_THRESH + 10; // make sure all mods are out of recent window
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        assert!(r.is_none());
        // when a parent is rendered, its frame should update
        let r = select(&mut mstate, 100, 200, frame+60, &BoundTextures::empty());
        match r {
            Some(SelectedMod::One(nmod)) => {
                assert_eq!(nmod.name, "mod1p".to_string());
                assert_eq!(nmod.last_frame_render, frame+60);
            },
            _ => panic!("unexpected result failed")
        }
    }

    #[test]
    fn test_exact_parent() {
        // when there are multiple variants with the same mesh params as a mod parent,
        // the child should only render if that parent is active,
        // not if some other random mod with the same params is active.
        let mut modmap:LoadedModsMap = new_fnv_map(10);
        add_mod(&mut modmap, new_mod("Mod1P", 100, 200));
        add_mod(&mut modmap, new_mod("Mod4P", 100, 200));
        let mut child = new_mod("ModC", 101, 201);
        child.parent_mod_names.push("Mod4P".to_string());
        add_mod(&mut modmap, child);

        let mut mstate = new_state(modmap);
        // Make Mod1P active recently, which should not matter for ModC because it isn't
        // ModC's parent.
        let pmod = get_parent(&mut mstate, "Mod1P");
        let frame = MAX_RECENT_RENDER_PARENT_THRESH + 10; // make sure all mods are out of recent window
        pmod.last_frame_render = frame;
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        assert!(r.is_none());
        // and if we update our parent, we should be selected now
        let pmod = get_parent(&mut mstate, "Mod4P");
        pmod.last_frame_render = frame;
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        match r.expect("no mod found") {
            SelectedMod::One(mod_data) => assert_eq!(mod_data.name, "modc".to_string()),
            _ => panic!("Expected SelectedMod::One"),
        }
    }

    #[test]
    fn test_multi_parent() {
        // if we have two parents, we should render if one or the other is recently rendered.
        // but not if both are.  technically we could render if both are active but this might
        // obscure problems in how the parents are set up, which may cause problems later.  so
        // hide it.

        // mix cases in some of the names to test case insensitivity
        let mut modmap:LoadedModsMap = new_fnv_map(10);
        add_mod(&mut modmap, new_mod("Mod1P", 100, 200));
        add_mod(&mut modmap, new_mod("mod4P", 100, 200));
        let mut child = new_mod("ModC", 101, 201);
        child.parent_mod_names.push("Mod4P".to_string());
        child.parent_mod_names.push("Mod1P".to_string());
        add_mod(&mut modmap, child);
        let mut mstate = new_state(modmap);
        // both recent = no child render.  since they are new mods their last recent frame is zero
        // (which is a bit ugly, actually it should be an option with None)
        let r = select(&mut mstate, 101, 201, 0, &BoundTextures::empty());
        assert!(r.is_none());
        let pmod = get_parent(&mut mstate, "Mod4p");
        // advance frame to put all mods out of recent window except this one
        let frame = MAX_RECENT_RENDER_PARENT_THRESH + 10;
        pmod.last_frame_render = frame;
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        match r.expect("no mod found") {
            SelectedMod::One(mod_data) => assert_eq!(mod_data.name, "modc".to_string()),
            _ => panic!("Expected SelectedMod::One"),
        }
        let pmod = get_parent(&mut mstate, "Mod1P");
        let frame = frame + MAX_RECENT_RENDER_PARENT_THRESH + 10;
        pmod.last_frame_render = frame;
        let r = select(&mut mstate, 101, 201, frame, &BoundTextures::empty());
        match r.expect("no mod found") {
            SelectedMod::One(mod_data) => assert_eq!(mod_data.name, "modc".to_string()),
            _ => panic!("Expected SelectedMod::One"),
        }
    }

    fn assert_selected_mod_name(selected: Option<SelectedMod>, expected_name: &str) {
        match selected {
            Some(SelectedMod::One(mod_data)) => {
                assert_eq!(mod_data.name, expected_name.to_string());
            },
            x => panic!("Expected SelectedMod::One with name: {}; got: {:?}", expected_name, x),
        }
    }

    #[test]
    fn variants() {
        let mut modmap:LoadedModsMap = new_fnv_map(10);
        add_mod(&mut modmap, new_mod("Mod1", 100, 200)); // variant in this ref
        add_mod(&mut modmap, new_mod("Mod2", 100, 200)); // variant in this ref
        add_mod(&mut modmap, new_mod("ModP", 101, 201)); // variant in another ref
        let mut child = new_mod("ModC", 100, 200); // child in this ref
        child.parent_mod_names.push("ModP".to_string());
        add_mod(&mut modmap, child);
        let mut mstate = new_state(modmap);
        // selecting 100/200 mod should return the ModC because its parent is active - the other
        // two have no parent and so are lower priority, so we exclude them.
        let r = select(&mut mstate, 100, 200, 0, &BoundTextures::empty());

        assert_selected_mod_name(r, "modc");
        // now select with a more recent frame to exclude the parent, this should return the first
        // mod, because we haven't selected a variant yet, so the default is the first
        let frame = MAX_RECENT_RENDER_PARENT_THRESH + 10;
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        //assert!(r.is_none(), "unexpected mod: {:?}", r.unwrap().name);
        assert_selected_mod_name(r, "mod1");
        // now pick a variant.  the indexes will be the same as the mod insertion order.
        let mk = NativeModData::mod_key(200, 100);
        mstate.selected_variant.insert(mk, 0);
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "mod1");
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 1;
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "mod2");
        // select() should not return a selected child
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 2;
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert!(r.is_none(), "unexpected mod");
        // select() should not puke if selected child is out of range
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 3;
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert!(r.is_none(), "unexpected mod");
    }

    #[test]
    fn test_variant_cycling() {
        //
        // If this test fails, it may be helpful for debugging to turn on ENABLE_DEBUG_SPAM and DEBUG_SPAM_TO_STDERR in lib.rs
        //

        let mut modmap:LoadedModsMap = new_fnv_map(10);

        // Add two variants for prim/vert count A (100,200)
        add_mod(&mut modmap, new_mod("Variant1", 100, 200));
        add_mod(&mut modmap, new_mod("Variant2", 100, 200));

        // Add a parent mod with a different prim/vert count B (101,201)
        add_mod(&mut modmap, new_mod("ParentB", 101, 201));

        // Add a child mod for B but with prim/vert count A, making it non-variant.
        let mut child = new_mod("Child", 100, 200);
        child.parent_mod_names.push("ParentB".to_string());
        add_mod(&mut modmap, child);

        // add another child of the (upcoming) Variant3, which will be in this ref (100,200), which means when that variant is selected
        // both it and this child should be returned by select
        let mut child = new_mod("ChildOfV3", 100, 200);
        child.parent_mod_names.push("Variant3".to_string());
        add_mod(&mut modmap, child);

        // Add another variant (no parent) for prim/vert count A. because of the mod re-sort 
        // after adding, this should end up after the two previous variants in the 
        // variant cycle list, despite the fact that we just added children with parents, 
        // (which will get sorted to be later in the list)
        add_mod(&mut modmap, new_mod("Variant3", 100, 200));

        let mut mstate = new_state(modmap);
        let frame = MAX_RECENT_RENDER_PARENT_THRESH + 10;

        // Cycle through variants by updating the selected_variant index.
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "variant1");
        select_next_variant(&mut mstate, frame);
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "variant2");
        select_next_variant(&mut mstate, frame);
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        match r {
            None => panic!("expected return for variant 3 case, got none"),
            Some(SelectedMod::One(_)) => panic!("expected many but got: {:?}", r),
            Some(SelectedMod::Many(list)) => {
                let names:Vec<String> = list.as_slice().iter().map(|nmd| nmd.name.to_string()).collect();
                assert_eq!(names, vec!["variant3", "childofv3"])
            }
        }
        // should wrap around
        select_next_variant(&mut mstate, frame);
        let r = select(&mut mstate, 100, 200, frame, &BoundTextures::empty());
        assert_selected_mod_name(r, "variant1");
    }

    fn new_constrained_mod(name:&str, prims:i32, verts:i32, slot:usize, crc:u32) -> NativeModData {
        let mut m = new_mod(name, prims, verts);
        m.mod_data.tex_checksums[slot] = crc;
        m.mod_data.tex_checksum_mask |= 1u8 << slot;
        m
    }

    /// Build a BoundTextures with a single bound texture on one stage, using
    /// a synthetic pointer and a provided checksum map.
    fn bound_with<'a>(stage:usize, ptr:usize, cmap:&'a FnvHashMap<usize,u32>) -> BoundTextures<'a> {
        let mut per_stage = [0usize; MAX_CHECKSUM_STAGES];
        per_stage[stage] = ptr;
        BoundTextures { per_stage, checksums: Some(cmap) }
    }

    #[test]
    fn test_checksum_constrained_wins() {
        // Two mods with same prim/vert, each constrained to a different
        // texture checksum on stage 0. Binding texture A should pick modA,
        // binding texture B should pick modB, binding a texture with no
        // known checksum should pick neither.
        let mut modmap:LoadedModsMap = new_fnv_map(4);
        add_mod(&mut modmap, new_constrained_mod("ModA", 100, 200, 0, 0xAAAA_1111));
        add_mod(&mut modmap, new_constrained_mod("ModB", 100, 200, 0, 0xBBBB_2222));
        let mut mstate = new_state(modmap);

        let mut cmap: FnvHashMap<usize,u32> = global_state::new_fnv_map(4);
        cmap.insert(0xAAAA_0000usize, 0xAAAA_1111);
        cmap.insert(0xBBBB_0000usize, 0xBBBB_2222);

        let bound_a = bound_with(0, 0xAAAA_0000usize, &cmap);
        let r = select(&mut mstate, 100, 200, 1, &bound_a);
        assert_selected_mod_name(r, "moda");

        let bound_b = bound_with(0, 0xBBBB_0000usize, &cmap);
        let r = select(&mut mstate, 100, 200, 2, &bound_b);
        assert_selected_mod_name(r, "modb");

        // Bind a texture whose checksum isn't recorded for any candidate.
        let unknown = bound_with(0, 0xCCCC_0000usize, &cmap);
        let r = select(&mut mstate, 100, 200, 3, &unknown);
        assert!(r.is_none(), "expected no mod for unknown bound texture");
    }

    #[test]
    fn test_checksum_fallback_to_unconstrained() {
        // Same prim/vert has one constrained mod and one unconstrained.  If
        // the constrained mod matches the bound texture, it wins.  If not,
        // fall through to the unconstrained one.
        let mut modmap:LoadedModsMap = new_fnv_map(4);
        add_mod(&mut modmap, new_constrained_mod("ModCons", 100, 200, 0, 0xDEAD_BEEF));
        add_mod(&mut modmap, new_mod("ModUncons", 100, 200));
        let mut mstate = new_state(modmap);

        let mut cmap: FnvHashMap<usize,u32> = global_state::new_fnv_map(4);
        cmap.insert(0x1000usize, 0xDEAD_BEEF);

        // Bound texture matches the constrained mod -> constrained wins.
        let matching = bound_with(0, 0x1000usize, &cmap);
        let r = select(&mut mstate, 100, 200, 1, &matching);
        assert_selected_mod_name(r, "modcons");

        // Bound texture doesn't match any constrained mod -> fall back to
        // unconstrained.  Note: the variant cycle for unconstrained needs
        // variant selection, which isn't set; by design the first allowed
        // (which is ModUncons) is used and it has no parent so it renders.
        let nonmatching = bound_with(0, 0x9999usize, &cmap);
        let r = select(&mut mstate, 100, 200, 2, &nonmatching);
        assert_selected_mod_name(r, "moduncons");

        // Empty bound state also falls back to unconstrained.
        let r = select(&mut mstate, 100, 200, 3, &BoundTextures::empty());
        assert_selected_mod_name(r, "moduncons");
    }

    #[test]
    fn test_checksum_multi_stage_requires_all() {
        // A single mod constrained on stages 0 AND 2: both must match to
        // select.  Partial match is rejected (falls through, but with no
        // unconstrained fallback, yields None).
        let mut modmap:LoadedModsMap = new_fnv_map(4);
        let mut m = new_mod("ModMulti", 100, 200);
        m.mod_data.tex_checksums[0] = 0x1111_1111;
        m.mod_data.tex_checksums[2] = 0x3333_3333;
        m.mod_data.tex_checksum_mask = 0b0000_0101;
        add_mod(&mut modmap, m);
        let mut mstate = new_state(modmap);

        let mut cmap: FnvHashMap<usize,u32> = global_state::new_fnv_map(4);
        cmap.insert(0xA0usize, 0x1111_1111);
        cmap.insert(0xC0usize, 0x3333_3333);
        cmap.insert(0xBADusize, 0xDEAD_DEAD);

        // Full match
        let mut per_stage = [0usize; MAX_CHECKSUM_STAGES];
        per_stage[0] = 0xA0;
        per_stage[2] = 0xC0;
        let bound = BoundTextures { per_stage, checksums: Some(&cmap) };
        let r = select(&mut mstate, 100, 200, 1, &bound);
        assert_selected_mod_name(r, "modmulti");

        // Only stage 0 matches - should not select
        let mut per_stage = [0usize; MAX_CHECKSUM_STAGES];
        per_stage[0] = 0xA0;
        per_stage[2] = 0xBAD;
        let bound = BoundTextures { per_stage, checksums: Some(&cmap) };
        let r = select(&mut mstate, 100, 200, 2, &bound);
        assert!(r.is_none(), "partial checksum match should not select");

        // Only stage 2 matches - should not select
        let mut per_stage = [0usize; MAX_CHECKSUM_STAGES];
        per_stage[0] = 0xBAD;
        per_stage[2] = 0xC0;
        let bound = BoundTextures { per_stage, checksums: Some(&cmap) };
        let r = select(&mut mstate, 100, 200, 3, &bound);
        assert!(r.is_none(), "partial checksum match should not select");
    }

    #[test]
    fn test_checksum_missing_map_fails_constraint() {
        // A constrained mod with no checksum map available (e.g., DX9 non-
        // lockable pool never checksummed the texture) should not match;
        // should fall through to unconstrained if one exists.
        let mut modmap:LoadedModsMap = new_fnv_map(4);
        add_mod(&mut modmap, new_constrained_mod("ModCons", 100, 200, 0, 0xDEAD_BEEF));
        add_mod(&mut modmap, new_mod("ModUncons", 100, 200));
        let mut mstate = new_state(modmap);

        // BoundTextures::empty() has checksums=None, so any constraint fails.
        let r = select(&mut mstate, 100, 200, 1, &BoundTextures::empty());
        assert_selected_mod_name(r, "moduncons");
    }

    #[test]
    fn test_checksum_all_constrained_no_match_yields_none() {
        // All candidates are constrained and none match -> no fallback
        // available -> None.
        let mut modmap:LoadedModsMap = new_fnv_map(4);
        add_mod(&mut modmap, new_constrained_mod("ModA", 100, 200, 0, 0xAAAA));
        add_mod(&mut modmap, new_constrained_mod("ModB", 100, 200, 0, 0xBBBB));
        let mut mstate = new_state(modmap);

        let mut cmap: FnvHashMap<usize,u32> = global_state::new_fnv_map(4);
        cmap.insert(0x10usize, 0xCCCC);
        let bound = bound_with(0, 0x10usize, &cmap);
        let r = select(&mut mstate, 100, 200, 1, &bound);
        assert!(r.is_none(), "no constrained match and no unconstrained fallback => None");
    }

    #[test]
    fn uniq_keys() {
        // slow test to make sure the modkey hash doesn't have obvious, bad collisions
        use std::collections::HashMap;

        let mut seen_keys:HashMap<u32, (i32,i32)> = HashMap::new();

        for prim in 0..1000 {
            for vert in 0..1000 {
                let mk = NativeModData::mod_key(
                    vert as u32,
                    prim as u32);
                if seen_keys.contains_key(&mk) {
                    let existing = seen_keys.get(&mk).unwrap();
                    panic!("key for {} already exists; curr {}p,{}v, existing: {:?}", mk, prim, vert, existing);
                }
                seen_keys.insert(mk, (prim,vert));
            }
        }
    }
}