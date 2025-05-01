

use shared_dx::dx11rs::VertexFormat;
use shared_dx::error;
use shared_dx::error::HookError;
use winapi::shared::dxgiformat::DXGI_FORMAT_R16G16_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32B32A32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32B32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R32G32_FLOAT;
use winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;
pub use winapi::shared::winerror::S_OK;
use winapi::um::d3d11::D3D11_INPUT_ELEMENT_DESC;
pub use winapi::um::winnt::HRESULT;

use util;
use std;
use std::ffi::CStr;

use shared_dx::util::*;

use global_state::GLOBAL_STATE;

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
pub fn update_normals(data:*mut u8, name:&str, mod_ts_update:i32, vert_count:u32, layout:&VertexFormat) -> error::Result<()> {

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

                reverse = util::reg_query_dword(profile_root,"GameProfileReverseNormals",)
                .map(|f| f > 0)
                .map_err(|e| {
                    write_log_file(&format!("using default {:?} for reverse normals: {:?}", reverse, e));
                }).unwrap_or(reverse);

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

                Ok(())
            }
        });
    res?;

    let mod_wants_ts_update = match mod_ts_update {
        0 => Some(false),
        1 => Some(true),
        -1 => None,
        wat => {
            write_log_file(&format!("mod '{}' wants unknown tangent update setting {}", name, wat));
            None
        }
    };

    if let Some(mod_wants_ts_update) = mod_wants_ts_update {
        if mod_wants_ts_update != update_tangents {
            write_log_file(&format!("mod '{}' tangent update setting {} overridding default {}", name, mod_wants_ts_update, update_tangents));
            update_tangents = mod_wants_ts_update;
        }
    }

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
    write_log_file(&format!("mod '{}': updating {}; reverse: {}", name, what, reverse));

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