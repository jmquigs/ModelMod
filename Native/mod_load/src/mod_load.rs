
use shared_dx::types::DevicePointer;
use types::d3ddata;
use types::native_mod::ModD3DState;
use types::native_mod::NativeModData;
use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::d3d11::D3D11_BIND_VERTEX_BUFFER;
use winapi::um::d3d11::D3D11_BUFFER_DESC;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;
use winapi::um::d3d11::D3D11_SUBRESOURCE_DATA;
use winapi::um::d3d11::D3D11_USAGE_DEFAULT;
use winapi::um::d3d11::ID3D11Buffer;
use winapi::um::d3d11::ID3D11Device;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use fnv::FnvHashMap;

use util;
use d3dx;
use std;
use std::ptr::null_mut;
use shared_dx::util::*;
use device_state::*;
use global_state::{GLOBAL_STATE, GLOBAL_STATE_LOCK, LoadedModState};
use types::interop;
use types::native_mod;

pub enum AsyncLoadState {
    NotStarted = 51,
    Pending,
    InProgress,
    Complete,
}

/// Release any d3d resources owned by a mod.
fn clear_d3d_data(nmd:&mut NativeModData) {
        match nmd.d3d_data {
            native_mod::ModD3DState::Loaded(ref mut d3dd) => unsafe {d3dd.release();},
            native_mod::ModD3DState::Partial(ref mut d3dd) => unsafe {d3dd.release();},
            native_mod::ModD3DState::Unloaded => {}
        };
        nmd.d3d_data = native_mod::ModD3DState::Unloaded;
}

pub unsafe fn clear_loaded_mods(device: DevicePointer) {
    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to clear mod data");
        return;
    }

    // get device ref count prior to adding everything
    let pre_rc = device.get_ref_count();

    let mods = GLOBAL_STATE.loaded_mods.take();
    let mut cnt = 0;
    mods.map(|mods| {
        for (_key, modvec) in mods.mods.into_iter() {
            for mut nmd in modvec {
                cnt += 1;
                clear_d3d_data(&mut nmd);
            }
        }
    });
    GLOBAL_STATE.loaded_mods = None;
    GLOBAL_STATE.load_on_next_frame.as_mut().map(|hs| hs.clear());

    let post_rc = (device).get_ref_count();
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

/// Create D3D resources for a mod using the data loaded by managed code. This usually consists of a
/// vertex buffer, declaration and optionally one or more textures.  `midx` is the mod index
/// into the current mod DB (and should be less than GetModCount()).
pub unsafe fn load_d3d_data9(device: *mut IDirect3DDevice9, callbacks: interop::ManagedCallbacks,
    midx: i32, nmd: &mut NativeModData) {
    let mdat = &nmd.mod_data;

    if let native_mod::ModD3DState::Loaded(_) = nmd.d3d_data {
        // bug, should have been cleared first
        write_log_file(&format!(
            "Error, d3d data for mod {} already loaded",
            nmd.name
        ));
        return;
    }

    let decl_size = mdat.numbers.decl_size_bytes;
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
            nmd.name, hr
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

    let mut d3dd = d3ddata::ModD3DData9::new();

    d3dd.vb = vb;

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
    d3dd.decl = out_decl;

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
    d3dd.textures[0] = load_tex(&(*mdat).texPath0);
    d3dd.textures[1] = load_tex(&(*mdat).texPath1);
    d3dd.textures[2] = load_tex(&(*mdat).texPath2);
    d3dd.textures[3] = load_tex(&(*mdat).texPath3);

    write_log_file(&format!(
        "allocated vb/decl for mod {}, idx {}: {:?}", nmd.name,
        midx,
        mdat.numbers
    ));

    nmd.d3d_data = native_mod::ModD3DState::Loaded(native_mod::ModD3DData::D3D9(d3dd));
}

pub unsafe fn load_d3d_data11(device: *mut ID3D11Device, callbacks: interop::ManagedCallbacks, midx: i32, nmd: &mut NativeModData) -> bool {
    let mdat = &nmd.mod_data;

    if device.is_null() {
        write_log_file(&format!("Error, device is null"));
        return false;
    }

    if let native_mod::ModD3DState::Loaded(_) = nmd.d3d_data {
        // bug, should have been cleared first
        write_log_file(&format!(
            "Error, d3d data for mod {} already loaded",
            nmd.name
        ));
        return false;
    }

    // extract the vertex layout pointer and d3d data to finish the load
    let (vlayout,d3d_data) =
        if let ModD3DState::Partial(native_mod::ModD3DData::D3D11(ref mut d3dd)) = nmd.d3d_data {
            if d3dd.vlayout.is_null() {
                write_log_file(&format!(
                    "Error, d3d11 data for mod {} is missing vertex layout",
                    nmd.name
                ));
                return false;
            }
            (d3dd.vlayout,d3dd)
        } else {
            write_log_file(&format!("Error, d3d11 data for mod {} has not been partially loaded", nmd.name));
            return false;
        };
    // lookup actual layout data in render state using the pointer
    let vlayout = {
        let layout_u64 = vlayout as u64;
        let res = GLOBAL_STATE.dx11rs.input_layouts_by_ptr
            .as_ref().map(|hm| hm.get(&layout_u64)).flatten();
        match res {
            None => {
                write_log_file(&format!(
                    "Error, d3d11 data for mod {} has vertex layout but it is not in the render state",
                    nmd.name
                ));
                return false;
            },
            Some(vf) => vf,
        }
    };

    // in dx11 I pass the layout as an _in_ parameter containing the layout.  Contrast with
    // dx9 where the declaration is an _out_ parameter and receives the declaration from managed
    // code.

    // clone data because we need a mut pointer to pass it
    let mut layout_data: Vec<_> = vlayout.layout.clone();
    let decl_size = std::mem::size_of::<D3D11_INPUT_ELEMENT_DESC>() * layout_data.len();
    let decl_data = layout_data.as_mut_ptr();

    // set vb size and create scratch buffer
    let vert_size = vlayout.size as i32;
    if vert_size == 0 {
        // gawd get this far and size is zero??
        write_log_file(&format!("Error, vertex size is zero for mod {}", nmd.name));
        return false;
    }
    let vb_size = (*mdat).numbers.prim_count * 3 * vert_size;
    let mut vb_data = vec![0u8; vb_size as usize];

    // index buffers not currently supported
    let ib_size = 0; //mdat->indexCount * mdat->indexElemSizeBytes;
    let ib_data: *mut u8 = null_mut();

    // fill all data buckets with managed code.
    let ret = (callbacks.FillModData)(
        midx, decl_data as *mut u8, decl_size as i32, vb_data.as_mut_ptr() , vb_size, ib_data, ib_size,
    );

    if ret != 0 {
        write_log_file(&format!("failed to fill mod data: {}", ret));
        return false;
    }

    // create vb
    let mut out_vb: *mut ID3D11Buffer = null_mut();
    let out_vb: *mut *mut ID3D11Buffer = &mut out_vb;
    let mut vb_desc = D3D11_BUFFER_DESC {
        ByteWidth: vb_size as UINT,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_VERTEX_BUFFER,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut vb_init_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vb_data.as_mut_ptr() as *const c_void,
        SysMemPitch: 0,
        SysMemSlicePitch: 0,
    };
    let hr = (*device).CreateBuffer(&mut vb_desc, &mut vb_init_data, out_vb);
    if hr != 0 {
        write_log_file(&format!(
            "failed to create vertex buffer for mod {}: HR {:x}",
            nmd.name, hr
        ));
        return false;
    }

    d3d_data.vb = *out_vb;
    // TODO11
    //d3d_data.textures

    write_log_file(&format!(
        "allocated vb for mod {}, idx {}: {:?}", nmd.name,
        midx,
        mdat.numbers
    ));

    nmd.d3d_data.set_loaded();
    true
}

/// Set up mod data structures.  Should be called after the managed code is done loading
/// on its side.  Note that this will also clear any previously loaded mods (and their DX
/// resources, if any).  However it does not load any new DX resources, that is done
/// by `load_deferred_mods`.
pub unsafe fn setup_mod_data(device: DevicePointer, callbacks: interop::ManagedCallbacks) {
    clear_loaded_mods(device);

    let mod_count = (callbacks.GetModCount)();
    if mod_count <= 0 {
        return;
    }

    let lock = GLOBAL_STATE_LOCK.lock();
    if let Err(_e) = lock {
        write_log_file("failed to lock global state to setup mod data");
        return;
    }

    let ml_start = std::time::SystemTime::now();

    let mut loaded_mods: FnvHashMap<u32, Vec<native_mod::NativeModData>> =
        FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());
    // map of modname -> mod key, which can then be indexed into loaded mods.  used by
    // child mods to find the parent.
    let mut mods_by_name: FnvHashMap<String,u32> =
        FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());

    // temporary list of all mods that have been referenced as a parent by something
    use std::collections::HashSet;
    let mut all_parent_mods:HashSet<String> = HashSet::new();
    for midx in 0..mod_count {
        let mdat: *mut interop::ModData = (callbacks.GetModData)(midx);

        if mdat == null_mut() {
            write_log_file(&format!("null mod at index {}", midx));
            continue;
        }
        let mod_name = util::from_wide_str(&(*mdat).modName).unwrap_or_else(|_e| "".to_owned());
        let mod_name = mod_name.trim().to_owned();
        let parent_mods = util::from_wide_str(&(*mdat).parentModName).unwrap_or_else(|_e| "".to_owned());
        let parent_mods = parent_mods.trim();
        // check for an "or" list of parents
        let parent_mods:Vec<String> = native_mod::NativeModData::split_parent_string(&parent_mods);
        let (prims,verts) = if (*mdat).numbers.mod_type == (interop::ModType::Deletion as i32) {
            ((*mdat).numbers.ref_prim_count as u32,(*mdat).numbers.ref_vert_count as u32)
        } else {
            ((*mdat).numbers.prim_count as u32, (*mdat).numbers.vert_count as u32)
        };
        write_log_file(&format!("==> Initializing mod: name '{}', parents '{:?}', type {}, prims {}, verts {} (ref prims {}, ref verts {})",
            mod_name, parent_mods, (*mdat).numbers.mod_type, prims, verts,
            (*mdat).numbers.ref_prim_count, (*mdat).numbers.ref_vert_count));
        let mod_type = (*mdat).numbers.mod_type;
        if mod_type != interop::ModType::GPUReplacement as i32
            && mod_type != interop::ModType::GPUAdditive as i32
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
        let mut native_mod_data = native_mod::NativeModData {
            midx: midx,
            mod_data: (*mdat),
            d3d_data: native_mod::ModD3DState::Unloaded,
            is_parent: false,
            parent_mod_names: parent_mods,
            last_frame_render: 0,
            name: mod_name.to_owned(),
        };

        // get mod key
        let mod_key = native_mod::NativeModData::mod_key(
            native_mod_data.mod_data.numbers.ref_vert_count as u32,
            native_mod_data.mod_data.numbers.ref_prim_count as u32,
        );

        // wrangle names
        if native_mod_data.parent_mod_names.len() > 0 {
            // lowercase these and make parent mod entries for them
            native_mod_data.parent_mod_names = native_mod_data.parent_mod_names.iter().map(|parent_mod| {
                let plwr = parent_mod.to_lowercase();
                all_parent_mods.insert(plwr.clone());
                plwr
            }).collect();
        }

        let is_deletion_mod = (*mdat).numbers.mod_type == (interop::ModType::Deletion as i32);
        if !mod_name.is_empty() {
            // if it is a deletion mod, then there may be multiple mods with the same name
            // (one for each prim/vert combo that is deleted).  construct a new name that includes
            // the prim and vert count
            let mod_name = if is_deletion_mod {
                let new_mod_name = format!("{}_{}_{}", mod_name,
                    native_mod_data.mod_data.numbers.ref_prim_count,
                    native_mod_data.mod_data.numbers.ref_vert_count);
                write_log_file(&format!("using mod name {} for deletion mod: {}", new_mod_name, mod_name));
                new_mod_name
            } else {
                mod_name
            };

            if mods_by_name.contains_key(&mod_name) {
                write_log_file(&format!("error, duplicate mod name: ignoring dup: {}", mod_name));
            } else {
                mods_by_name.insert(mod_name, mod_key);
            }
        }
        //write_log_file(&format!("mod: {}, parents: {:?}", native_mod_data.name, native_mod_data.parent_mod_names));

        if is_deletion_mod {
            loaded_mods.entry(mod_key).or_insert(vec![]).push(native_mod_data);
            // thats all we need to do for these.
            continue;
        }

        // used to load the d3d resources here for all mods, but now that is delayed until the
        // mod is actually referenced so that we don't clog d3d with a bunch of possibly unused
        // stuff. (see `load_deferred_mods`)

        loaded_mods.entry(mod_key).or_insert(vec![]).push(native_mod_data);
    }

    // mark all parent mods as such, and also warn about any parents that didn't load
    let mut resolved_parents = 0;
    let num_parents = all_parent_mods.len();
    for parent in all_parent_mods {
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
            if nmod.parent_mod_names.is_empty() {
                write_log_file(&format!("Note: mod '{}' ({} prims,{} verts) has no parent \
mod but it overlaps with another mod.  Use the variant key to select this.",
                nmod.name, nmod.mod_data.numbers.prim_count, nmod.mod_data.numbers.vert_count));
            } else {
                nmod.parent_mod_names.iter().for_each(|parent_mod_name| {
                    parent_names.insert(parent_mod_name.to_string());
                });
            }

        }
        if nmodv.len() != parent_names.len() {
            write_log_file("Variants found:");
            for nmod in nmodv.iter() {
                write_log_file(&format!("  mod: {}, prims: {}, verts: {}, parents: {:?}",
                nmod.name, nmod.mod_data.numbers.prim_count, nmod.mod_data.numbers.vert_count,
                nmod.parent_mod_names));
            }
        }
    }


    let now = std::time::SystemTime::now();
    let elapsed = now.duration_since(ml_start);
    if let Ok(elapsed) = elapsed {
        write_log_file(&format!("mod load complete in {}ms", elapsed.as_millis()));
    };

    GLOBAL_STATE.loaded_mods = Some(LoadedModState {
        mods: loaded_mods,
        mods_by_name: mods_by_name,
        selected_variant: global_state::new_fnv_map(16),
    } );
}

pub fn get_mod_by_name<'a>(name:&str, loaded_mods:&'a mut Option<LoadedModState>) -> Option<&'a mut NativeModData> {
    let (mods,mods_by_name) =
        match loaded_mods {
            Some(ref mut gs) => (&mut gs.mods, &gs.mods_by_name),
            _ => { return None; }
        };

    let mkey = mods_by_name.get(name);
    if let Some(mkey) = mkey {
        let nmods = mods.get_mut(mkey);
        if let Some(nmods) = nmods {
            return nmods.iter_mut().find(|nmod| &nmod.name == name);
        }
    }

    None
}

pub unsafe fn load_deferred_mods(device: DevicePointer, callbacks: interop::ManagedCallbacks) {
        let lock = GLOBAL_STATE_LOCK.lock();
        if let Err(_e) = lock {
            write_log_file("failed to lock global state to setup mod data");
            return;
        }

        // TODO11: dx11 has its own version of this, so port it
        if let DevicePointer::D3D9(dev) = device {
            // need d3dx for textures
            GLOBAL_STATE.device = Some(dev);
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
        }

        let to_load = match GLOBAL_STATE.load_on_next_frame {
            Some(ref mut hs) if hs.len() > 0 => hs,
            _ => { return; }
        };

        let ml_start = std::time::SystemTime::now();

        // get device ref count prior to adding mod
        let pre_rc = device.get_ref_count();

        let mut cnt = 0;
        for nmd in to_load.iter() {
            let mut nmod =
                get_mod_by_name(&nmd, &mut GLOBAL_STATE.loaded_mods);
            if let Some(ref mut nmod) = nmod {
                if let ModD3DState::Loaded(_) = nmod.d3d_data {
                    write_log_file(&format!("load_deferred_mods: mod already loaded: {}", nmod.name));
                    continue;
                }
                match device {
                    DevicePointer::D3D9(device) => {
                        load_d3d_data9(device, callbacks, nmod.midx, nmod);
                        cnt += 1;
                    }
                    DevicePointer::D3D11(device) => {
                        if load_d3d_data11(device, callbacks, nmod.midx, nmod) {
                            cnt += 1;
                        }
                    },
                }
            }
        }

        to_load.clear();

        // get new ref count
        let post_rc = device.get_ref_count();
        let diff = post_rc - pre_rc;
        (*DEVICE_STATE).d3d_resource_count += diff;

        let now = std::time::SystemTime::now();
        let elapsed = now.duration_since(ml_start);
        if let Ok(elapsed) = elapsed {
            write_log_file(
                &format!("load_deferred_mods: {} in {}ms, added {} to device {:x} ref count, new count: {}",
                cnt, elapsed.as_millis(), diff, device.as_u64(), (*DEVICE_STATE).d3d_resource_count
            ));
        };
}