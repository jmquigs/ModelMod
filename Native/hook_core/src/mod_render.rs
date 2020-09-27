use global_state::LoadedModState;
use types::native_mod::NativeModData;
use shared_dx9::util::*;

fn find_parent<'a>(name:&str, mvec:&'a mut Vec<NativeModData>) -> Option<&'a mut NativeModData> {
    for p in mvec.iter_mut() {
        if p.name == name {
            return Some(p)
        }
    }
    return None
}

fn lookup_parent_mod<'a>(nmod:&NativeModData, mstate: &'a LoadedModState) -> Option<&'a NativeModData> {
    mstate.mods_by_name.get(&nmod.parent_mod_name)
    .and_then(|parmodkey| mstate.mods.get(parmodkey))
    .and_then(|parent_mods| {
        parent_mods.iter().find(|p| p.name == nmod.parent_mod_name)
    })
}

/// Select a mod for rendering, if any.
///
/// The mod state is &mut because we may need to update the last frame rendered for any 
/// parent mods we find.
///
/// Perf note: the first part of this function is very hot and will be called for literally 
/// everything drawn by the game.  So its important to get out of here early if there is no match.
/// This could check could even be inlined as a separate function, but hopefully the call 
/// doesn't add much overhead (if I did profile optimization, llvm could maybe split this into 
/// hot/cold parts)
pub fn select(mstate: &mut LoadedModState, prim_count:u32, vert_count:u32, current_frame_num:u64) -> Option<&NativeModData> {
    let mod_key = NativeModData::mod_key(vert_count, prim_count);
    let r = mstate.mods.get(&mod_key);
    // just get out of here if we didn't have a match
    if r.is_none() {
        return None;
    }
    // found at least one mod.  do some more checks to see if each has a parent, and if the parent
    // is active.  count the active parents we find because if more than one is active,
    // we have ambiguity and can't render any of them.
    let mut target_mod_index:usize = 0;
    let mut active_parent_name:&str = "";
    let r2 = r.and_then(|nmods| {
        let mut num_active_parents = 0;
        let num_mods = nmods.len();
        for (midx,nmod) in nmods.iter().enumerate() {
            if !nmod.parent_mod_name.is_empty() {
                mstate.mods_by_name.get(&nmod.parent_mod_name)
                    .and_then(|parmodkey| mstate.mods.get(parmodkey))
                    .map(|parent_mods| {
                        // count any active parents
                        for parent_mod in parent_mods.iter() {
                            if num_active_parents > 1 {
                                // fail, ambiguity
                                break;
                            }
                            if parent_mod.recently_rendered(current_frame_num) {
                                // parent is active
                                num_active_parents += 1;

                                // if this parent is for the mod we are looking at,
                                // remember that mod index.  note that we'll slam this if we
                                // have multiple active parents for multiple mods,
                                // but we are screwed anyway in that case.
                                if nmod.parent_mod_name == parent_mod.name {
                                    active_parent_name = &parent_mod.name;
                                    target_mod_index = midx;
                                }
                            }
                        }
                    });
            }
        }
        
        // return Some(()) if we found a valid one.
        match num_mods {
            0 => None, 
            // multiple mods but only one parent
            n if n > 1 && num_active_parents == 1 => {
                // write_log_file(&format!("rend mod {} because just one active parent named '{}'",
                //     nmods[target_mod_index].name, active_parent_name));
                Some(())
            },
            // just one mod it doesn't have a parent, or if it does and there is just one parent
            n if n == 1 && (nmods[0].parent_mod_name.is_empty() || num_active_parents == 1) => {
                // write_log_file(&format!("rend mod {} because just one mod with parname '{}' or {} parents",
                // nmods[target_mod_index].name, nmods[0].parent_mod_name, num_active_parents));
                Some(())
            },
            // more than one mod, 0 or >1 active parents, so if we have a selected variant
            // index, use that index
            n if n > 1 => { //&& mstate.selected_variant.contains_key(&mod_key) 
                let tmic = target_mod_index;
                let sel_index = mstate.selected_variant.get(&mod_key).unwrap_or(&tmic);
                if *sel_index < n {
                    // currently child mods can't be variants - this avoids messy cases with 
                    // one or more children whose parents may or may not have rendered recently.
                    nmods.get(*sel_index).and_then(|nmod| {
                        if !nmod.parent_mod_name.is_empty() {
                            None
                        } else {
                            target_mod_index = *sel_index;
                            Some(())
                        }
                        //
                        // let p = lookup_parent_mod(nmod, mstate);
                        // match p {
                        //     Some(p) if !p.recently_rendered(current_frame_num) => {
                        //         None
                        //     },
                        //     _ => {
                        //         target_mod_index = *sel_index;
                        //         Some(())
                        //     },
                        // }
                    })
                } else {
                    None
                }
            }
            _ => None
        }
    });
    // return if we aren't rendering it.
    if r2.is_none() {
        return None;
    }
    // ok, we're rendering it, so need to update last render frame on it,
    // which requires a mutable reference.  we couldn't use a
    // mutable ref earlier, because we had to do two simultaneous lookups on the hash table.
    // so we have to refetch as mutable, set the frame value and then (for safety)
    // refetch as immutable again so that we can pass that value on.  that's three
    // hash lookups guaranteed but fortunately we're only doing this for active mods.
    drop(r);
    drop(r2);
    mstate.mods.get_mut(&mod_key).map(|nmods| {
        if target_mod_index >= nmods.len() {
            // error, spam the log i guess
            write_log_file(&format!("selected target mod index {} exceeds number of mods {}",
                target_mod_index, nmods.len()));
        } else {
            let nmod = &mut nmods[target_mod_index];
            // we set the last frame render on all mods (not just parents) because 
            // variant-tracking uses it.
            nmod.last_frame_render = current_frame_num;
        }
    });
    let r = mstate.mods.get(&mod_key).and_then(|nmods| {
        if target_mod_index < nmods.len() {
            Some(&nmods[target_mod_index])
        } else {
            None
        }
    });
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_state::new_fnv_map;
    use global_state::{LoadedModState,LoadedModsMap,ModsByNameMap};
    use types::native_mod::NativeModData;

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
    }
    fn new_state(mmap:LoadedModsMap) -> LoadedModState {
        let mut mods_by_name:ModsByNameMap  = new_fnv_map(mmap.len());
        use std::collections::HashSet;
        let mut parent_mods:HashSet<String> = HashSet::new();
        
        for (mk,nmdv) in mmap.iter() {
            for nmod in nmdv.iter() {
                mods_by_name.insert(nmod.name.to_owned(), *mk);
                if !nmod.parent_mod_name.is_empty() {
                    parent_mods.insert(nmod.parent_mod_name.to_owned());
                }
            }
        }
        let mut mmap = mmap;
        // mark parents
        for parent in parent_mods {
            let pmk = mods_by_name.get(&parent).expect("no parent");
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
    #[test]
    fn test_select_basic() {
        let mut modmap:LoadedModsMap = new_fnv_map(10);
        
        add_mod(&mut modmap, new_mod("Mod1", 100, 200));
        add_mod(&mut modmap, new_mod("Mod2", 101, 201));
        let mut mstate = new_state(modmap);
        let r = select(&mut mstate, 99, 100, 1);
        assert!(r.is_none());
        let r = select(&mut mstate, 100, 202, 1);
        assert!(r.is_none());
        let r = select(&mut mstate, 100, 200, 1);
        assert_eq!(r.expect("no mod found").name, "Mod1".to_string());
        let r = select(&mut mstate, 101, 201, 1);
        assert_eq!(r.expect("no mod found").name, "Mod2".to_string());
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
        child.parent_mod_name = "Mod1P".to_string();
        add_mod(&mut modmap, child);
        // add another child for a different parent
        let mut child = new_mod("Mod3C", 101, 201);
        child.parent_mod_name = "Mod4P".to_string();
        add_mod(&mut modmap, child);
        let mut mstate = new_state(modmap);
        // when both parents have rendered recently, trying to select either child will be None
        // since both are eligible and we can't pick one
        // (note, variations doesn't apply here, because variations
        // should select root parent mods, not children).
        let r = select(&mut mstate, 101, 201, 1);
        assert!(r.is_none());
        // update so that we have just one recent parent
        let pkey = mstate.mods_by_name.get(&"Mod1P".to_owned()).expect("no parent");
        let pmods = mstate.mods.get_mut(pkey).expect("no parent");
        let pmod = find_parent("Mod1P", pmods).expect("no parent");
        pmod.last_frame_render = 50;
        // trying to select child when one parent has rendered recently should find it 
        let r = select(&mut mstate, 101, 201, 50);
        assert_eq!(r.expect("no mod found").name, "Mod2C".to_string());
        // and should not when parent hasn't been rendered
        let r = select(&mut mstate, 101, 201, 100);
        assert!(r.is_none());
        // when a parent is rendered, its frame should update
        let r = select(&mut mstate, 100, 200, 60);
        match r {
            Some(nmod) => {
                assert_eq!(nmod.name, "Mod1P".to_string());
                assert_eq!(nmod.last_frame_render, 60);
            },
            _ => panic!("test failed")
        }
    }
    
    #[test]
    fn variants() {
        let mut modmap:LoadedModsMap = new_fnv_map(10);
        add_mod(&mut modmap, new_mod("Mod1", 100, 200));
        add_mod(&mut modmap, new_mod("Mod2", 100, 200));
        add_mod(&mut modmap, new_mod("ModP", 101, 201));
        let mut child = new_mod("ModC", 100, 200);
        child.parent_mod_name = "ModP".to_string();
        add_mod(&mut modmap, child);
        let mut mstate = new_state(modmap);
        // selecting 100/200 mod should return the ModC because its parent is active - the other 
        // two have no parent and so are lower priority, so we exclude them.
        let r = select(&mut mstate, 100, 200, 0);
        assert_eq!(r.expect("no mod found").name, "ModC".to_string());
        // now select with a more recent frame to exclude the parent, this should return the first
        // mod, because we haven't selected a variant yet, so the default is the first
        let r = select(&mut mstate, 100, 200, 50);
        //assert!(r.is_none(), "unexpected mod: {:?}", r.unwrap().name);
        assert_eq!(r.expect("no mod found").name, "Mod1".to_string());
        // now pick a variant.  the indexes will be the same as the mod insertion order.
        let mk = NativeModData::mod_key(200, 100);
        mstate.selected_variant.insert(mk, 0);
        let r = select(&mut mstate, 100, 200, 50);
        assert_eq!(r.expect("no mod found").name, "Mod1".to_string());
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 1;
        let r = select(&mut mstate, 100, 200, 50);
        assert_eq!(r.expect("no mod found").name, "Mod2".to_string());
        // select() should not return a selected child 
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 2;
        let r = select(&mut mstate, 100, 200, 50);
        assert!(r.is_none(), "unexpected mod: {:?}", r.unwrap().name);
        // select() should not puke if selected child is out of range
        *mstate.selected_variant.get_mut(&mk).expect("oops") = 3;
        let r = select(&mut mstate, 100, 200, 50);
        assert!(r.is_none(), "unexpected mod: {:?}", r.unwrap().name);
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