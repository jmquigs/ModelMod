use global_state::LoadedModState;
use types::native_mod::NativeModData;
use shared_dx9::util::*;

/// Select a mod for rendering, if any.
///
/// The mod state is &mut because we may need to update the last frame rendered for any 
/// parent mods we find.
///
/// Perf note: the first part of this function is very hot and will be called for literally 
/// everything drawn by the game.  So its important to get out of here early if there is no match.
/// This could check could even be inlined as a separate function, but hopefully the jump 
/// doesn't add much overhead (and maybe llvm will split this into hot/cold parts for us.)
pub fn select(mstate: &mut LoadedModState, prim_count:u32, vert_count:u32, current_frame_num:u64) -> Option<&NativeModData> {
    let mod_key = NativeModData::mod_key(vert_count, prim_count);
    let r = mstate.mods.get(&mod_key);
    // just get out of here if we didn't have a match
    if let None = r {
        return None;
    }
    // found at least one mod.  do some more checks to see if each has a parent, and if the parent
    // is active.  count the active parents we find because if more than one is active,
    // we have ambiguity and can't render any of them.
    let mut target_mod_index:usize = 0;
    let mut active_parent_name:&str = "";
    let r2 = r.and_then(|nmods| {
        let mut num_active_parents = 0;
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
                                // remember that mod index.  not that we'll slam this if we
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
        // if multiple mods but only one parent, we're good
        if nmods.len() > 1 && num_active_parents == 1 {
            // write_log_file(&format!("rend mod {} because just one active parent named '{}'",
            //     nmods[target_mod_index].name, active_parent_name));
            Some(())
        }
        // if just one mod it doesn't have a parent, or if it does and there is just one parent,
        // also good.
        else if nmods.len() == 1 && (nmods[0].parent_mod_name.is_empty() || num_active_parents == 1) {
            // write_log_file(&format!("rend mod {} because just one mod with parname '{}' or {} parents",
            // nmods[target_mod_index].name, nmods[0].parent_mod_name, num_active_parents));

            Some(())
        } else {
            None
        }
    });
    // return if we aren't rendering it.
    if let None = r2 {
        return None;
    }
    // ok, we're rendering it, but it might be a parent mod too, so we have to set
    // the last frame on it, which requires a mutable reference.  we couldn't use a
    // mutable ref earlier, because we had to do two lookups on the hash table.
    // so we have to refetch as mutable, set the frame value and then (for safety)
    // refetch as immutable again so that we can pass that value on.  that's three
    // hash lookups guaranteed but fortunately we're only doing this for active mods.
    // we also can't be clever and return an immutable ref now if it isn't a parent,
    // because we won't be able to even write the code that checks for the parent
    // since it would require the get_mut call and thus a mutable and immutable ref
    // would be active at the same time.
    drop(r);
    drop(r2);
    mstate.mods.get_mut(&mod_key).map(|nmods| {
        if target_mod_index >= nmods.len() {
            // error, spam the log i guess
            write_log_file(&format!("selected target mod index {} exceeds number of mods {}",
                target_mod_index, nmods.len()));
        } else {
            let nmod = &mut nmods[target_mod_index];
            if nmod.is_parent {
                nmod.last_frame_render = current_frame_num;
            }
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