
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use fnv::FnvHashMap;
use interop;
use interop::NativeModData;
use util;
use d3dx;
use std;
use std::ptr::null_mut;
use shared_dx9::util::*;
use crate::hook_render::{dev_state, DEVICE_STATE, GLOBAL_STATE, GLOBAL_STATE_LOCK};

pub enum AsyncLoadState {
    NotStarted = 51,
    Pending,
    InProgress,
    Complete,
}

pub unsafe fn clear_loaded_mods(device: *mut IDirect3DDevice9) {
    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to clear mod data");
        return;
    }

    // get device ref count prior to adding everything
    (*device).AddRef();
    let pre_rc = (*device).Release();

    let mods = GLOBAL_STATE.loaded_mods.take();
    let mut cnt = 0;
    mods.map(|mods| {
        for (_key, modvec) in mods.into_iter() {
            for nmd in modvec {
                cnt += 1;
                if nmd.vb != null_mut() {
                    (*nmd.vb).Release();
                }
                if nmd.ib != null_mut() {
                    (*nmd.ib).Release();
                }
                if nmd.decl != null_mut() {
                    (*nmd.decl).Release();
                }
                
                for tex in nmd.textures.iter() {
                    if *tex != null_mut() {
                        let tex = *tex as *mut IDirect3DBaseTexture9;
                        (*tex).Release();
                    }
                }
            }
        }
    });
    GLOBAL_STATE.mods_by_name = None;

    (*device).AddRef();
    let post_rc = (*device).Release();
    let diff = pre_rc - post_rc;
    if (dev_state().d3d_resource_count as i64 - diff as i64) < 0 {
        write_log_file(&format!(
            "DOH resource count would go below zero (curr: {}, removed {}),",
            dev_state().d3d_resource_count, diff
        ));
    } else {
        dev_state().d3d_resource_count -= diff;
    }

    write_log_file(&format!("unloaded {} mods", cnt));
}

pub unsafe fn setup_mod_data(device: *mut IDirect3DDevice9, callbacks: interop::ManagedCallbacks) {
    clear_loaded_mods(device);

    let mod_count = (callbacks.GetModCount)();
    if mod_count <= 0 {
        return;
    }

    if device == null_mut() {
        return;
    }

    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to setup mod data");
        return;
    }
    
    // need d3dx for textures
    GLOBAL_STATE.device = Some(device);
    if GLOBAL_STATE.d3dx_fn.is_none() {
        GLOBAL_STATE.d3dx_fn = d3dx::load_lib(&GLOBAL_STATE.mm_root)
            .map_err(|e| {
                write_log_file(&format!(
                    "failed to load d3dx: texture snapping not available: {:?}",
                    e
                ));
                e
            })
            .ok();
    }

    // get device ref count prior to adding everything
    (*device).AddRef();
    let pre_rc = (*device).Release();

    let mut loaded_mods: FnvHashMap<u32, Vec<interop::NativeModData>> =
        FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());
    // map of modname -> mod key, which can then be indexed into loaded mods.  used by 
    // child mods to find the parent.
    let mut mods_by_name: FnvHashMap<String,u32> = 
        FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());

    // temporary list of all mods that have been referenced as a parent by something
    use std::collections::HashSet;
    let mut parent_mods:HashSet<String> = HashSet::new();
    for midx in 0..mod_count {
        let mdat: *mut interop::ModData = (callbacks.GetModData)(midx);

        if mdat == null_mut() {
            write_log_file(&format!("null mod at index {}", midx));
            continue;
        }
        let mod_name = util::from_wide_str(&(*mdat).modName).unwrap_or_else(|_e| "".to_owned());
        let mod_name = mod_name.trim().to_owned();
        let parent_mod = util::from_wide_str(&(*mdat).parentModName).unwrap_or_else(|_e| "".to_owned());
        let parent_mod = parent_mod.trim().to_owned();
        let (prims,verts) = if (*mdat).numbers.mod_type == (interop::ModType::Deletion as i32) {
            ((*mdat).numbers.ref_prim_count as u32,(*mdat).numbers.ref_vert_count as u32)
        } else {
            ((*mdat).numbers.prim_count as u32, (*mdat).numbers.vert_count as u32)
        };
        write_log_file(&format!("==> Initializing mod: name '{}', parent '{}', type {}, prims {}, verts {}", 
            mod_name, parent_mod, (*mdat).numbers.mod_type, prims, verts));
        let mod_type = (*mdat).numbers.mod_type;
        if mod_type != interop::ModType::GPUReplacement as i32
            && mod_type != interop::ModType::Deletion as i32
        {
            write_log_file(&format!(
                "Unsupported mod type: {}",
                (*mdat).numbers.mod_type
            ));
            continue;
        }

        // names are case insensitive
        let mod_name = mod_name.to_lowercase();
        let mut native_mod_data = interop::NativeModData {
            mod_data: (*mdat),
            vb_data: null_mut(),
            ib_data: null_mut(),
            decl_data: null_mut(),
            vb: null_mut(),
            ib: null_mut(),
            decl: null_mut(),
            textures: [null_mut(); 4],
            is_parent: false,
            parent_mod_name: "".to_owned(),
            last_frame_render: 0,
            name: mod_name.to_owned(),
        };

        if (*mdat).numbers.mod_type == (interop::ModType::Deletion as i32) {
            let hash_code = NativeModData::mod_key(
                native_mod_data.mod_data.numbers.ref_vert_count as u32,
                native_mod_data.mod_data.numbers.ref_prim_count as u32,
            );

            loaded_mods.entry(hash_code).or_insert(vec![]).push(native_mod_data);
            // thats all we need to do for these.
            continue;
        }

        let decl_size = (*mdat).numbers.decl_size_bytes;
        // vertex declaration construct copies the vec bytes, so just keep a temp vector reference for the data
        let (decl_data, _decl_vec) = if decl_size > 0 {
            let mut decl_vec: Vec<u8> = Vec::with_capacity(decl_size as usize);
            let decl_data: *mut u8 = decl_vec.as_mut_ptr();
            (decl_data, Some(decl_vec))
        } else {
            (null_mut(), None)
        };

        let vb_size = (*mdat).numbers.prim_count * 3 * (*mdat).numbers.vert_size_bytes;
        let mut vb_data: *mut u8 = null_mut();

        // index buffers not currently supported
        let ib_size = 0; //mdat->indexCount * mdat->indexElemSizeBytes;
        let ib_data: *mut u8 = null_mut();

        // create vb
        let mut out_vb: *mut IDirect3DVertexBuffer9 = null_mut();
        let out_vb: *mut *mut IDirect3DVertexBuffer9 = &mut out_vb;
        let hr = (*device).CreateVertexBuffer(
            vb_size as UINT,
            D3DUSAGE_WRITEONLY,
            0,
            D3DPOOL_MANAGED,
            out_vb,
            null_mut(),
        );
        if hr != 0 {
            write_log_file(&format!(
                "failed to create vertex buffer for mod {}: HR {:x}",
                midx, hr
            ));
            return;
        }

        let vb = *out_vb;

        // lock vb to obtain write buffer
        let hr = (*vb).Lock(0, 0, std::mem::transmute(&mut vb_data), 0);
        if hr != 0 {
            write_log_file(&format!("failed to lock vertex buffer: {:x}", hr));
            return;
        }

        // fill all data buckets with managed code
        let ret = (callbacks.FillModData)(
            midx, decl_data, decl_size, vb_data, vb_size, ib_data, ib_size,
        );

        let hr = (*vb).Unlock();
        if hr != 0 {
            write_log_file(&format!("failed to unlock vertex buffer: {:x}", hr));
            (*vb).Release();
            return;
        }

        if ret != 0 {
            write_log_file(&format!("failed to fill mod data: {}", ret));
            (*vb).Release();
            return;
        }

        native_mod_data.vb = vb;

        // create vertex declaration
        let mut out_decl: *mut IDirect3DVertexDeclaration9 = null_mut();
        let pp_out_decl: *mut *mut IDirect3DVertexDeclaration9 = &mut out_decl;
        let hr =
            (*device).CreateVertexDeclaration(decl_data as *const D3DVERTEXELEMENT9, pp_out_decl);
        if hr != 0 {
            write_log_file(&format!("failed to create vertex declaration: {}", hr));
            (*vb).Release();
            return;
        }
        if out_decl == null_mut() {
            write_log_file("vertex declaration is null");
            (*vb).Release();
            return;
        }
        native_mod_data.decl = out_decl;

        let load_tex = |texpath:&[u16]| {
            let tex = util::from_wide_str(texpath).unwrap_or_else(|e| {
                write_log_file(&format!("failed to load texture: {:?}", e));
                "".to_owned()
            });
            let tex = tex.trim();
            
            let mut outtex = null_mut();
            if !tex.is_empty() {
                outtex = d3dx::load_texture(texpath.as_ptr()).map_err(|e| {
                    write_log_file(&format!("failed to load texture: {:?}", e));
                }).unwrap_or(null_mut());
                write_log_file(&format!("Loaded tex: {:?} {:x}", tex, outtex as u64));
            }
            outtex
        };
        native_mod_data.textures[0] = load_tex(&(*mdat).texPath0);
        native_mod_data.textures[1] = load_tex(&(*mdat).texPath1);
        native_mod_data.textures[2] = load_tex(&(*mdat).texPath2);
        native_mod_data.textures[3] = load_tex(&(*mdat).texPath3);

        let mod_key = NativeModData::mod_key(
            native_mod_data.mod_data.numbers.ref_vert_count as u32,
            native_mod_data.mod_data.numbers.ref_prim_count as u32,
        );
        if !parent_mod.is_empty() {
            native_mod_data.parent_mod_name = parent_mod.to_lowercase();
            parent_mods.insert(native_mod_data.parent_mod_name.clone());
        }
        // TODO: is hashing the generated mod key better than just hashing a tuple of prims,verts?
        loaded_mods.entry(mod_key).or_insert(vec![]).push(native_mod_data);
        if !mod_name.is_empty() {
            if mods_by_name.contains_key(&mod_name) {
                write_log_file(&format!("error, duplicate mod name: ignoring dup: {}", mod_name));
            } else {
                mods_by_name.insert(mod_name, mod_key);
            }
        }
        write_log_file(&format!(
            "allocated vb/decl for mod data {}: {:?}",
            midx,
            (*mdat).numbers
        ));
    }
    
    // mark all parent mods as such, and also warn about any parents that didn't load
    let mut resolved_parents = 0;
    let num_parents = parent_mods.len();
    for parent in parent_mods {
        match mods_by_name.get(&parent) {
            None => write_log_file(&format!("error, mod referenced as parent failed to load: {}", parent)),
            Some(modkey) => {
                match loaded_mods.get_mut(modkey) {
                    None => write_log_file(&format!("error, mod referenced as parent was found, but no loaded: {}", parent)),
                    Some(nmdatavec) => {
                        for nmdata in nmdatavec.iter_mut() {
                            if nmdata.name == parent {
                                resolved_parents += 1;
                                nmdata.is_parent = true
                            }
                        }
                    }
                }
            }
        }
    }
    write_log_file(&format!("resolved {} of {} parent mods", resolved_parents, num_parents));
    
    // verify that all multi-mod cases have parent mod names set.
    for nmodv in loaded_mods.values() {
        if nmodv.len() <= 1 {
            continue
        }
        // in a multimod case, all mods must have parents, and the parents have to be different
        // names.
        let mut parent_names: HashSet<String> = HashSet::new();
        for nmod in nmodv.iter() {
            if nmod.parent_mod_name.is_empty() {
                write_log_file(&format!("Error: mod '{}' ({} prims,{} verts) has no parent \
mod but it overlaps with another mod.  This won't render correctly.",
                nmod.name, nmod.mod_data.numbers.prim_count, nmod.mod_data.numbers.vert_count));
            } else {
                parent_names.insert(nmod.parent_mod_name.to_string());
            }
            
        }
        if nmodv.len() != parent_names.len() {
            write_log_file("Error: mod overlap found, check that all of these mods have proper/unique parents:");
            for nmod in nmodv.iter() {
                write_log_file(&format!("  mod: {}, prims: {}, verts: {}, parent: {}",
                nmod.name, nmod.mod_data.numbers.prim_count, nmod.mod_data.numbers.vert_count, nmod.parent_mod_name));
            }
        }
    }

    // get new ref count
    (*device).AddRef();
    let post_rc = (*device).Release();
    let diff = post_rc - pre_rc;
    (*DEVICE_STATE).d3d_resource_count += diff;
    write_log_file(&format!(
        "mod loading added {} to device {:x} ref count, new count: {}",
        diff, device as u64, (*DEVICE_STATE).d3d_resource_count
    ));

    GLOBAL_STATE.loaded_mods = Some(loaded_mods);
    GLOBAL_STATE.mods_by_name = Some(mods_by_name);
}
