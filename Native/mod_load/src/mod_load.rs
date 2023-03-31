
use shared_dx::dx11rs::VertexFormat;
use shared_dx::error;
use shared_dx::error::HookError;
use shared_dx::types::D3D11Tex;
use shared_dx::types::DevicePointer;
use shared_dx::types::TexPtr;
use types::d3ddata;
use types::native_mod::ModD3DState;
use types::native_mod::NativeModData;
use winapi::ctypes::c_void;
pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
use winapi::shared::dxgiformat::DXGI_FORMAT_R16G16_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32B32A32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32B32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
pub use winapi::shared::minwindef::*;
pub use winapi::shared::windef::{HWND, RECT};
pub use winapi::shared::winerror::{E_FAIL, S_OK};
use winapi::um::d3d11::D3D11_BIND_VERTEX_BUFFER;
use winapi::um::d3d11::D3D11_BUFFER_DESC;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;
use winapi::um::d3d11::D3D11_SHADER_RESOURCE_VIEW_DESC;
use winapi::um::d3d11::D3D11_SUBRESOURCE_DATA;
use winapi::um::d3d11::D3D11_TEXTURE2D_DESC;
use winapi::um::d3d11::D3D11_USAGE_DEFAULT;
use winapi::um::d3d11::ID3D11Device;
use winapi::um::d3d11::ID3D11ShaderResourceView;
use winapi::um::d3d11::ID3D11Texture2D;
use winapi::um::d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};
use fnv::FnvHashMap;

use util;
use d3dx;
use std;
use std::ffi::CStr;
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

unsafe fn load_tex(dp:DevicePointer, texpath:&[u16]) -> Option<TexPtr> {
    let tex = util::from_wide_str(texpath).unwrap_or_else(|e| {
        write_log_file(&format!("failed to load texture: {:?}", e));
        "".to_owned()
    });
    let tex = tex.trim();

    if !tex.is_empty() {
        match d3dx::load_texture(dp, texpath.as_ptr()) {
            Ok(tp)  if !tp.is_null() => {
                write_log_file(&format!("loaded texture: {}", tex));
                Some(tp)
            },
            Ok(_) => {
                write_log_file(&format!("failed to load texture: {}", tex));
                None
            },
            Err(e) => {
                write_log_file(&format!("failed to load texture: {}: {:?}", tex, e));
                None
            }
        }
    } else {
        None
    }
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
        write_log_file(&format!("failed to fill mod data: fill ret {} for mod {} ", ret, nmd.name));
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

    let dp = DevicePointer::D3D9(device);
    let load_tex_d3d9 = |texpath:&[u16]| {
        match load_tex(dp, texpath) {
            Some(TexPtr::D3D9(lp)) => lp,
            Some(TexPtr::D3D11(_)) => {
                write_log_file("ERROR: loaded d3d11 tex WTF");
                null_mut()
            },
            None => null_mut()
        }
    };
    d3dd.textures[0] = load_tex_d3d9(&(*mdat).texPath0);
    d3dd.textures[1] = load_tex_d3d9(&(*mdat).texPath1);
    d3dd.textures[2] = load_tex_d3d9(&(*mdat).texPath2);
    d3dd.textures[3] = load_tex_d3d9(&(*mdat).texPath3);

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

    //write_log_file(&format!("loading mod data on device {:x}", device as usize));

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
        match dev_state_d3d11_nolock() {
            Some(state) => {
                let layout_usize = vlayout as usize;
                let res = state.rs.context_input_layouts_by_ptr
                    .get(&layout_usize);
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
            },
            _ => {
                write_log_file(&format!(
                    "Error, no d3d11 hook state while loading mod {}",
                    nmd.name
                ));
                return false;
            }
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
    let vert_size = vlayout.size;
    if vert_size <= 0 {
        // gawd get this far and size is zero??
        write_log_file(&format!("Error, vertex size is invalid for mod {}: {}", nmd.name, vert_size));
        return false;
    }
    let vert_count = (*mdat).numbers.prim_count * 3;
    let vert_count =
        if vert_count <= 0 {
            write_log_file(&format!("Error, vertex count is invalid for mod {}: {}", nmd.name, vert_count));
            return false;
        } else {
            vert_count as u32
        };
    let vb_size = vert_count * vert_size;
    let mut vb_data = vec![0u8; vb_size as usize];

    // index buffers not currently supported
    let ib_size = 0; //mdat->indexCount * mdat->indexElemSizeBytes;
    let ib_data: *mut u8 = null_mut();

    // fill all data buckets with managed code.
    // not sure why I used signed ints in this interface, but if you are creating a >2GB mod vertex buffer
    // you've got bigger problems.
    let i32_vb_size = vb_size as i32;
    let ret = (callbacks.FillModData)(
        midx, decl_data as *mut u8, decl_size as i32, vb_data.as_mut_ptr(), i32_vb_size, ib_data, ib_size,
    );

    if ret != 0 {
        write_log_file(&format!("failed to fill mod data: {}", ret));
        return false;
    }

    let _ = update_normals(vb_data.as_mut_ptr(), vert_count, &vlayout)
        .map_err(|e| {
            write_log_file(&format!("Warning: failed to update normals: {:?}", e));
        });

    // create vb
    let mut vb_desc = D3D11_BUFFER_DESC {
        ByteWidth: vb_size as UINT,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_VERTEX_BUFFER,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut vb_init_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vb_data.as_ptr() as *const c_void,
        SysMemPitch: 0,
        SysMemSlicePitch: 0,
    };
    let mut vertex_buffer = std::ptr::null_mut();
    let hr = (*device).CreateBuffer(
        &mut vb_desc, &mut vb_init_data, &mut vertex_buffer);
    if hr != 0 {
        write_log_file(&format!(
            "failed to create vertex buffer for mod {}: HR {:x}",
            nmd.name, hr
        ));

        use winapi::shared::winerror::*;
        if hr == DXGI_ERROR_DEVICE_REMOVED {
            let dev_removed_reason = (*device).GetDeviceRemovedReason();
            match dev_removed_reason {
                DXGI_ERROR_DEVICE_HUNG => write_log_file(&format!("device hung")),
                DXGI_ERROR_DEVICE_REMOVED => write_log_file(&format!("device removed")),
                DXGI_ERROR_DEVICE_RESET => write_log_file(&format!("device reset")),
                DXGI_ERROR_DRIVER_INTERNAL_ERROR => write_log_file(&format!("driver internal error")),
                DXGI_ERROR_INVALID_CALL => write_log_file(&format!("invalid call")),
                _ => write_log_file(&format!("unknown device removed reason")),
            }
        }
        // check for E_OUTOFMEMORY
        else if hr as i64 == 0x8007000e {
            write_log_file(&format!("out of memory"));
        }

        return false;
    }

    d3d_data.vb = vertex_buffer;
    d3d_data.vert_size = vert_size as u32;
    d3d_data.vert_count = vert_count as u32;

    // load textures, if any
    let dp = DevicePointer::D3D11(device);
    d3d_data.has_textures = false;
    let mut load_tex_d3d11 = |texpath:&[u16], idx:usize| {
        let res = match load_tex(dp, texpath) {
            Some(TexPtr::D3D11(D3D11Tex::Tex(lp))) => lp,
            Some(TexPtr::D3D11(D3D11Tex::TexSrv(..))) => {
                write_log_file("ERROR: not expecting d3d11 texsrv here");
                return;
            },
            Some(TexPtr::D3D9(_)) => {
                write_log_file("ERROR: loaded d3d9 tex WTF");
                null_mut()
            },
            None => null_mut()
        };

        if !res.is_null() {
            // d3d11 makes us work harder to use the texture
            let p_tex = res as *mut ID3D11Texture2D;
            d3d_data.textures[idx] = p_tex;

            let mut desc:D3D11_TEXTURE2D_DESC = unsafe { std::mem::zeroed() };
            (*p_tex).GetDesc(&mut desc);

            let mut sv_desc:D3D11_SHADER_RESOURCE_VIEW_DESC = unsafe { std::mem::zeroed() };
            sv_desc.Format = desc.Format;
            sv_desc.ViewDimension = D3D11_SRV_DIMENSION_TEXTURE2D;
            sv_desc.u.Texture2D_mut().MipLevels = desc.MipLevels;
            sv_desc.u.Texture2D_mut().MostDetailedMip = 0;
            let mut p_srview: *mut ID3D11ShaderResourceView = null_mut();
            let pp_srview = &mut p_srview as *mut *mut ID3D11ShaderResourceView;

            let hr = (*device).CreateShaderResourceView(res, &sv_desc, pp_srview);
            if hr == 0 {
                d3d_data.srvs[idx] = p_srview;
                // since there is at least one valid texture, set the flag in the data
                d3d_data.has_textures = true;
            } else {
                write_log_file(&format!("failed to create shader resource view for mod {}, tex {}: HR {:x}", nmd.name, idx, hr));
            }
        }
    };
    load_tex_d3d11(&(*mdat).texPath0, 0);
    load_tex_d3d11(&(*mdat).texPath1, 1);
    load_tex_d3d11(&(*mdat).texPath2, 2);
    load_tex_d3d11(&(*mdat).texPath3, 3);

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
    write_log_file(&format!("setting up {} mods", mod_count));
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
        write_log_file(&format!("==> Initializing mod: name '{}', idx: {}, parents '{:?}', type {}, prims {}, verts {} (ref prims {}, ref verts {})",
            mod_name, midx,
            parent_mods, (*mdat).numbers.mod_type, prims, verts,
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
            loaded_mods.entry(mod_key).or_insert_with(|| vec![]).push(native_mod_data);
            // thats all we need to do for these.
            continue;
        }

        // used to load the d3d resources here for all mods, but now that is delayed until the
        // mod is actually referenced so that we don't clog d3d with a bunch of possibly unused
        // stuff. (see `load_deferred_mods`)

        loaded_mods.entry(mod_key).or_insert_with(|| vec![]).push(native_mod_data);
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

    let mut printed_variant_lead = false;
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
                //  replaced this with variant lead below

//                 write_log_file(&format!("Note: mod '{}' ({} prims,{} verts) has no parent \
// mod but it overlaps with another mod.  Use the variant next/prev keys to select it.",
//                 nmod.name, nmod.mod_data.numbers.prim_count, nmod.mod_data.numbers.vert_count));
            } else {
                nmod.parent_mod_names.iter().for_each(|parent_mod_name| {
                    parent_names.insert(parent_mod_name.to_string());
                });
            }

        }
        if nmodv.len() != parent_names.len() {
            if !printed_variant_lead {
                printed_variant_lead = true;
                write_log_file("Note: the following mods were found that overlap with the same ref, but have no parent set.");
                write_log_file("these mods will be initialized as variants and available via the next/prev variant keybindings.");
                write_log_file("if you did not mean for these to be variants, sent the parent field in the mod so that they are only");
                write_log_file("rendered when that parent mod is rendered.");
            }
            let (ref_prims,ref_verts) =
                (nmodv[0].mod_data.numbers.ref_prim_count, nmodv[0].mod_data.numbers.ref_vert_count);
            write_log_file(&format!("Variants for ref geom ({} prims, {} verts):", ref_prims, ref_verts));
            for nmod in nmodv.iter() {
                write_log_file(&format!("  mod: {}, geom ({} prims, {} verts), parents: {:?}",
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

        // ensure d3dx is loaded in case we need to load mod textures
        match device {
            DevicePointer::D3D9(dev) => {
                GLOBAL_STATE.device = Some(dev); // TODO: this gross even for d3d9, really
                // should just pass the device in from whomever is calling it (who should have
                // the current device pointer)
            },
            DevicePointer::D3D11(_dev) => {}, // skip
        }
        if GLOBAL_STATE.d3dx_fn.is_none() {
            GLOBAL_STATE.d3dx_fn = d3dx::load_lib(&GLOBAL_STATE.mm_root, &device)
                .map_err(|e| {
                    write_log_file(&format!(
                    "failed to load d3dx: texture loading and snapping not available: {:?}",
                        e
                    ));
                    e
                })
                .ok();
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
            if cnt > 0 {
                write_log_file(
                    &format!("load_deferred_mods: {} in {}ms, added {} to device {:x} ref count, new count: {}",
                    cnt, elapsed.as_millis(), diff, device.as_usize(), (*DEVICE_STATE).d3d_resource_count
                ));
            }
        };
}

#[repr(C)]
#[derive(Debug)]
struct Float3 {
    x:f32,
    y:f32,
    z:f32
}

#[repr(C)]
#[derive(Debug)]
struct Float2 {
    x:f32,
    y:f32,
}


#[allow(dead_code)]
#[repr(u32)]
#[derive(Debug, Copy, Clone)]
enum CNormFlags {
    Default = 0,
    WeightByArea = 1,
    WeightEqual = 2,
    WindCW = 4,
}

#[allow(non_camel_case_types)]
/// DirectXMesh doesn't normally build as a dll.  Had to change it to do that and then manually
/// export this function.
type DirectX_ComputeNormals_32Fn = unsafe extern "stdcall" fn(indices:*const u32,
    nFaces: usize, positions: *const Float3, nVerts:usize, flags:CNormFlags, normals:*mut Float3) -> HRESULT;
    #[allow(non_camel_case_types)]
type DirectX_ComputeTangentFrame_32TBFn = unsafe extern "stdcall" fn(indices:*const u32,
    nFaces: usize, positions: *const Float3, normals:*const Float3, texcoords:*const Float2,
    nVerts:usize, tangents:*mut Float3, binormals:*mut Float3) -> HRESULT;

// use different dlls for 32/64 bit
#[cfg(target_pointer_width = "32")]
const DXMESH_DLL:&'static str = r#"TPLib\DirectXMesh_x86.dll"#;
#[cfg(target_pointer_width = "64")]
const DXMESH_DLL:&'static str = r#"TPLib\DirectXMesh_x64.dll"#;

/// Update normals and tangents/bitangents using DirectXMesh.
///
/// Normal update generally disabled by default since it doesn't do smooth normals like blender,
/// and the faceting looks bad most of the time.  Useful for debugging normals though, since the
/// normals are accurate.
///
/// Tangent/bitangent update is enabled by default since its generates vectors that are much more
/// accurate for most models than what the managed code generates (which is basically just wrong).
///
fn update_normals(data:*mut u8, vert_count:u32, layout:&VertexFormat) -> error::Result<()> {
    let mut update_normals = false;
    let mut update_tangents = true;
    let mut flags = CNormFlags::Default;
    let mut reverse = false;

    // determine config and whether we should even do this
    let res = unsafe { &GLOBAL_STATE.interop_state }
        .as_ref()
        .ok_or(HookError::MeshUpdateFailed(String::from(
            "no interop state: was device created?",
        )))
        .and_then(|is| {
            let carr_ptr = &is.conf_data.ProfileKey[0] as *const i8;
            unsafe { CStr::from_ptr(carr_ptr) }
            .to_str()
            .map_err(HookError::CStrConvertFailed)
        })
        .and_then(|profile_root| {
            unsafe {
                let do_update_nrm = util::reg_query_dword(profile_root, "GameProfileUpdateNormals")
                .map_err(|_e| {
                    //write_log_file(&format!("normal update disabled: {:?}", e));
                }).unwrap_or(0);
                update_normals = do_update_nrm > 0;

                let tankey = "GameProfileUpdateTangents";
                let do_update_tan = util::reg_query_dword(profile_root, tankey)
                .map(|f| {
                    if f == 0 {
                        write_log_file(&format!("tangent update disabled by registry {}\\{}", profile_root, tankey));
                    }
                    f
                })
                .map_err(|_e| {
                    //write_log_file(&format!("tangent update disabled: {:?}", e));
                }).unwrap_or(1);
                update_tangents = do_update_tan > 0;

                if !update_normals && !update_tangents {
                    return Ok(());
                }
                if update_normals {
                    flags = util::reg_query_dword(profile_root, "GameProfileUpdateNormalFlags",)
                    .map(|f| std::mem::transmute(f))
                    .map_err(|e| {
                        write_log_file(&format!("using default {:?} for update normal flags: {:?}", flags, e));
                    }).unwrap_or(flags);
                }

                reverse = util::reg_query_dword(profile_root,"GameProfileReverseNormals",)
                .map(|f| f > 0)
                .map_err(|e| {
                    write_log_file(&format!("using default {:?} for reverse normals: {:?}", reverse, e));
                }).unwrap_or(reverse);

                Ok(())
            }
        });
    res?;
    if !update_normals && !update_tangents {
        return Ok(());
    }
    let what = if update_normals && update_tangents {
        format!("normals, tangents, bitangents; normal flags: {:?}", flags)
    } else if update_normals {
        format!("normals; normal flags: {:?}", flags)
    } else {
        format!("tangents and bitangents")
    };
    write_log_file(&format!("updating {}; reverse: {}", what, reverse));

    let mut dllpath = unsafe { &GLOBAL_STATE.mm_root.as_ref() }.ok_or_else (||
        HookError::MeshUpdateFailed(String::from("no mmroot")))?.to_owned();
    dllpath.push('\\');
    dllpath.push_str(DXMESH_DLL);
    let lib = util::load_lib(&dllpath)?;

    let compute_normals_32:Option<DirectX_ComputeNormals_32Fn> = if update_normals {
        let addr = util::get_proc_address(lib, "DirectX_ComputeNormals_32")?;
        unsafe { Some(std::mem::transmute(addr)) }
    } else {
        None
    };
    let compute_tangentframe_32tb:Option<DirectX_ComputeTangentFrame_32TBFn> = if update_tangents {
        let addr = util::get_proc_address(lib, "DirectX_ComputeTangentFrame_32TB")?;
        unsafe { Some(std::mem::transmute(addr)) }
    } else {
        None
    };

    // don't have an index buffer, so will need to generate an index array, using a 1:1 mapping between verts and indices
    let indices:Vec<u32> = (0..vert_count).collect();

    // helper function to convert semantic name ptrs to a lowercase string
    let ptr_to_str = |ptr:*const i8| -> String {
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let s = cstr.to_string_lossy().to_ascii_lowercase().to_string();
        //write_log_file(&format!("ptr_to_str: {:?} for ptr {:p}", s, ptr));
        s
    };
    // find the position offset in the layout
    let pos_elem = layout.layout.iter()
        .find(|l| ptr_to_str(l.SemanticName).starts_with("position"))
        .ok_or(HookError::MeshUpdateFailed("missing position in input layout".to_owned()))?;
    // we can do this if the pos is 3 or 4 F32s but not anything else
    if pos_elem.Format != DXGI_FORMAT_R32G32B32_FLOAT && pos_elem.Format != DXGI_FORMAT_R32G32B32A32_FLOAT {
        return Err(HookError::MeshUpdateFailed("unsupported position format".to_owned()).into());
    }
    // also need normal offset
    let norm_elem = layout.layout.iter()
        .find(|l| ptr_to_str(l.SemanticName).starts_with("normal"))
        .ok_or(HookError::MeshUpdateFailed("missing normal in input layout".to_owned()))?;

    // to compute tangents need the texcoord offset
    let tex_elem = if update_tangents {
        Some(layout.layout.iter()
        .find(|l| ptr_to_str(l.SemanticName).starts_with("texcoord"))
        .ok_or(HookError::MeshUpdateFailed("missing texcoord in input layout".to_owned()))?)
    } else {
        None
    };

    let pos_offset = pos_elem.AlignedByteOffset as usize;
    let norm_offset = norm_elem.AlignedByteOffset as usize;

    // need to create separate arrays for the input positions and the normals.
    let mut positions:Vec<Float3> = Vec::with_capacity(vert_count as usize);
    let mut normals:Vec<Float3> = Vec::with_capacity(vert_count as usize);
    let mut texcoords:Vec<Float2> = Vec::with_capacity(vert_count as usize);
    let mut tangents:Vec<Float3> = Vec::with_capacity(vert_count as usize);
    let mut bitangents:Vec<Float3> = Vec::with_capacity(vert_count as usize);
    for i in 0..vert_count {
        // compute the offset to the vert and then the offset to the position in the vert using
        // pos_offset
        unsafe {
            let vertpos = data.offset((i * layout.size) as isize + pos_offset as isize);
            // there are at least 3 f32s starting at vertpos so copy them into a Float3 position
            positions.push(Float3 {
                x:*(vertpos as *const f32),
                y:*(vertpos.offset(4) as *const f32),
                z:*(vertpos.offset(8) as *const f32) });
        }

        // if we are computing the normals just push a zero normal.  otherwise, fill in the normal from the data
        if update_normals {
            normals.push(Float3 { x:0.0, y:0.0, z:0.0 });
        } else {
            unsafe {
                let vertpos = data.offset((i * layout.size) as isize + norm_offset as isize);
                match norm_elem.Format {
                    DXGI_FORMAT_R32G32B32_FLOAT => {
                        // skip until I have test data for this
                        return Err(HookError::MeshUpdateFailed("unsupported normal format".to_owned()).into());
                        // normals.push(Float3 {
                        //     x:*(vertpos as *const f32),
                        //     y:*(vertpos.offset(4) as *const f32),
                        //     z:*(vertpos.offset(8) as *const f32) });
                    },
                    DXGI_FORMAT_R32G32B32A32_FLOAT => {
                        return Err(HookError::MeshUpdateFailed("unsupported normal format".to_owned()).into());
                    },
                    DXGI_FORMAT_R8G8B8A8_UNORM => {
                        // the normal is stored as a 4 byte RGBA value where each
                        // component is a byte.  the value is in the range 0-255.  we need to convert it
                        // to a float in the range -1.0 to 1.0
                        // furthermore, MM might have reversed it, so we need to check the reverse flag
                        // the reverse only applies to the first three components, w is always at end.
                        // so basically x and z are swapped if reverse is true.
                        // (don't ask me why games use this format, I just "work here", though it could be a bug in mm)
                        let fval = |f| f / 255.0 * 2.0 - 1.0;
                        if reverse {
                            let x = fval(*(vertpos.offset(2)) as f32);
                            let y = fval(*(vertpos.offset(1)) as f32);
                            let z = fval(*(vertpos.offset(0)) as f32);
                            normals.push(Float3 { x, y, z });
                        } else {
                            let x = fval(*(vertpos) as f32);
                            let y = fval(*(vertpos.offset(1)) as f32);
                            let z = fval(*(vertpos.offset(2)) as f32);
                            normals.push(Float3 { x, y, z });
                        }
                    },
                    _ => {
                        return Err(HookError::MeshUpdateFailed("unsupported normal format".to_owned()).into());
                    }
                }
            }
        }

        if update_tangents {
            // push empty vecs for update
            tangents.push(Float3 { x:0.0, y:0.0, z:0.0 });
            bitangents.push(Float3 { x:0.0, y:0.0, z:0.0 });

            // seek to the tex coord offset and read the u,v values
            let tex_elem = tex_elem.ok_or(HookError::MeshUpdateFailed("missing texcoord in input layout".to_owned()))?;
            unsafe {
                let vertpos = data.offset((i * layout.size) as isize + tex_elem.AlignedByteOffset as isize);
                // support these formats
                match tex_elem.Format {
                    DXGI_FORMAT_R32G32_FLOAT => {
                        texcoords.push(Float2 {
                            x:*(vertpos as *const f32),
                            y:*(vertpos.offset(4) as *const f32) });
                    },
                    DXGI_FORMAT_R16G16_FLOAT => {
                        // convert to f32
                        // TODO: not sure this is right
                        let x = *(vertpos as *const u16) as f32;
                        let y = *(vertpos.offset(2) as *const u16) as f32;
                        texcoords.push(Float2 { x, y });
                    },
                    _ => {
                        return Err(HookError::MeshUpdateFailed("unsupported texcoord format".to_owned()).into());
                    }
                }
            }
        }
    }

    // make sure we didn't eff up
    if positions.len() != vert_count as usize || (update_normals && normals.len() != vert_count as usize) {
        return Err(HookError::MeshUpdateFailed("failed to read vertex data (normal)".to_owned()));
    }
    if update_tangents && (normals.len() != vert_count as usize || tangents.len() != vert_count as usize
        || bitangents.len() != vert_count as usize || texcoords.len() != vert_count as usize) {
        return Err(HookError::MeshUpdateFailed("failed to read vertex data (tangent)".to_owned()));
    }

    // can now compute the normals
    let nfaces = indices.len() / 3;

    if update_normals {
        let ret = unsafe {
            compute_normals_32.ok_or_else(|| HookError::MeshUpdateFailed("failed to find DirectX_ComputeNormals_32".to_owned()))?
                (indices.as_ptr(), nfaces, positions.as_ptr(), positions.len(), flags, normals.as_mut_ptr())
        };

        if ret != S_OK {
            return Err(HookError::MeshUpdateFailed(format!("failed to compute normals: {}", ret)));
        }
    }

    let (tan_elem, bitan_elem) = if update_tangents {
        // make sure we have the tangent and bitangent elements
        let tan_elem = Some(layout.layout.iter()
        .find(|l| ptr_to_str(l.SemanticName).starts_with("tangent"))
        .ok_or(HookError::MeshUpdateFailed("missing tangent in input layout".to_owned()))?);
        let mut bitan_elem = layout.layout.iter()
            .find(|l| ptr_to_str(l.SemanticName).starts_with("bitangent"));
        // check for other name on bitan
        if bitan_elem.is_none() {
            bitan_elem = Some(layout.layout.iter()
                .find(|l| ptr_to_str(l.SemanticName).starts_with("binormal"))
                .ok_or(HookError::MeshUpdateFailed("missing bitangent in input layout".to_owned()))?);
        }
        let ret = unsafe {
            compute_tangentframe_32tb.ok_or_else(|| HookError::MeshUpdateFailed("failed to find DirectX_ComputeTangentFrame_32TB".to_owned()))?
                (indices.as_ptr(), nfaces, positions.as_ptr(), normals.as_ptr(), texcoords.as_ptr(), positions.len(), tangents.as_mut_ptr(), bitangents.as_mut_ptr())
        };
        if ret != S_OK {
            return Err(HookError::MeshUpdateFailed(format!("failed to compute tangents: {}", ret)));
        }
        (tan_elem, bitan_elem)
    } else {
        (None, None)
    };

    // now we need to write the normals back to the original data using the normal offset
    let f_to_u8 = |f:f32| -> u8 {
        //((f + 1.0) * 127.5 + 0.5) as u8
        ((f + 1.0) * 127.5).round() as u8
        //round((floating_point_value + 1) * 127.5)
    };
    let write_vector = |i:u32,elem:&D3D11_INPUT_ELEMENT_DESC, vec:&Float3| unsafe {
        let vertpos = data.offset((i * layout.size) as isize + elem.AlignedByteOffset as isize);
        // handle various formats
        match elem.Format {
            DXGI_FORMAT_R8G8B8A8_UNORM => {
                if reverse {
                    *(vertpos) = f_to_u8(vec.z);
                    *(vertpos.offset(1)) = f_to_u8(vec.y);
                    *(vertpos.offset(2)) = f_to_u8(vec.x);
                    *(vertpos.offset(3)) = 0;
                } else {
                    *(vertpos) = f_to_u8(vec.x);
                    *(vertpos.offset(1)) = f_to_u8(vec.y);
                    *(vertpos.offset(2)) = f_to_u8(vec.z);
                    *(vertpos.offset(3)) = 0;
                }
            },
            DXGI_FORMAT_R32G32B32A32_FLOAT => {
                return Err(HookError::MeshUpdateFailed(format!("unsupported vector format: {}", elem.Format)));
            },
            _ => {
                return Err(HookError::MeshUpdateFailed(format!("unsupported vector format: {}", elem.Format)));
            }
        }

        Ok(())
    };

    for i in 0..vert_count {
        if update_normals {
            write_vector(i, norm_elem, &normals[i as usize])?;
        }

        if update_tangents {
            let tan_elem = tan_elem.ok_or(HookError::MeshUpdateFailed("missing tangent in input layout".to_owned()))?;
            write_vector(i, tan_elem, &tangents[i as usize])?;
            let bitan_elem = bitan_elem.ok_or(HookError::MeshUpdateFailed("missing bitangent in input layout".to_owned()))?;
            write_vector(i, bitan_elem, &bitangents[i as usize])?;
        }
    }

    //write_log_file(&format!("finished updating {}", what));

    Ok(())
}