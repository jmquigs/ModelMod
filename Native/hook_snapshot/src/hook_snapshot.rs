use shared_dx::types::DevicePointer;
use winapi::shared::d3d9::*;
use winapi::shared::d3d9types::*;
use winapi::shared::minwindef::{DWORD, UINT, BOOL};

use constant_tracking;
use d3dx;
use global_state::{GLOBAL_STATE};

use std;
use std::ptr::null_mut;
use std::time::SystemTime;

use shared_dx::util::*;
use shared_dx::error::*;

use snaplib::snap_config::{SnapConfig};
use snaplib::anim_frame::{AnimFrame, AnimFrameFile};
use snaplib::anim_frame::RenderStateMap;
use snaplib::anim_frame::write_obj_to_file;
use snaplib::anim_snap_state::AnimSnapState;

use std::collections::HashMap;

use std::sync::RwLock;
use std::sync::Arc;

lazy_static! {
    // Snapshotting currently stops after a certain amount of real time has passed from the start of
    // the snap, specified by this constant.
    // One might expect that just snapping everything drawn within a single begin/end scene combo is
    // sufficient, but this often misses data,
    // and sometimes fails to snapshot anything at all.  This may be because the game is using multiple
    // begin/end combos, so maybe
    // present->present would be more reliable (TODO: check this)
    // Using a window makes it much more likely that something useful is captured, at the expense of
    // some duplicates; even though
    // some objects may still be missed.  Some investigation to make this more reliable would be useful.

    pub static ref SNAP_CONFIG: Arc<RwLock<SnapConfig>> = Arc::new(RwLock::new(SnapConfig {
        snap_ms: 250,
        snap_anim: false,
        snap_anim_on_count: 2,
        // TODO: should read these limits from device, it might support fewer!
        vconsts_to_capture: 256,
        pconsts_to_capture: 224,
        autosnap: None,
        require_gpu: None,
        plugins: None,
    }));
}

fn snapshot_extra() -> bool {
    return constant_tracking::is_enabled() || shader_capture::is_enabled()
}

fn set_vconsts(device: *mut IDirect3DDevice9, num_to_read:usize, vconsts: &mut constant_tracking::ConstantGroup, includeIntsBools:bool) {
    let mut dest:Vec<f32> = vec![];
    dest.resize_with(num_to_read * 4, || Default::default());
    unsafe { (*device).GetVertexShaderConstantF(0, dest.as_mut_ptr(), num_to_read as u32); }
    vconsts.floats.set(0, dest.as_ptr(), num_to_read as u32);
    if includeIntsBools {
        let mut dest:Vec<i32> = vec![];
        dest.resize_with(num_to_read * 4, || Default::default());
        unsafe { (*device).GetVertexShaderConstantI(0, dest.as_mut_ptr(), num_to_read as u32); }
        vconsts.ints.set(0, dest.as_ptr(), num_to_read as u32);
        let mut dest:Vec<BOOL> = vec![];
        dest.resize_with(num_to_read, || Default::default());
        unsafe { (*device).GetVertexShaderConstantB(0, dest.as_mut_ptr(), num_to_read as u32); }
        vconsts.bools.set(0, dest.as_ptr(), num_to_read as u32);
    }
}

fn set_pconsts(device: *mut IDirect3DDevice9, num_to_read:usize, pconsts: &mut constant_tracking::ConstantGroup) {
    let mut dest:Vec<f32> = vec![];
    dest.resize_with(num_to_read * 4, || Default::default());
    unsafe { (*device).GetPixelShaderConstantF(0, dest.as_mut_ptr(), num_to_read as u32); }
    pconsts.floats.set(0, dest.as_ptr(), num_to_read as u32);
    let mut dest:Vec<i32> = vec![];
    dest.resize_with(num_to_read * 4, || Default::default());
    unsafe { (*device).GetPixelShaderConstantI(0, dest.as_mut_ptr(), num_to_read as u32); }
    pconsts.ints.set(0, dest.as_ptr(), num_to_read as u32);
    let mut dest:Vec<BOOL> = vec![];
    dest.resize_with(num_to_read, || Default::default());
    unsafe { (*device).GetPixelShaderConstantB(0, dest.as_mut_ptr(), num_to_read as u32); }
    pconsts.bools.set(0, dest.as_ptr(), num_to_read as u32);
}

pub fn take(device:*mut IDirect3DDevice9, sd:&mut types::interop::SnapshotData, this_is_selected:bool) {
    if device == null_mut() {
        return;
    }
    let snap_conf =
        match SNAP_CONFIG.read() {
            Err(e) => {
                write_log_file(&format!("failed to lock snap config: {}", e));
                SnapConfig::new()
            },
            Ok(c) => c.clone()
        };

    let mut autosnap = false;

    let gs = unsafe {&mut GLOBAL_STATE };

    // experimental animation snapping. not normally used and no modding support at this time.
    // VERY game and even model specific, hard to generalize.
    if gs.anim_snap_state.is_some() {
        let ass = gs.anim_snap_state.as_mut().unwrap();
        let primvert = &(sd.prim_count,sd.num_vertices);
        if gs.metrics.total_frames > ass.curr_frame {
            ass.curr_frame = gs.metrics.total_frames;
            ass.capture_count_this_frame.clear();
        }
        if ass.expected_primverts.contains(primvert) {
            let cap_count = ass.capture_count_this_frame.entry(*primvert).or_insert(0);
            *cap_count += 1;

            // ignore it if it isn't the target snap count
            if *cap_count != snap_conf.snap_anim_on_count {
                ();
            }
            else if ass.seen_all {
                if ass.start_frame == 0 {
                    ass.start_frame = ass.curr_frame
                }

                if ass.next_vconst_idx == 0 {
                    ass.sequence_start_time = SystemTime::now();
                }
                // seen everything once, so we can start snapping the constants now
                if ass.next_vconst_idx >= ass.sequence_vconstants.len() {
                    write_log_file("too many constant captures!");
                } else {
                    let next = &mut ass.sequence_vconstants[ass.next_vconst_idx];
                    set_vconsts(device, snap_conf.vconsts_to_capture, &mut next.constants, false);
                    // (*THIS).GetTransform(D3DTS_WORLD, std::mem::transmute(next.worldmat.m.as_mut_ptr()));
                    // (*THIS).GetTransform(D3DTS_VIEW, std::mem::transmute(next.viewmat.m.as_mut_ptr()));
                    // (*THIS).GetTransform(D3DTS_PROJECTION, std::mem::transmute(next.projmat.m.as_mut_ptr()));
                    next.snapped_at = SystemTime::now();
                    next.prim_count = sd.prim_count;
                    next.vert_count = sd.num_vertices;
                    next.sequence = ass.next_vconst_idx;
                    next.frame = ass.curr_frame;
                    next.capture_count = *cap_count;
                    ass.next_vconst_idx += 1;
                    //unsafe {
                        //this was where I would call into the external toolbox app to get the
                        //player transform.  I removed this module because it was game-specific
                        //and not generalized, but the code still exists in the gamesnap branch.
                        // TBSTATE.as_mut().map(|tbstate| {
                        //     next.player_transform = tbstate.get_player_transform();
                        // });
                    //}
                }
            }
            else if !ass.seen_primverts.contains(primvert) {
                // this is a match and we haven't seen it yet, do a full snap
                autosnap = true;
                ass.seen_primverts.insert(*primvert);
                if ass.expected_primverts.len() == ass.seen_primverts.len() {
                    ass.seen_all = true;
                }
            }
        }
    }

    let do_snap = (this_is_selected || autosnap) && gs.is_snapping;

    if !do_snap {
        return;
    }

    let pre_rc;
    // snap in a block so that drops within activate and we can check ref count after
    unsafe {
        write_log_file("Snap started");

        (*device).AddRef();
        pre_rc = (*device).Release();

        gs.device = Some(device);

        if gs.d3dx_fn.is_none() {
            let dp = DevicePointer::D3D9(device);
            gs.d3dx_fn = d3dx::load_lib(&gs.mm_root, &dp)
                .map_err(|e| {
                    write_log_file(&format!(
                        "failed to load d3dx: texture snapping not available: {:?}",
                        e
                    ));
                    e
                })
                .ok();
        }
        // constant tracking workaround: read back all the constants
        if constant_tracking::is_enabled() {
            gs.vertex_constants.as_mut().map(|vconsts| {
                set_vconsts(device, snap_conf.vconsts_to_capture, vconsts, true);
            });
            gs.pixel_constants.as_mut().map(|pconsts| {
                set_pconsts(device, snap_conf.pconsts_to_capture, pconsts);
            });
        }

        use std::collections::BTreeMap;
        let mut blendstates: BTreeMap<DWORD, DWORD> = BTreeMap::new();
        let mut tstagestates: Vec<BTreeMap<DWORD, DWORD>> = vec![];

        let mut save_state = |statename| {
            let mut state = 0;
            (*device).GetRenderState(statename, &mut state);
            blendstates.insert(statename, state);
        };

        save_state(D3DRS_CULLMODE);

        save_state(D3DRS_ALPHABLENDENABLE);
        save_state(D3DRS_SRCBLEND);
        save_state(D3DRS_DESTBLEND);
        save_state(D3DRS_BLENDOP);
        save_state(D3DRS_SEPARATEALPHABLENDENABLE);
        save_state(D3DRS_SRCBLENDALPHA);
        save_state(D3DRS_DESTBLENDALPHA);
        save_state(D3DRS_BLENDOPALPHA);
        save_state(D3DRS_ALPHATESTENABLE);
        save_state(D3DRS_ALPHAFUNC);
        save_state(D3DRS_ALPHAREF);
        save_state(D3DRS_COLORWRITEENABLE);

        tstagestates.resize_with(4, BTreeMap::new);
        let mut save_state = |tex, statename| {
            let mut state = 0;
            (*device).GetTextureStageState(tex, statename, &mut state);
            let tex = tex as usize;
            if tex >= tstagestates.len() {
                return;
            }
            tstagestates[tex].insert(statename, state);
        };

        for tex in (0..4).rev() {
            save_state(tex, D3DTSS_COLOROP);
            save_state(tex, D3DTSS_COLORARG1);
            save_state(tex, D3DTSS_COLORARG2);
            save_state(tex, D3DTSS_COLORARG0);
            save_state(tex, D3DTSS_ALPHAOP);
            save_state(tex, D3DTSS_ALPHAARG1);
            save_state(tex, D3DTSS_ALPHAARG2);
            save_state(tex, D3DTSS_ALPHAARG0);
            save_state(tex, D3DTSS_TEXTURETRANSFORMFLAGS);
            save_state(tex, D3DTSS_RESULTARG);
        }

        // TODO: warn about active streams that are in use but not supported
        let mut blending_enabled: DWORD = 0;
        let hr = (*device).GetRenderState(D3DRS_INDEXEDVERTEXBLENDENABLE, &mut blending_enabled);
        if hr == 0 && blending_enabled > 0 {
            write_log_file("WARNING: vertex blending is enabled, this mesh may not be supported");
        }

        let mut ok = true;
        let mut vert_decl: *mut IDirect3DVertexDeclaration9 = null_mut();
        // sharpdx does not expose GetVertexDeclaration, so need to do it here
        let hr = (*device).GetVertexDeclaration(&mut vert_decl);

        if hr != 0 {
            write_log_file(&format!(
                "Error, can't get vertex declaration.
                Cannot snap; HR: {:x}",
                hr
            ));
        }
        let _vert_decl_rod = ReleaseOnDrop::new(vert_decl);

        ok = ok && hr == 0;
        let mut ib: *mut IDirect3DIndexBuffer9 = null_mut();
        let hr = (*device).GetIndices(&mut ib);
        if hr != 0 {
            write_log_file(&format!(
                "Error, can't get index buffer.  Cannot snap; HR: {:x}",
                hr
            ));
        }
        let _ib_rod = ReleaseOnDrop::new(ib);

        ok = ok && hr == 0;

        if ok {
            // fill in remaining snap data
            sd.vert_decl = vert_decl;
            sd.index_buffer = ib;

            write_log_file(&format!("snapshot data size is: {}", sd.sd_size));
            GLOBAL_STATE.interop_state.as_mut().map(|is| {
                let cb = is.callbacks;
                let res = (cb.TakeSnapshot)(device, sd);
                if res == 0 && snapshot_extra() {
                    let sresult = *(cb.GetSnapshotResult)();
                    let dir = &sresult.directory[0..(sresult.directory_len as usize)];
                    let sprefix = &sresult.snap_file_prefix[0..(sresult.snap_file_prefix_len as usize)];

                    let dir = String::from_utf16(&dir).unwrap_or_else(|_| "".to_owned());

                    gs.anim_snap_state.as_mut().map(|ass| {
                        if ass.snap_dir == "" {
                            ass.snap_dir = dir.to_owned();
                        }
                    });
                    let sprefix = String::from_utf16(&sprefix).unwrap_or_else(|_| "".to_owned());
                    // write_log_file(&format!("snap save dir: {}", dir));
                    // write_log_file(&format!("snap prefix: {}", sprefix));
                    let (gotpix,gotvert) = shader_capture::take_snapshot(&dir, &sprefix);
                    let vc = if gotvert { &gs.vertex_constants } else { &None };
                    let pc = if gotpix { &gs.pixel_constants } else { &None };
                    constant_tracking::take_snapshot(&dir, &sprefix, vc, pc);

                    if blendstates.len() > 0 {
                        let file = format!("{}/{}_rstate.yaml", &dir, &sprefix);
                        let _r = write_obj_to_file(&file, false, &RenderStateMap {
                            blendstates,
                            tstagestates,
                        }).map_err(|e| {
                            write_log_file(&format!("failed to snap blend states: {:?}", e));
                        });
                    }

                        // for animations, validate that shader is GPU animated
                        if snap_conf.snap_anim || snap_conf.require_gpu == Some(true) {
                            use std::path::Path;
                            let file = format!("{}/{}_vshader.asm", &dir, &sprefix);
                            if Path::new(&file).exists() {
                                use std::fs;

                                fs::read_to_string(&file).map_err(|e| {
                                    write_log_file(&format!("failed to read shader asm after snap: {:?}", e));
                                }).map(|contents| {
                                    if contents.contains("m4x4 oPos, v0, c0") {
                                        write_log_file("=======> error: shader contains simple position multiply, likely not gpu animated, aborting snap.  you must not snap an animation or set require_cpu to false in the conf to snap this mesh");
                                        write_log_file(&format!("file: {}", &file));
                                        gs.is_snapping = false;
                                        gs.anim_snap_state = None;
                            }
                            }).unwrap_or_default();
                        }
                    }
                }
            });
        }
        gs.device = None;
    }
    // check for resource leak, we do this in another block so that all the release on
    // drops activated.
    unsafe {
        (*device).AddRef();
        let post_rc = (*device).Release();
        if pre_rc != post_rc {
            write_log_file(&format!(
                "WARNING: device ref count before snapshot ({}) does not
                equal count after snapshot ({}), likely resources were leaked",
                pre_rc, post_rc
            ));
        }
    }
}

fn write_anim_snap_state(ass:&AnimSnapState) -> Result<()> {
    if ass.snap_dir == "" {
        return Err(HookError::SnapshotFailed("oops snap_dir is empty".to_owned()));
    }
    if !ass.seen_all {
        return Err(HookError::SnapshotFailed("error, not all expected primvert combos were seen!".to_owned()));
    }
    let snap_on_count = match SNAP_CONFIG.read() {
        Err(e) => {
            return Err(HookError::SnapshotFailed(format!("failed to lock snap config: {}", e)))
        },
        Ok(c) => c.snap_anim_on_count
    };

    let mut frames_by_mesh:HashMap<(UINT,UINT), AnimFrameFile> = HashMap::new();

    for sidx in 0..ass.next_vconst_idx {
        let aseq = &ass.sequence_vconstants[sidx];
        let frame = aseq.frame - ass.start_frame;
        // discard results from the first frame (always partial because the first is the meshes).
        if frame == 0 {
            continue;
        }
        // for players: 1st render is shadow or something.  2nd is normal model.  3rd
        // is inventory previews.  might be more in some cases (reflections, etc)
        if aseq.capture_count != snap_on_count {
            continue;
        }

        // get the frame file for this frame
        let frame_file = frames_by_mesh.entry((aseq.prim_count, aseq.vert_count))
            .or_insert_with(AnimFrameFile::new);

        let pxform = match aseq.player_transform.as_ref() {
            Err(e) => {
                return Err(HookError::SnapshotFailed(
                    format!("player transform not available at frame {}, aborting constant write (error: {:?})", frame, e)
                ));
            },
            Ok(xfrm) => {
                xfrm
            }
        };
        let mut pxform = pxform.split(" ");
        // meh https://stackoverflow.com/questions/31046763/does-rust-have-anything-like-scanf
        let parse_next = |split: &mut std::str::Split<&str>| -> Result<f32> {
            let res = split.next().ok_or(HookError::SnapshotFailed("Failed transform parse".to_owned()))?;
            Ok(res.parse().map_err(|_| HookError::SnapshotFailed("failed to parse float".to_owned()))?)
        };
        let x = parse_next(&mut pxform)?;
        let y = parse_next(&mut pxform)?;
        let z = parse_next(&mut pxform)?;
        let rot = parse_next(&mut pxform)?;
        let pxform = constant_tracking::vecToVec4(&vec![x,y,z,rot], 0);
        let framedata = AnimFrame {
            snapped_at: aseq.snapped_at,
            floats: aseq.constants.floats.get_as_btree(),
            transform1: Some(pxform),
            transform2: None,
            transform3: None,
            transform4: None,
        };
        frame_file.frames.push(framedata);
    }
    let anim_dir = &ass.snap_dir;

    // note, capture count ignored since now we only capture only one set of constants for
    // each mesh per frame
    for ((prims,verts), frame_file) in frames_by_mesh {
        let out_file = format!("{}/animframes_{}p_{}v.dat", anim_dir, prims, verts);
        frame_file.write_to_file(&out_file)?;
    }
    write_log_file("wrote anim sequences");
    Ok(())
}

pub fn present_process() {
    let snap_ms = match SNAP_CONFIG.read() {
        Err(e) => {
            write_log_file(&format!("failed to lock snap config: {}", e));
            0
        },
        Ok(c) => c.snap_ms
    };

    let gs = unsafe { &mut GLOBAL_STATE };

    if gs.is_snapping {
        let now = SystemTime::now();
        let max_dur = std::time::Duration::from_millis(snap_ms as u64);
        if now
            .duration_since(gs.snap_start)
            .unwrap_or(max_dur)
            >= max_dur
        {
            write_log_file("ending snapshot");
            gs.is_snapping = false;
            gs.anim_snap_state.as_ref().map(|ass| {
                let duration = now.duration_since(ass.sequence_start_time).unwrap_or_default();
                write_log_file(&format!("captured {} anim constant sequences in {}ms", ass.next_vconst_idx, duration.as_millis()));
                write_anim_snap_state(ass)
                .unwrap_or_else(|e| write_log_file(&format!("failed to write anim state: {:?}", e)));
            });
            gs.anim_snap_state = None;
        }
    }
}

/// Called when the clear texture key is pressed, and when a new snapshot is started.
pub fn reset() {
    // this used to load/init the snapshot toolbox (removed)
}