pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use hookd3d9::{ dev_state, GLOBAL_STATE };
use shared_dx9::error::*;
use shared_dx9::util;

pub use std::collections::HashMap;
use serde::{Serialize};

/*
The memory model for these constants, from empirical tests as the documentation is sparse,
is that each data type (Float, Int, Bool) has its own storage.  So for instance,
you can set register 0 To be a certain float value, and also a certain bool value, and both of
those will be preserved (they don't overwrite each other).  Presumably the shader declares the
types it wants for each register and then the driver takes care of filling those in with the
correct values.

The constants have default values, 0.0, False, or 0, which you'll get if you call one
of the Get*() functions before the game has set anything.  However I ignored this
case since I'm only interested in the constants that the game has set explicitly.
So I use Option types to track whether a particular constant was set to any value explicitly.
*/

#[derive(Debug,PartialEq,Serialize,Copy,Clone)]
pub struct Vec4<T: Serialize> {
    a: T,
    b: T,
    c: T,
    d: T
}

pub fn vecToVec4<T>(vec:&Vec<T>, offset: usize) -> Vec4<T>
where T: Copy + serde::Serialize {
    Vec4 {
        a: vec[offset+0],
        b: vec[offset+1],
        c: vec[offset+2],
        d: vec[offset+3],
    }
}

// This is like the 'From' trait except that it allows the caller to specify an offset
// from the source that they want converted.  Useful for doing (dangerous) raw pointer reads.
pub trait FromOffset<T> {
    fn fromOffset(T, offset:isize) -> Self;
}
pub struct ConstantList<S,T> {
    offset_incr: usize,
    list: Vec<Option<T>>,
    _phantom: std::marker::PhantomData<S>,
}

// Copy floats from a raw pointer into a Vec4 in batches of 4
impl FromOffset<*const f32> for Vec4<f32> {
    #[inline]
    fn fromOffset(src:*const f32, offset:isize) -> Self {
        unsafe {
            Vec4 {
                a: *src.offset(offset+0),
                b: *src.offset(offset+1),
                c: *src.offset(offset+2),
                d: *src.offset(offset+3)
            }
        }
    }
}
// Copy ints from a raw pointer into a Vec4 in batches of 4
impl FromOffset<*const i32> for Vec4<i32> {
    #[inline]
    fn fromOffset(src:*const i32, offset:isize) -> Self {
        unsafe {
            Vec4 {
                a: *src.offset(offset+0),
                b: *src.offset(offset+1),
                c: *src.offset(offset+2),
                d: *src.offset(offset+3)
            }
        }
    }
}
// Copy BOOLs from a raw pointer into a Vec4 in batches of 1
impl FromOffset<*const BOOL> for BOOL {
    #[inline]
    fn fromOffset(src:*const i32, offset:isize) -> Self {
        unsafe {
            *src.offset(offset)
        }
    }
}

// The generic impl that does the list tracking for all types.
impl<S,T> ConstantList<S,T>
where T: FromOffset<*const S> + serde::ser::Serialize + Copy
{
    // the offset_incr is how much to bump the pConstantData pointer in `set()` after each
    // successive element.  Really this value should be baked into the specialized types
    // below (like FloatConstList), but I don't know how to do that.
    pub fn new(offset_incr:usize) -> Self {
        Self {
            offset_incr: offset_incr,
            list: Vec::new(),
            _phantom: std::marker::PhantomData
        }
    }
    pub fn set(&mut self, StartRegister: UINT,
        pConstantData: *const S,
        count: UINT) {
            let end = StartRegister + count;

            if self.list.capacity() < end as usize {
                self.list.resize_with(end as usize, || None);
            }

            let mut offset = 0;
            for reg in StartRegister..end {
                self.list[reg as usize] = Some(FromOffset::fromOffset(pConstantData, offset));
                offset += self.offset_incr as isize;
            }
        }

    pub fn get_as_btree(&self) -> std::collections::BTreeMap<UINT, T>
    where T: Copy
    {
        let mut hm: std::collections::BTreeMap<UINT, T> = std::collections::BTreeMap::new();
        self.list.iter().enumerate().for_each( |(i,x)| {
            match x {
                None => {},
                Some(x) => {
                    let i = i as u32;
                    hm.insert(i.into(), *x);
                }
            }
        });
        hm
    }

    pub fn serialize(&self) -> Result<String> {
        let hm = self.get_as_btree();

        let s = serde_yaml::to_string(&hm).map_err(|e| {
            // convert error here because I don't want to make shared dx9 depend on serde
            HookError::SerdeError(format!("Serialization error: {:?}", e))
        })?;
        Ok(s)
    }
}

pub type FloatConstList = ConstantList<f32, Vec4<f32>>;
pub type IntConstList = ConstantList<i32, Vec4<i32>>;
pub type BoolConstList = ConstantList<BOOL, BOOL>;

pub struct ConstantGroup {
    pub floats: FloatConstList,
    pub ints: IntConstList,
    pub bools: BoolConstList,
}

impl ConstantGroup {
    pub fn new() -> Self {
        ConstantGroup {
            floats: FloatConstList::new(4),
            ints: IntConstList::new(4),
            bools: BoolConstList::new(1),
        }
    }
}
pub fn is_enabled() -> bool {
    true
}

/// Save current device pixel and shader constants to files.
pub fn take_snapshot(snap_dir:&str, snap_prefix:&str) {
    if !is_enabled() {
        return;
    }
    if snap_dir != "" && snap_prefix != "" {
        unsafe {
            GLOBAL_STATE.vertex_constants.as_ref().map(|vconst| {
                let out = snap_dir.to_owned()  + "/" + snap_prefix + "_vconst.yaml";
                util::write_log_file(&format!("saving vertex constants to file: {}", out));
                write_to_file(&out, &vconst)
                    .unwrap_or_else(|e| {
                        util::write_log_file(&format!("ERROR: failed to write vertex constants: {:?}", e));
                    });
            });
            GLOBAL_STATE.pixel_constants.as_ref().map(|pconst| {
                let out = snap_dir.to_owned()  + "/" + snap_prefix + "_pconst.yaml";
                util::write_log_file(&format!("saving pixel constants to file: {}", out));
                write_to_file(&out, &pconst)
                    .unwrap_or_else(|e| {
                        util::write_log_file(&format!("ERROR: failed to write pixel constants: {:?}", e));
                    });
            });
        }
    } else {
        util::write_log_file(&format!("ERROR: no directory set, can't save shader constants"));
    }
}

#[derive(Serialize)]
struct GroupFile {
    pub floats: std::collections::BTreeMap<UINT, Vec4<f32>>,
    pub ints: std::collections::BTreeMap<UINT, Vec4<i32>>,
    pub bools: std::collections::BTreeMap<UINT, BOOL>,
}

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
    pub player_transform: Option<Vec4<f32>>,
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

pub unsafe extern "system" fn hook_set_vertex_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    util::write_log_file(&format!("hook_set_vertex_sc_f: {} {}", StartRegister, Vector4fCount));
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount);
    if hr == 0 {
        let is_snapping = GLOBAL_STATE.is_snapping;
        if is_snapping && Vector4fCount > 0 {
            util::write_log_file(&format!("snapping vf const {}, {} count: {} {} {} {}...",
                StartRegister,
                Vector4fCount,
                *pConstantData,
                *pConstantData.offset(1),
                *pConstantData.offset(2),
                *pConstantData.offset(3),
            ));
        }
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.floats.set(StartRegister, pConstantData, Vector4fCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_vertex_sc_i(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_i)(THIS, StartRegister, pConstantData, Vector4iCount);
    if hr == 0 {
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.ints.set(StartRegister, pConstantData, Vector4iCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_vertex_sc_b(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_b)(THIS, StartRegister, pConstantData, BoolCount);
    if hr == 0 {
        GLOBAL_STATE.vertex_constants.as_mut().map(|vconsts| {
            vconsts.bools.set(StartRegister, pConstantData, BoolCount);
        });
    }
    hr
}
// pixel functions:
pub unsafe extern "system" fn hook_set_pixel_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.floats.set(StartRegister, pConstantData, Vector4fCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_pixel_sc_i(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const i32,
    Vector4iCount: UINT,
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_i)(THIS, StartRegister, pConstantData, Vector4iCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.ints.set(StartRegister, pConstantData, Vector4iCount);
        });
    }
    hr
}

pub unsafe extern "system" fn hook_set_pixel_sc_b(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const BOOL,
    BoolCount: UINT
) -> HRESULT {
    let hr = (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_pixel_sc_b)(THIS, StartRegister, pConstantData, BoolCount);
    if hr == 0 {
        GLOBAL_STATE.pixel_constants.as_mut().map(|pconsts| {
            pconsts.bools.set(StartRegister, pConstantData, BoolCount);
        });
    }
    hr
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    pub fn test_float2() {
        let mut fconst = FloatConstList::new(4);
        let floats0 = vec![0.5, 1.0, 2.0, 3.0];
        {
            let constant_data: *const f32 = floats0.as_ptr();
            fconst.set(0, constant_data, 1);
            assert_eq!(fconst.list[0], Some(vecToVec4(&floats0, 0)));
        }

        {
            let floats = vec![0.5, 1.0, 2.0, 3.0,
                10.5, 11.0, 12.0, 13.0,
                20.5, 21.0, 22.0, 23.0]; // this one will be ignored
            let constant_data: *const f32 = floats.as_ptr();
            fconst.set(15, constant_data, 2);
            assert_eq!(fconst.list[15], Some(vecToVec4(&floats, 0)));
            assert_eq!(fconst.list[16], Some(vecToVec4(&floats, 4)));

            assert_eq!(fconst.list[0], Some(vecToVec4(&floats0, 0)));
            for i in 1..15 {
                assert_eq!(fconst.list[i], None)
            }
        }
    }

    #[test]
    pub fn test_int2() {
        let mut iconst = IntConstList::new(4);
        let ints0 = vec![5, 10, 20, 30];
        {
            let constant_data: *const _ = ints0.as_ptr();
            iconst.set(0, constant_data, 1);
            assert_eq!(iconst.list[0], Some(vecToVec4(&ints0, 0)));
        }

        {
            let ints = vec![5, 10, 20, 30,
                105, 110, 120, 130,
                205, 210, 220, 230]; // this one will be ignored
            let constant_data: *const _ = ints.as_ptr();
            iconst.set(15, constant_data, 2);
            assert_eq!(iconst.list[15], Some(vecToVec4(&ints, 0)));
            assert_eq!(iconst.list[16], Some(vecToVec4(&ints, 4)));

            assert_eq!(iconst.list[0], Some(vecToVec4(&ints, 0)));
            for i in 1..15 {
                assert_eq!(iconst.list[i], None)
            }
        }
    }

    #[test]
    pub fn test_bool2() {
        // Note that unlike floats and ints, bool constants values are not tuples of 4 elements.
        // each constant maps to one bool value.
        let mut iconst = BoolConstList::new(1);

        let ints0 = vec![TRUE, FALSE, TRUE, FALSE];
        let check_first = {
            let constant_data: *const _ = ints0.as_ptr();
            iconst.set(0, constant_data, 4);
            assert_eq!(iconst.list.len(), 4);

            let check_first = |iconst:&BoolConstList| {
                ints0.iter().enumerate().for_each(|(i,x)| {
                    assert_eq!(iconst.list[i], Some(*x));
                });
            };
            // check it, then return the closure so that we can call it again after next phase.
            // this is kinda obtuse but whatev
            check_first(&iconst);
            check_first
        };

        {
            let ints = vec![TRUE, TRUE, TRUE, FALSE,
                FALSE, FALSE, TRUE, TRUE,
                TRUE, TRUE, TRUE, TRUE]; // this one will be ignored
            let constant_data: *const _ = ints.as_ptr();
            iconst.set(15, constant_data, 8);
            assert_eq!(iconst.list.len(), 15 + 8);
            ints.iter().take(8).enumerate().for_each(|(i,x)| {
                assert_eq!(iconst.list[15 + i], Some(*x));
            });
            for i in 4..15 {
                assert_eq!(iconst.list[i], None, "fail on index {}", i);
            }
            check_first(&iconst);
        }
    }

    #[test]
    fn test_serialization() {
        let mut iconst = BoolConstList::new(1);

        iconst.set(0, (vec![TRUE, FALSE, TRUE, FALSE]).as_ptr(), 4 );
        iconst.set(4, (vec![FALSE, FALSE, TRUE, FALSE]).as_ptr(), 4 );
        iconst.set(8, (vec![TRUE, TRUE, TRUE, FALSE]).as_ptr(), 4 );
        iconst.set(16, (vec![FALSE, TRUE, TRUE, FALSE]).as_ptr(), 4 );

        let s = iconst.serialize().expect("shit");
        assert_eq!(s, r"---
0: 1
1: 0
2: 1
3: 0
4: 0
5: 0
6: 1
7: 0
8: 1
9: 1
10: 1
11: 0
16: 0
17: 1
18: 1
19: 0");

    }
}
