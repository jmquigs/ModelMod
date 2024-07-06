use device_state::dev_state_d3d11_nolock;
use device_state::dev_state_d3d11_read;
use global_state::get_global_state_ptr;
use global_state::HookState;
use shared_dx::types::DevicePointer;
use types::d3dx::D3DXFn;
use types::interop::D3D11SnapshotRendData;
use types::interop::D3D9SnapshotRendData;
use winapi::Interface;
use winapi::shared::d3d9::*;
use winapi::shared::d3d9types::*;
use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R16_UINT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32_UINT;
use winapi::shared::dxgiformat::DXGI_FORMAT_UNKNOWN;
use winapi::shared::minwindef::{DWORD, UINT, BOOL};

use constant_tracking;
use d3dx;
use global_state::GLOBAL_STATE;
use winapi::um::{d3d11::{D3D11_BIND_RENDER_TARGET, D3D11_BUFFER_DESC, D3D11_INPUT_ELEMENT_DESC,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_TEXTURE2D_DESC, ID3D11Buffer, ID3D11Device,
    ID3D11DeviceContext, ID3D11Resource, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11View}, d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D};

use std;
use std::collections::BTreeMap;
use std::ffi::CStr;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
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
        snap_ms: 250, // TODO11: dx11 needs longer, 5 seconds?
        snap_anim: false,
        snap_anim_on_count: 2,
        // TODO: should read these limits from device, it might support fewer!
        vconsts_to_capture: 256,
        pconsts_to_capture: 224,
        autosnap: None,
        require_gpu: None,
        plugins: None,
        clear_sd_on_reset: false,
    }));

    pub static ref WAS_RESET: AtomicBool = AtomicBool::new(false);
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

pub fn take(devptr:&mut DevicePointer, sd:&mut types::interop::SnapshotData, this_is_selected:bool) {
    if devptr.is_null() {
        return;
    }
    if sd.prim_count == 0 || sd.num_vertices == 0 {
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

    let mut gs_ptr = get_global_state_ptr();
    let gs = gs_ptr.as_mut();
    let autosnap = if let Some(_) = &gs.anim_snap_state {
        auto_snap_anim(devptr, sd, gs, &snap_conf)
    } else {
        false
    };

    let do_snap = (this_is_selected || autosnap) && gs.is_snapping;

    if !do_snap {
        return;
    }

    let pre_rc;
    // snap in a block so that drops within activate and we can check ref count after
    unsafe {
        write_log_file(&format!("==> New snap started: prims: {}, verts: {}, basevert: {}, startindex: {}", sd.prim_count, sd.num_vertices, sd.base_vertex_index, sd.start_index));

        pre_rc = devptr.get_ref_count();

        gs.device = Some(*devptr);

        if gs.d3dx_fn.is_none() {
            d3dx::load_and_set_in_gs(&gs.mm_root, &devptr)
                .map_err(|e| {
                    write_log_file(&format!(
                        "failed to load d3dx: texture snapping not available: {:?}",
                        e
                    ));
                    e
                })
                .ok();
        }

        save_constants(devptr, gs, &snap_conf);

        let save_rs = save_render_state(devptr);

        match set_buffers(devptr, sd) {
            Ok(bufs) => {
                write_log_file(&format!("snapshot data size is: {}", sd.sd_size));
                GLOBAL_STATE.interop_state.as_mut().map(|is| {
                    // If the snapshot state was reset set that flag in sd and clear WAS_RESET
                    sd.clear_sd_on_reset = snap_conf.clear_sd_on_reset;
                    sd.was_reset = WAS_RESET.load(Ordering::Relaxed);
                    if sd.was_reset {
                        WAS_RESET.store(false, Ordering::Relaxed);
                    }
                    

                    // call into managed code to do the a lot of the data writing
                    let cb = is.callbacks;
                    let res = (cb.TakeSnapshot)(devptr.as_c_void(), sd);

                    let sresult = if res == 0 { 
                        Some(*(cb.GetSnapshotResult)())
                    } else {
                        None
                    };

                    let (dir,sprefix) = if let Some(sresult) = sresult {
                        let dir = &sresult.directory[0..(sresult.directory_len as usize)];
                        let sprefix = &sresult.snap_file_prefix[0..(sresult.snap_file_prefix_len as usize)];

                        let dir = String::from_utf16(&dir).unwrap_or_else(|_| "".to_owned());
                        let sprefix = String::from_utf16(&sprefix).unwrap_or_else(|_| "".to_owned());
                        (dir,sprefix)
                    } else {
                        ("".to_owned(),"".to_owned())
                    };

                    let new_dir = match &gs.last_snapshot_dir {
                        None => true,
                        Some(d) if d != &dir => true,
                        Some(_) => false
                    };
                    if new_dir {
                        write_log_file(&format!("snapshot dir updated: {}", new_dir));
                        gs.last_snapshot_dir = Some(dir.clone())
                    }                    

                    if res == 0 && snapshot_extra() {
                        gs.anim_snap_state.as_mut().map(|ass| {
                            if ass.snap_dir == "" {
                                ass.snap_dir = dir.to_owned();
                            }
                        });

                        // write_log_file(&format!("snap save dir: {}", dir));
                        // write_log_file(&format!("snap prefix: {}", sprefix));

                        let _ = save_textures(devptr, &bufs, &dir, &sprefix).map_err(|e| {
                            write_log_file(&format!("failed to save textures: {:?}", e));
                        });

                        let (gotpix,gotvert) = shader_capture::take_snapshot(devptr, &dir, &sprefix);
                        let vc = if gotvert { &gs.vertex_constants } else { &None };
                        let pc = if gotpix { &gs.pixel_constants } else { &None };
                        constant_tracking::take_snapshot(&dir, &sprefix, vc, pc);

                        if save_rs.has_state() {
                            let file = format!("{}/{}_rstate.yaml", &dir, &sprefix);
                            let _r = save_rs.save(&file).map_err(|e| {
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
            },
            Err(e) => {
                write_log_file(&format!("snapshot::take: failed to set buffers: {:?}", e));
            }
        }
        gs.device = None;
    }
    // check for resource leak, we do this in another block so that all the release on
    // drops activated.
    {
        let post_rc = devptr.get_ref_count();
        if pre_rc != post_rc {
            write_log_file(&format!(
                "WARNING: device ref count before snapshot ({}) does not
                equal count after snapshot ({}), likely resources were leaked",
                pre_rc, post_rc
            ));
        }
    }
}

unsafe fn save_textures(devtr:&mut DevicePointer, buffers:&Box<dyn SnapDeviceBuffers>, snap_dir:&str, snap_prefix:&str) -> Result<()> {
    // in d3d9 the managed code already did this
    let device = match devtr {
        DevicePointer::D3D11(d) => *d,
        _ => return Ok(()),
    };
    let d3dx_fn = GLOBAL_STATE
        .d3dx_fn
        .as_ref()
        .ok_or(HookError::SnapshotFailed("d3dx not found".to_owned()))?;
        let d3dx_fn = match d3dx_fn {
            D3DXFn::DX11(d) => d,
            _ => return Err(HookError::SnapshotFailed("wrong d3dx??".to_owned())),
        };
    let mut context:*mut ID3D11DeviceContext = null_mut();
    (*device).GetImmediateContext(&mut context);
    if context.is_null() {
        return Err(HookError::SnapshotFailed("failed to get immediate context".to_string()));
    }
    let _context_rod = ReleaseOnDrop::new(context);
    // downcast the buffers to D3D11SnapDeviceBuffers
    let d3d11bufs = buffers.as_any().downcast_ref::<D3D11SnapDeviceBuffers>()
        .ok_or_else(|| HookError::SnapshotFailed("failed to downcast buffers".to_owned()))?;
    if d3d11bufs.srvs.len() == 0 {
        return Err(HookError::SnapshotFailed(format!("no 2D textures found for snapshot: {}", snap_prefix)));
    }
    let num_2d = d3d11bufs.srv_2d_tex.len();
    // find the srvs that are 2d textures, try to save them
    let mut num_saved = 0;
    let mut num_skipped = 0;
    for idx in d3d11bufs.srv_2d_tex.iter() {
        if *idx as usize >= d3d11bufs.srvs.len() {
            return Err(HookError::SnapshotFailed(format!("invalid srv index {} of {}", idx, d3d11bufs.srvs.len())));
        }
        let srv = d3d11bufs.srvs[*idx as usize];
        if !srv.is_null() {
            let viewptr:*mut ID3D11View = srv as *mut _;
            let mut resptr:*mut ID3D11Resource = null_mut();
            (*viewptr).GetResource(&mut resptr);
            if resptr.is_null() {
                return Err(HookError::SnapshotFailed("failed to get resource from srv".to_string()));
            }
            let _res_rod = ReleaseOnDrop::new(resptr);

            // query interface to get ID3D11Texture2D for more info about it
            let mut texptr:*mut ID3D11Texture2D = null_mut();
            let riid = &ID3D11Texture2D::uuidof();
            let hr = (*resptr).QueryInterface(riid, &mut texptr as *mut *mut _ as *mut *mut c_void);
            if hr != 0 {
                return Err(HookError::SnapshotFailed(format!("failed to query interface for texture: {:x}", hr)));
            }
            let _tex_rod = ReleaseOnDrop::new(texptr);
            // now get description
            let mut desc:D3D11_TEXTURE2D_DESC = std::mem::zeroed();
            (*texptr).GetDesc(&mut desc);
            let heightwidth_format = (desc.Height, desc.Width, desc.Format);
            // skip render targets, these won't snap
            if desc.BindFlags & D3D11_BIND_RENDER_TARGET > 0 {
                num_skipped += 1;
                write_log_file(&format!("Warning: skipping texture {} [{:?}] due to render target binding (bindflags is {})",
                    idx, heightwidth_format, desc.BindFlags));
                continue;
            }
            write_log_file(&format!("tex {} [{:?}] has usage {}, cpu access flags {}, bindflags {}, miscflags {}, format {}",
                idx, heightwidth_format, desc.Usage, desc.CPUAccessFlags, desc.BindFlags, desc.MiscFlags, desc.Format));

            let out = format!("{}/{}_texture{}.dds", snap_dir, snap_prefix, idx);
            let out = util::to_wide_str(&out);
            const D3DX11_IFF_DDS: u32 = 4;
            let hr = (d3dx_fn.D3DX11SaveTextureToFileW)(context, resptr, D3DX11_IFF_DDS, out.as_ptr());
            if hr != 0 {
                // write out an error but keep going to see if we can get others
                write_log_file(&format!("failed to save texture from srv {}: {}", idx, hr));
            } else {
                num_saved += 1;
            }
        }
    }
    if num_2d > 0 && num_2d == (num_saved + num_skipped) {
        write_log_file(&format!("wrote {} textures for snapshot {}", num_saved, &snap_prefix));
        Ok(())
    } else {
        Err(HookError::SnapshotFailed(
            format!("failed to save some textures for snapshot: {}; {} of {} successfully saved, {} skipped", &snap_prefix, num_saved, num_2d, num_skipped)))
    }
}

fn auto_snap_anim(devptr:&mut DevicePointer, sd:&mut types::interop::SnapshotData, gs:&mut HookState, snap_conf:&SnapConfig) -> bool {
    let device = match devptr {
        DevicePointer::D3D9(d) => *d as *mut _,
        | _ => {
            write_log_file("auto_snap_anim: not a d3d9 device");
            return false;
        }
    };
    let mut autosnap = false;

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

    return autosnap;

}

fn save_constants(devptr:&mut DevicePointer, gs:&mut HookState, snap_conf:&SnapConfig) {
    let device = match devptr {
        &mut DevicePointer::D3D9(device) => device,
        &mut DevicePointer::D3D11(_) => {
            return;
        },
    };
    // constant tracking workaround: read back all the constants
    if constant_tracking::is_enabled() {
        gs.vertex_constants.as_mut().map(|vconsts| {
            set_vconsts(device, snap_conf.vconsts_to_capture, vconsts, true);
        });
        gs.pixel_constants.as_mut().map(|pconsts| {
            set_pconsts(device, snap_conf.pconsts_to_capture, pconsts);
        });
    }
}

trait SnapRendState {
    fn has_state(&self) -> bool;
    fn save(self: Box<Self>, file:&str) -> Result<()>;
}

struct D3D9SnapRenderState {
    blendstates: BTreeMap<DWORD, DWORD>,
    tstagestates: Vec<BTreeMap<DWORD, DWORD>>,
}
impl SnapRendState for D3D9SnapRenderState {
    fn has_state(&self) -> bool {
        self.blendstates.len() > 0
    }
    fn save(self: Box<Self>, file:&str) -> Result<()> {
        if self.has_state() {
            write_obj_to_file(&file, false, &RenderStateMap {
                blendstates: self.blendstates,
                tstagestates: self.tstagestates,
            })
        } else {
            Ok(())
        }
    }
}

unsafe fn save_render_state_d3d9(device:*mut IDirect3DDevice9) -> Box<dyn SnapRendState> {
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

    Box::new(D3D9SnapRenderState {
        blendstates,
        tstagestates,
    })
}

struct D3D11SnapRenderState {}
impl SnapRendState for D3D11SnapRenderState {
    fn has_state(&self) -> bool {
        false
    }
    fn save(self: Box<Self>, _file:&str) -> Result<()> {
        Ok(())
    }
}

fn save_render_state_d3d11() -> Box<dyn SnapRendState> {
    Box::new(D3D11SnapRenderState{})
}
fn save_render_state(devptr:&mut DevicePointer) -> Box<dyn SnapRendState> {
    match devptr {
        &mut DevicePointer::D3D9(device) => unsafe { save_render_state_d3d9(device) },
        &mut DevicePointer::D3D11(_) => save_render_state_d3d11(),
    }
}

use std::any::Any;

trait SnapDeviceBuffers {
    fn as_any(&self) -> &dyn Any;
}

struct D3D9SnapDeviceBuffers {
    _index_buffer: *mut IDirect3DIndexBuffer9,
    _vert_decl: *mut IDirect3DVertexDeclaration9,
    _ib_rod: ReleaseOnDrop<*mut IDirect3DIndexBuffer9>,
    _vert_decl_rod: ReleaseOnDrop<*mut IDirect3DVertexDeclaration9>,
}

impl SnapDeviceBuffers for D3D9SnapDeviceBuffers{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe fn set_buffers_d3d9(device:*mut IDirect3DDevice9, sd:&mut types::interop::SnapshotData) -> Result<Box<dyn SnapDeviceBuffers>> {
    let mut vert_decl: *mut IDirect3DVertexDeclaration9 = null_mut();
    // sharpdx does not expose GetVertexDeclaration, so need to do it here
    let hr = (*device).GetVertexDeclaration(&mut vert_decl);

    if hr != 0 {
        write_log_file(&format!(
            "Error, can't get vertex declaration.
            Cannot snap; HR: {:x}",
            hr
        ));
        return Err(HookError::SnapshotFailed("failed snapshot".to_string()));
    }
    let vert_decl_rod = ReleaseOnDrop::new(vert_decl);

    let mut ib: *mut IDirect3DIndexBuffer9 = null_mut();
    let hr = (*device).GetIndices(&mut ib);
    if hr != 0 {
        write_log_file(&format!(
            "Error, can't get index buffer.  Cannot snap; HR: {:x}",
            hr
        ));
        return Err(HookError::SnapshotFailed("failed snapshot".to_string()));
    }
    let ib_rod = ReleaseOnDrop::new(ib);

    // fill in snap data
    sd.rend_data.d3d9 = D3D9SnapshotRendData::from(vert_decl, ib);
    //write_log_file(&format!("rend data: {:?}", sd.rend_data.d3d9));

    Ok(Box::new(D3D9SnapDeviceBuffers {
        _index_buffer: ib,
        _vert_decl: vert_decl,
        _ib_rod: ib_rod,
        _vert_decl_rod: vert_decl_rod,
    }))
}

const MAX_SRV: u32 = 32;

struct D3D11SnapDeviceBuffers {
    pub _ld:Vec<D3D11_INPUT_ELEMENT_DESC>,
    pub _context_rod:ReleaseOnDrop<*mut ID3D11DeviceContext>,
    pub _ib_data:Vec<u8>,
    pub _vb_data:Vec<u8>,
    pub srvs:[*mut ID3D11ShaderResourceView; MAX_SRV as usize],
    pub _srv_rods:Vec<ReleaseOnDrop<*mut ID3D11ShaderResourceView>>,
    pub srv_2d_tex:Vec<u32>,
}

impl SnapDeviceBuffers for D3D11SnapDeviceBuffers{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe fn set_buffers_d3d11(device:*mut ID3D11Device, sd:&mut types::interop::SnapshotData) -> Result<Box<dyn SnapDeviceBuffers>> {
    if let Some(state) = dev_state_d3d11_nolock() {
        let vf = state.rs.get_current_vertex_format().ok_or_else(|| {
            HookError::SnapshotFailed("no current input layout, cannot snap".to_string())
        })?;
        let vert_size = vf.size as usize;
        if vert_size == 0 {
            return Err(HookError::SnapshotFailed("vertex size is 0".to_string()));
        }
        // abort if it lacks a texture coordinate semantic, this won't snap properly ATM (might
        // be a python importer error in mmobj, not sure)
        let ptr_to_str = |ptr:*const i8| -> String {
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let s = cstr.to_string_lossy().to_ascii_lowercase().to_string();
            s
        };
        vf.layout.iter()
            .find(|l| ptr_to_str(l.SemanticName).starts_with("texcoord"))
            .ok_or(HookError::SnapshotFailed("snap aborted, vertex layout lacks texcoord so this will not capture properly".to_owned()))?;

        let mut ld = vf.layout.clone();
        let layout_data_size = std::mem::size_of::<D3D11_INPUT_ELEMENT_DESC>() * ld.len();
        let decl_data = ld.as_mut_ptr();

        let mut context:*mut ID3D11DeviceContext = null_mut();
        (*device).GetImmediateContext(&mut context);
        if context.is_null() {
            return Err(HookError::SnapshotFailed("failed to get immediate context".to_string()));
        }
        let context_rod = ReleaseOnDrop::new(context);

        // and we'll need the index buffer
        let mut curr_ibuffer: *mut ID3D11Buffer = null_mut();
        let mut curr_ibuffer_offset: UINT = 0;
        let mut curr_ibuffer_format: DXGI_FORMAT = DXGI_FORMAT_UNKNOWN;
        (*context).IAGetIndexBuffer(&mut curr_ibuffer, &
            mut curr_ibuffer_format, &mut curr_ibuffer_offset);
        if curr_ibuffer.is_null() {
            return Err(HookError::SnapshotFailed("failed to get index buffer".to_string()));
        }
        // create the ib_rod but actually we don't need to save it outside of this func
        let _ib_rod = ReleaseOnDrop::new(curr_ibuffer);

        // determine if we have the data for the buffer since at this time we can't read it
        // directly via Map
        let ib_copy = dev_state_d3d11_read()
            .map(|(_lock,ds)| {
                ds.rs.device_index_buffer_data.get(&(curr_ibuffer as usize)).map(|v| v.clone())
            }).flatten()
            .ok_or_else(|| {
                HookError::SnapshotFailed("failed to get index buffer data, was not previously saved".to_string())
            })?;

        // determine if 16 or 32 bit indices
        let mut ib_desc:D3D11_BUFFER_DESC = std::mem::zeroed();
        (*curr_ibuffer).GetDesc(&mut ib_desc);
        let index_size = match curr_ibuffer_format {
            DXGI_FORMAT_R16_UINT => 2,
            DXGI_FORMAT_R32_UINT => 4,
            _ => return Err(HookError::SnapshotFailed(format!("unknown index buffer format: {:x}", curr_ibuffer_format))),
        };

        // should match expected size
        let ex_size = (sd.prim_count * 3 * index_size) as usize;
        if ib_copy.len() != ex_size {
            return Err(HookError::SnapshotFailed(format!("index buffer data size mismatch, expected: {}, got: {}", ex_size, ib_copy.len())));
        }

        write_log_file(&format!("index buffer size: {}, format: {}", ib_copy.len(), curr_ibuffer_format));

        // now same for vertex buffers
        const MAX_VBUFFERS: usize = 16;
        let mut curr_vbuffers: [*mut ID3D11Buffer; MAX_VBUFFERS] = [null_mut(); MAX_VBUFFERS];
        let mut curr_vbuffer_strides: [UINT; MAX_VBUFFERS] = [0; MAX_VBUFFERS];
        let mut curr_vbuffer_offsets: [UINT; MAX_VBUFFERS] = [0; MAX_VBUFFERS];
        (*context).IAGetVertexBuffers(0, MAX_VBUFFERS as u32,
            curr_vbuffers.as_mut_ptr(),
            curr_vbuffer_strides.as_mut_ptr(),
            curr_vbuffer_offsets.as_mut_ptr());
        let _vb_rods =
            curr_vbuffers.iter().filter(|vb| !vb.is_null())
             .map(|vb| ReleaseOnDrop::new(*vb)).collect::<Vec<_>>();
        // filter active
        let curr_vbuffers = curr_vbuffers.iter().filter(|vb| !vb.is_null()).collect::<Vec<_>>();
        if curr_vbuffers.is_empty() {
            return Err(HookError::SnapshotFailed("no vertex buffers".to_string()));
        }
        if curr_vbuffers.len() > 1 {
            return Err(HookError::SnapshotFailed(format!("more than 1 vertex buffer not supported (got {})", curr_vbuffers.len())));
        }
        // copy the data
        let vb_copy = dev_state_d3d11_read()
            .map(|(_lock,ds)| {
                let vb_usize = *curr_vbuffers[0] as usize;
                ds.rs.device_vertex_buffer_data.get(&vb_usize).map(|v| v.clone())
            }).flatten()
            .ok_or_else(|| {
                HookError::SnapshotFailed("failed to get vertex buffer data, was not previously saved".to_string())
            })?;
        // number of vertices should be = size / vert size
        let num_verts = vb_copy.len() / vert_size;
        if sd.num_vertices != num_verts as u32 {
            return Err(HookError::SnapshotFailed(format!("vertex buffer data size mismatch, expected: {}, got: {}", sd.num_vertices, num_verts)));
        }
        write_log_file(&format!("vertex buffer size: {}, num verts: {}, vertsize: {}", vb_copy.len(), num_verts, vert_size));

        // now save all the srvs that might contain textures, note any that are 2D and save the
        // indexes of those so that managed code has them
        let _srv_rods;

        let mut orig_srvs: [*mut ID3D11ShaderResourceView; MAX_SRV as usize] = [null_mut(); MAX_SRV as usize];
        (*context).PSGetShaderResources(0, MAX_SRV, orig_srvs.as_mut_ptr());
        _srv_rods = orig_srvs.iter().filter(|srv| !srv.is_null())
            .map(|srv| ReleaseOnDrop::new(*srv)).collect::<Vec<_>>();

        let mut tex_indices: Vec<u32> = Vec::new();
        for (idx,srv) in orig_srvs.iter_mut().enumerate() {
            if !srv.is_null() {
                let mut desc: D3D11_SHADER_RESOURCE_VIEW_DESC = std::mem::zeroed();
                (**srv).GetDesc(&mut desc);
                if desc.ViewDimension == D3D11_SRV_DIMENSION_TEXTURE2D {
                    tex_indices.push(idx.try_into()?);
                }
            }
        }

        sd.rend_data.d3d11 = D3D11SnapshotRendData {
            layout_elems: decl_data,
            layout_size_bytes: layout_data_size as u64,
            ib_data: ib_copy.as_ptr(),
            vb_data: vb_copy.as_ptr(),
            ib_size_bytes: ib_copy.len() as u64,
            vb_size_bytes: vb_copy.len() as u64,
            ib_index_size_bytes: index_size as u32,
            vb_vert_size_bytes: vert_size as u32,
            act_tex_indices: tex_indices.as_ptr(),
            num_act_tex_indices: tex_indices.len().try_into()?,
        };

        return Ok(Box::new(D3D11SnapDeviceBuffers{
            _context_rod: context_rod,
            _ld: ld,
            _ib_data: ib_copy,
            _vb_data: vb_copy,
            srvs: orig_srvs,
            srv_2d_tex: tex_indices,
            _srv_rods,
        }))
    }
    Err(HookError::SnapshotFailed("set_buffers_d3d11: failed snapshot".to_string()))
}

fn set_buffers(devptr:&mut DevicePointer, sd:&mut types::interop::SnapshotData) -> Result<Box<dyn SnapDeviceBuffers>> {
    match devptr {
        &mut DevicePointer::D3D9(device) => unsafe { set_buffers_d3d9(device, sd) },
        &mut DevicePointer::D3D11(device) => unsafe { set_buffers_d3d11(device, sd) },
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

    let mut gs_ptr = get_global_state_ptr();
    let gs = gs_ptr.as_mut();

    if gs.is_snapping {
        let now = SystemTime::now();
        let max_dur = std::time::Duration::from_millis(snap_ms as u64);
        let elapsed = now
            .duration_since(gs.snap_start)
            .unwrap_or(max_dur);
        if elapsed >= max_dur {
            write_log_file("ending snapshot");
            if let Some(dir) = &gs.last_snapshot_dir {
                let out_file = format!("{}/MMSnapshotComplete.txt", dir);
                let out_file = &out_file;
                let write_ss_complete = || -> std::io::Result<()> {
                    use std::io::Write;
                    let mut file = std::fs::File::create(out_file)?;
                    file.write_all(format!("Snap duration: {:?}. This file is updated at the end of each snapshot.", elapsed).as_bytes())
                };
                write_ss_complete().unwrap_or_else(|e| 
                    write_log_file(&format!("failed to write {}: {:?}", out_file, e)));
            }
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
    WAS_RESET.store(true, std::sync::atomic::Ordering::Relaxed);
}