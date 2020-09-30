#![allow(non_snake_case)]

// TODO remove this, prefer separate constant_tracking crate

// use winapi::shared::d3d9::*;
// use winapi::shared::d3d9types::*;
use winapi::shared::minwindef::*;
//use winapi::um::winnt::{HRESULT, LPCWSTR};

//use hookd3d9::{ dev_state, GLOBAL_STATE };
use shared_dx9::error::*;
use shared_dx9::util;

// use std::collections::HashMap;
use serde::{Serialize};

use constant_tracking::*;

use std::collections::BTreeMap;
#[derive(Serialize)]
pub struct RenderStateMap {
    pub blendstates: BTreeMap<DWORD, DWORD>,
    pub tstagestates: Vec<BTreeMap<DWORD, DWORD>>,
}

pub fn write_obj_to_file<T>(name:&str, binary:bool, what:&T) -> Result<()>
where T: Serialize {
    let ystr:String;
    let bvec:Vec<u8>;
    let bytes = if binary {
        bvec = bincode::serialize(what).map_err(|e| {
            HookError::SerdeError(format!("Serialization error: {:?}", e))
        })?;
        &bvec
    } else {
        ystr = serde_yaml::to_string(what).map_err(|e| {
            HookError::SerdeError(format!("Serialization error: {:?}", e))
        })?;
        ystr.as_bytes()
    };
    use std::io::Write;
    let mut file = std::fs::File::create(name)?;
    file.write_all(bytes)?;
    Ok(())
}

#[derive(Serialize)]
pub struct AnimFrame {
    pub snapped_at: std::time::SystemTime,
    pub floats: std::collections::BTreeMap<UINT, Vec4<f32>>,
    pub transform1: Option<Vec4<f32>>,
    pub transform2: Option<Vec4<f32>>,
    pub transform3: Option<Vec4<f32>>,
    pub transform4: Option<Vec4<f32>>,
}

#[derive(Serialize)]
pub struct AnimFrameFile {
    pub frames:Vec<AnimFrame>
}

impl AnimFrameFile {
    pub fn new() -> Self {
        Self {
            frames: vec![]
        }
    }

    pub fn write_to_file(&self, name:&str) -> Result<()> {
        let s = bincode::serialize(self).map_err(|e| {
            HookError::SerdeError(format!("Serialization error: {:?}", e))
        })?;

        use std::io::Write;
        let mut file = std::fs::File::create(name)?;
        file.write_all(&s)?;
        Ok(())
    }
}

pub fn write_to_file(name:&str, constants:&ConstantGroup) -> Result<()> {
    let file = GroupFile {
        floats: constants.floats.get_as_btree(),
        ints: constants.ints.get_as_btree(),
        bools: constants.bools.get_as_btree(),
    };

    let s = serde_yaml::to_string(&file).map_err(|e| {
        HookError::SerdeError(format!("Serialization error: {:?}", e))
    })?;

    use std::io::Write;

    let mut file = std::fs::File::create(name)?;
    file.write_all(s.as_bytes())?;

    Ok(())
}

/// Save specified pixel and shader constants to files.
pub fn take_snapshot(snap_dir:&str, snap_prefix:&str, vconst:Option<&ConstantGroup>, pconst:Option<&ConstantGroup>) {
    if !is_enabled() {
        return;
    }
    if snap_dir != "" && snap_prefix != "" {
        vconst.map(|vconst| {
            let out = snap_dir.to_owned()  + "/" + snap_prefix + "_vconst.yaml";
            util::write_log_file(&format!("saving vertex constants to file: {}", out));
            write_to_file(&out, &vconst)
                .unwrap_or_else(|e| {
                    util::write_log_file(&format!("ERROR: failed to write vertex constants: {:?}", e));
                });
        });
        pconst.map(|pconst| {
            let out = snap_dir.to_owned()  + "/" + snap_prefix + "_pconst.yaml";
            util::write_log_file(&format!("saving pixel constants to file: {}", out));
            write_to_file(&out, &pconst)
                .unwrap_or_else(|e| {
                    util::write_log_file(&format!("ERROR: failed to write pixel constants: {:?}", e));
                });
        });
    } else {
        util::write_log_file(&format!("ERROR: no directory set, can't save shader constants"));
    }
}
