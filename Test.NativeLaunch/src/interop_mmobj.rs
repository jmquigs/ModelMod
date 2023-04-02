//! Completely untested code to build an interop struct for MMObj
//!
use std::{ffi::OsStr};

use winapi::um::winnt::WCHAR;

use crate::load_mmobj::{BlendPair, FaceVert, Float3, Float2, MMObj};

const MAX_FILEPATH_LEN: usize = 8192;
const MAX_SHORT_STRING_LEN: usize = 512;
const MAX_VGROUP: usize = 32;
const MAX_SHORT_STRING_LIST: usize = 32;
const MAX_BLENDPAIR: usize = 32;


#[repr(C)]
#[derive(Debug,Copy,Clone)]
pub struct InteropShortString {
    pub buf: [WCHAR; MAX_SHORT_STRING_LEN],
    pub len_elems: usize,
}
impl Default for InteropShortString {
    fn default() -> Self {
        InteropShortString {
            buf: [0; MAX_SHORT_STRING_LEN],
            len_elems: 0,
        }
    }
}

#[repr(C)]
pub struct VGroupList {
    pub elem: [i32; MAX_VGROUP],
    pub len_elems: usize,
}

#[repr(C)]
pub struct BlendPairList {
    pub elem: [BlendPair; MAX_BLENDPAIR],
    pub len_elems: usize,
}

#[repr(C)]
pub struct ShortStringList {
    pub elem: [InteropShortString; MAX_SHORT_STRING_LIST],
    pub len_elems: usize,
}
impl Default for ShortStringList {
    fn default() -> Self {
        ShortStringList {
            elem: [InteropShortString::default(); MAX_SHORT_STRING_LIST],
            len_elems: 0,
        }
    }
}

#[repr(C)]
pub struct Face {
    pub verts: [FaceVert; 3],
}

#[repr(C)]
pub struct MMObjInterop {
    pub filename: [WCHAR; MAX_FILEPATH_LEN],
    pub positions: *const Float3,
    pub plen_bytes: usize,
    pub texcoord: *const Float2,
    pub texlen_bytes: usize,
    pub normals: *const Float3,
    pub nlen_bytes: usize,
    pub vgroup_names: *const InteropShortString,
    pub vgroup_names_len_bytes: usize,
    pub vgroup_lists: *const VGroupList,
    pub vgroup_lists_len_bytes: usize,
    pub vblend: *const BlendPairList,
    pub vblend_len_bytes: usize,
    pub posx: *const ShortStringList,
    pub posx_len_bytes: usize,
    pub uvx: *const ShortStringList,
    pub uvx_len_bytes: usize,
    pub faces: *const Face,
    pub faces_len_bytes: usize,
    pub mtllib: *const InteropShortString,
    pub mtllib_len_bytes: usize,
}

pub struct MMObjPair(MMObj, MMObjInterop);
impl MMObjPair {
    pub fn discard_interop(self) -> MMObj { self.0 }
}

/// Construct in `MMObjInterop` from `MMObj`.  Since the lifetime of the resulting interop object
/// is tied to the MMObj (due to retaining pointers to the MMObj),
/// this takes ownership of that and returns a pair object.
pub fn make_interop_mmobj(mmobj:MMObj) -> MMObjPair {
    use std::os::windows::ffi::OsStrExt;
    // convert file name to windows wide string
    let to_wide = |s:&str| -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect::<Vec<_>>()
    };
    //let wfilename = OsStr::new(&mmobj.filename).encode_wide().chain(Some(0)).collect::<Vec<_>>();

    fn copy_filepath(src:&Vec<u16>) -> [u16; MAX_FILEPATH_LEN] {
        let mut dst = [0_u16; MAX_FILEPATH_LEN];
        dst[..src.len()].copy_from_slice(src);
        dst
    }
    fn copy_short_string(src:&Vec<u16>) -> [u16; MAX_SHORT_STRING_LEN] {
        let mut dst = [0_u16; MAX_SHORT_STRING_LEN];
        dst[..src.len()].copy_from_slice(src);
        dst
    }

    let make_short_string = |s:&str| -> InteropShortString {
        let wstr = to_wide(s);
        InteropShortString {
            buf: copy_short_string(&wstr),
            len_elems: 0,
        }
    };

    let vgroup_names_ss:Vec<InteropShortString> = mmobj.vgroup_names.iter()
        .map(|s| {
            make_short_string(s)
        }).collect::<Vec<_>>();

    let vgroup_lists = mmobj.vgroup_lists.iter()
        .map(|v| {
            let maxlen = usize::min(v.len(), MAX_VGROUP);
            let mut vgl = VGroupList {
                elem: [0; MAX_VGROUP],
                len_elems: maxlen,
            };
            for i in 0..maxlen {
                vgl.elem[i] = v[i];
            }
            vgl
        }).collect::<Vec<_>>();

    let vblend = mmobj.vblend.iter()
        .map(|v| {
            let maxlen = usize::min(v.len(), MAX_BLENDPAIR);
            let mut vb = BlendPairList {
                elem: [BlendPair { idx: 0, weight: 0.0 }; MAX_BLENDPAIR],
                len_elems: maxlen
            };
            for i in 0..maxlen {
                vb.elem[i] = v[i];
            }
            vb
        }).collect::<Vec<_>>();

    let make_short_string_list = |vv:&Vec<Vec<String>>| -> Vec<ShortStringList> {
        vv.iter()
            .map(|v| {
                let maxlen = usize::min(v.len(), MAX_SHORT_STRING_LIST);
                let mut ssl = ShortStringList {
                    elem: [InteropShortString::default(); MAX_SHORT_STRING_LIST],
                    len_elems: maxlen,
                };
                for i in 0..maxlen {
                    ssl.elem[i] = make_short_string(&v[i]);
                }
                ssl
            }).collect::<Vec<_>>()
    };

    let posxlist = make_short_string_list(&mmobj.posx);
    let uvxlist = make_short_string_list(&mmobj.uvx);

    let facelist = mmobj.faces.iter()
        .map(|f| {
            let mut ff = Face {
                verts: [ FaceVert { pos: 0, nrm: 0, tex: 0 }; 3]
            };
            for i in 0..3 {
                ff.verts[i] = f[i];
            }
            ff
        }).collect::<Vec<_>>();

    let mtllib_list = mmobj.mtllib.iter()
        .map(|s| {
            make_short_string(s)
        }).collect::<Vec<_>>();

    // consruct an interop mod from the mmobj
    let io_mmobj = MMObjInterop {
        filename: copy_filepath(&to_wide(&mmobj.filename)),
        positions: mmobj.positions.as_ptr(),
        plen_bytes: mmobj.positions.len() * std::mem::size_of::<Float3>(),
        texcoord: mmobj.texcoord.as_ptr(),
        texlen_bytes: mmobj.texcoord.len() * std::mem::size_of::<Float2>(),
        normals: mmobj.normals.as_ptr(),
        nlen_bytes: mmobj.normals.len() * std::mem::size_of::<Float3>(),
        vgroup_names: vgroup_names_ss.as_ptr(),
        vgroup_names_len_bytes: vgroup_names_ss.len() * std::mem::size_of::<InteropShortString>(),
        vgroup_lists: vgroup_lists.as_ptr(),
        vgroup_lists_len_bytes: vgroup_lists.len() * std::mem::size_of::<VGroupList>(),
        vblend: vblend.as_ptr(),
        vblend_len_bytes: vblend.len() * std::mem::size_of::<BlendPairList>(),
        posx: posxlist.as_ptr(),
        posx_len_bytes: posxlist.len() * std::mem::size_of::<ShortStringList>(),
        uvx: uvxlist.as_ptr(),
        uvx_len_bytes: uvxlist.len() * std::mem::size_of::<ShortStringList>(),
        faces: facelist.as_ptr(),
        faces_len_bytes: facelist.len() * std::mem::size_of::<Face>(),
        mtllib: mtllib_list.as_ptr(),
        mtllib_len_bytes: mtllib_list.len() * std::mem::size_of::<InteropShortString>(),
    };

    MMObjPair(mmobj,io_mmobj)
}