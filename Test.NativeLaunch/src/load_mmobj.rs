use std::{str::{Split, FromStr}, time::{SystemTime, Duration}, collections::HashMap, ops::Add};
use std::fmt::Debug;
use aho_corasick::AhoCorasick;

use crate::interop_mmobj::make_interop_mmobj;


#[repr(C)]
#[derive(Debug,Copy,Clone)]
pub struct Float3 {
    pub x:f32,
    pub y:f32,
    pub z:f32
}

#[repr(C)]
#[derive(Debug,Copy,Clone)]
pub struct Float2 {
    pub x:f32,
    pub y:f32,
}

#[repr(C)]
#[derive(Debug,Copy,Clone)]
pub struct BlendPair {
    pub idx:u32,
    pub weight:f32,
}

#[repr(C)]
#[derive(Debug,Copy,Clone)]
pub struct FaceVert {
    pub pos:usize,
    pub nrm:usize,
    pub tex:usize,
}

pub struct MMObj {
    pub filename: String,
    pub positions: Vec<Float3>,
    pub texcoord: Vec<Float2>,
    pub normals: Vec<Float3>,
    pub vgroup_names:Vec<String>,
    pub vgroup_lists:Vec<Vec<i32>>,
    pub vblend:Vec<Vec<BlendPair>>,
    pub posx:Vec<Vec<String>>,
    pub uvx:Vec<Vec<String>>,
    pub faces:Vec<[FaceVert;3]>,
    pub mtllib:Vec<String>,
}

fn pf32 (sp: &mut Split<&[char]>) -> anyhow::Result<f32> {
    // call next on the iter and return the err if result is None
    let sval = sp.next().ok_or_else(|| anyhow!("pf32: bad split"))?.trim();
    let fval = sval.parse::<f32>().map_err(|e| anyhow!("failed to parse float from str '{}': {}", sval, e))?;
    Ok(fval)
}
fn parse_2_f32(off:usize, text:&str) -> anyhow::Result<Float2> {
    let slc = &text[off..];
    let mut i = slc.split([' ', '\n', '\r'].as_ref());
    let f1 = pf32(&mut i)?;
    let f2 = pf32(&mut i)?;
    Ok(Float2 { x:f1, y:f2 })
}
fn parse_3_f32(off:usize, text:&str) -> anyhow::Result<Float3> {
    let slc = &text[off..];
    let mut i = slc.split([' ', '\n', '\r'].as_ref());
    let f1 = pf32(&mut i)?;
    let f2 = pf32(&mut i)?;
    let f3 = pf32(&mut i)?;
    Ok(Float3 { x:f1, y:f2, z:f3 })
}
fn parse_1_str(off:usize, text:&str) -> anyhow::Result<String> {
    let slc = &text[off..];
    let mut i = slc.split([' ', '\n', '\r'].as_ref());
    let s = i.next().ok_or_else(|| anyhow!("parse_1_str: bad split"))?.trim();
    if s.is_empty() {
        return Err(anyhow!("parse_1_str: empty name"));
    }
    Ok(s.to_owned())
}
fn parse_n_int<T: FromStr>(off:usize, text:&str) -> anyhow::Result<Vec<T>>
    where <T as FromStr>::Err: Debug {
    let slc = &text[off..];
    let eol = slc.find('\n').ok_or_else(|| anyhow!("parse_n_int: newline required"))?;
    let slc = &slc[0..eol].trim();
    let i = slc.split([' '].as_ref());

    let mut v = Vec::with_capacity(10);
    for s in i {
        let val = s.parse::<T>().map_err(|e| anyhow!("parse_n_int: failed to parse int from str '{}': {:?}", slc, e))?;
        v.push(val);
    }
    Ok(v)
}
fn parse_blendpairs(off:usize, text:&str) -> anyhow::Result<Vec<BlendPair>> {
    let slc = &text[off..];
    let eol = slc.find('\n').ok_or_else(
        || anyhow!("parse_blendpairs: newline required"))?;
    let slc = &slc[0..eol].trim();
    let i = slc.split([' '].as_ref());

    let mut v = Vec::with_capacity(10);
    for s in i {
        let mut i = s.split(['/'].as_ref());
        let bidx = i.next().ok_or_else(|| anyhow!("parse_blendpairs: bad split"))?
            .parse::<u32>().map_err(|e| anyhow!("parse_blendpairs: failed to parse bp int from str '{}': {}", s, e))?;
        let bw = i.next().ok_or_else(|| anyhow!("parse_blendpairs: bad split"))?
            .parse::<f32>().map_err(|e| anyhow!("parse_blendpairs: failed to parse bp f32 from str '{}': {}", s, e))?;
        v.push(BlendPair { idx:bidx, weight:bw });
    }
    Ok(v)
}
fn parse_n_strs(off:usize, text:&str) -> anyhow::Result<Vec<String>> {
    let slc = &text[off..];
    let eol = slc.find('\n').ok_or_else(|| anyhow!("parse_n_strs: newline required"))?;
    let slc = &slc[0..eol].trim();
    let i = slc.split([' '].as_ref());

    let mut v = Vec::with_capacity(10);
    for s in i {
        if s.is_empty() {
            return Err(anyhow!("parse_n_strs: empty string"));
        }
        v.push(s.to_owned());
    }
    Ok(v)
}
fn parse_face_pos_tex_nrm(off:usize, text:&str) -> anyhow::Result<[FaceVert;3]> {
    let slc = &text[off..];
    let eol = slc.find('\n').ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: newline required"))?;
    let slc = &slc[0..eol].trim();
    let mut slc = slc.split(' ');
    let mut res = [FaceVert { pos: 0, nrm: 0, tex: 0} ;3];
    for f in 0..3 { // 3 points because triangles
        let slc = slc.next().ok_or_else(|| anyhow!("bad split"))?;
        let mut i = slc.split(['/'].as_ref());
        // obj indices are 1-based, sub1 to make them zero based
        let p = i.next().ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad split"))?
            .parse::<u32>().map_err(|e| anyhow!("failed to parse f pos int from str '{}': {}", slc, e))?
            .checked_sub(1).ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad index"))?;
        let t = i.next().ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad split"))?
            .parse::<u32>().map_err(|e| anyhow!("failed to parse f tex int from str '{}': {}", slc, e))?
            .checked_sub(1).ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad index"))?;
        let n = i.next().ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad split"))?
            .parse::<u32>().map_err(|e| anyhow!("failed to parse f nrm int from str '{}': {}", slc, e))?
            .checked_sub(1).ok_or_else(|| anyhow!("parse_face_pos_tex_nrm: bad index"))?;
        res[f] = FaceVert { pos:p as usize, nrm:n as usize, tex:t as usize };
    }

    Ok(res)
}

fn parse_x<F,D>(offsets:&Vec<usize>, text:&str, defv:D, pfn:F) -> anyhow::Result<Vec<D>>
    where
        D: Clone,
        F: Fn(usize, &str) -> anyhow::Result<D> {
    let mut parse_err = Ok(defv.clone());
    let vres:Vec<_> = offsets.iter().map_while(|startoff| {
        let res = pfn(*startoff, &text);
        if res.is_err() {
            parse_err = res;
            None
        } else {
            Some(res.unwrap_or_else(|_e| defv.clone()))
        }
    }).collect();
    parse_err?;
    Ok(vres)
}

fn dedup<T: Ord>(v:Vec<T>) -> Vec<T> {
    let set = std::collections::BTreeSet::from_iter(v.into_iter());
    set.into_iter().collect()
}

/// Experimental function to load a set of mmobj files.  Instead of using regex/captures like the managed
/// code does, this uses aho-corasick to sort all the various line types in the mmobj into buckets,
/// which is possible because each line type begins with a unique prefix.
/// Then each bucket can be parsed without regex since the format for each is very specific.
///
/// Although I haven't fully tested this, preliminary results show that its much faster than
/// the managed code.  The line loop in MeshUtil.readObj in managed code takes about 22 seconds
/// for 135 files, while this function processes the same files in 1.2 seconds.  So its probably worth
/// replacing the managed loader with this once I figure out the interop layer for it.  That interop
/// and some postprocessing in managed code will add some time but probably not much since most of
/// the heavy lift is the parsing here.  Getting this data to managed code could be a straight
/// blit of repr-C structs, which is pretty fast.
///
/// The list of files comes from a file
/// `mmobjlist.txt` which can be generated by running this program in its normal mode and then
/// after it finishes loading mods, running this command in a terminal:
/// `cat /m/ModelMod/Logs/ModelMod.test_native_launch.log | grep -i readmmobj > mmobjlist.txt`
///
/// Then run this program with `MODE=mmobj cargo run --release`
pub fn test_load_mmobj() -> anyhow::Result<()> {
    // read "base1.txt" to get a list of files to load
    let res = std::fs::read_to_string("mmobjlist.txt").map_err(|e| anyhow!(e))?;
    // split each line on "[M:SW:readmmobj:"
    let mut files = Vec::new();
    for line in res.lines() {
        let mut parts = line.split("[M:SW:readmmobj:");
        let _ = parts.next();
        let part = parts.next().ok_or_else(|| anyhow!("bad line"))?;
        let mut parts = part.split("]");
        let file = parts.next().ok_or_else(|| anyhow!("bad line"))?;
        files.push(file.trim());
    }
    // make sure all files still exist
    for file in &files {
        if !std::path::Path::new(file).exists() {
            return Err(anyhow!("file doesn't exist: {}", file));
        }
    }

    // make a list of mmobj line prefixes
    let patterns = &[
        r"vt ",
        r"v ",
        r"vn ",
        r"#vgn ",
        r"#vg ",
        r"#vbld ",
        r"#pos_xforms ",
        r"#uv_xforms ",
        r"f ",
        r"mtllib ",
    ];
    let start = SystemTime::now();
    let mut loadres:HashMap<String, MMObj> = HashMap::new();
    // slurp all files
    let mut io_total = Duration::from_millis(0);
    let mut ac_total = Duration::from_millis(0);
    let mut interop_copy_total = Duration::from_millis(0);
    for f in &files {
        let io_start = SystemTime::now();
        let filetext = std::fs::read_to_string(&f).map_err(|e| anyhow!(e))?;
        io_total = io_total.add(io_start.elapsed()?);

        let v_capac = 16;
        let mut outputs = patterns.map(|_| Vec::with_capacity(v_capac));

        let ac_start = SystemTime::now();
        let ac = AhoCorasick::new(patterns);
        ac.find_iter(&filetext).for_each(|mat| {
            outputs[mat.pattern()].push(mat.end());
        });
        ac_total = ac_total.add(ac_start.elapsed()?);

        let mut parseall = || -> anyhow::Result<MMObj> {
            let texcoord = parse_x(&outputs[0], &filetext, Float2 { x: 0.0, y: 0.0 }, parse_2_f32)?;
            let positions = parse_x(&outputs[1], &filetext, Float3 { x: 0.0, y: 0.0, z:0.0 }, parse_3_f32)?;
            let normals = parse_x(&outputs[2], &filetext, Float3 { x: 0.0, y: 0.0, z:0.0 }, parse_3_f32)?;
            let vgroup_names = parse_x(&outputs[3], &filetext, String::new(), parse_1_str)?;
            let vgroup_lists = parse_x(&outputs[4], &filetext, Vec::new(), parse_n_int::<i32>)?;
            let vblend = parse_x(&outputs[5], &filetext, Vec::new(), parse_blendpairs)?;
            let posx = parse_x(&outputs[6], &filetext, Vec::new(), parse_n_strs)?;
            let posx = dedup(posx);
            let uvx = parse_x(&outputs[7], &filetext, Vec::new(), parse_n_strs)?;
            let uvx = dedup(uvx);
            let faces = parse_x(&outputs[8], &filetext,
                [FaceVert { pos: 0, nrm: 0, tex: 0 };3], parse_face_pos_tex_nrm)?;
            let mtllib = parse_x(&outputs[9], &filetext, String::new(), parse_1_str)?;

            let mmobj = MMObj {
                filename: f.to_string(),
                positions,
                texcoord,
                normals,
                vgroup_names,
                vgroup_lists,
                vblend,
                posx,
                uvx,
                faces,
                mtllib,
            };

            let start = SystemTime::now();
            let pair = make_interop_mmobj(mmobj);

            interop_copy_total += start.elapsed().unwrap();

            Ok(pair.discard_interop())
        };

        let mmobj = parseall().map_err(|e| anyhow!("error parsing {}: {}", f, e))?;

        loadres.insert(f.to_string(), mmobj);
    }
    println!("{} files in {:?}ms",  files.len(), start.elapsed().unwrap().as_millis());
    println!("io: {:?}", io_total.as_millis());
    println!("ac: {:?}", ac_total.as_millis());
    println!("interop copy: {:?}", interop_copy_total.as_millis());

    // print one to make sure I didn't just make a bunch of bullshit
    let mmobjkey = loadres.keys().find(|k| k.contains("FNH_ZodiacArmorMod")).expect("no key?");
    let mmobj = loadres.get(mmobjkey).expect("no mmobj?");
    println!("sample mmobj: {:?}", mmobj.filename);
    println!("  positions: {:?}", mmobj.positions.len());
    (0_usize..2).for_each(|i| {
        println!("    pos[{}]: {:?}", i, mmobj.positions[i]);
    });
    println!("  texcoord: {:?}", mmobj.texcoord.len());
    (0_usize..2).for_each(|i| {
        println!("    tex[{}]: {:?}", i, mmobj.texcoord[i]);
    });
    println!("  normals: {:?}", mmobj.normals.len());
    (0_usize..2).for_each(|i| {
        println!("    nrm[{}]: {:?}", i, mmobj.normals[i]);
    });
    println!("  vgroup_names: {:?}", mmobj.vgroup_names.len());
    (0_usize..2).for_each(|i| {
        println!("    vgn[{}]: {:?}", i, mmobj.vgroup_names[i]);
    });
    println!("  vgroup_lists: {:?}", mmobj.vgroup_lists.len());
    (0_usize..2).for_each(|i| {
        println!("    vg[{}]: {:?}", i, mmobj.vgroup_lists[i]);
    });
    println!("  vblend: {:?}", mmobj.vblend.len());
    (0_usize..2).for_each(|i| {
        println!("    vblend[{}]: {:?}", i, mmobj.vblend[i]);
    });
    println!("  posx: {:?}", mmobj.posx.len());
    (0_usize..1).for_each(|i| {
        println!("    posx[{}]: {:?}", i, mmobj.posx[i]);
    });
    println!("  uvx: {:?}", mmobj.uvx.len());
    (0_usize..1).for_each(|i| {
        println!("    uvx[{}]: {:?}", i, mmobj.uvx[i]);
    });
    println!("  faces: {:?}", mmobj.faces.len());
    (0_usize..2).for_each(|i| {
        println!("    face[{}]: {:?}", i, mmobj.faces[i]);
    });
    println!("  mtllib: {:?}", mmobj.mtllib.len());
    // (0_usize..2).for_each(|i| {
    //     println!("    mtllib[{}]: {:?}", i, mmobj.mtllib[i]);
    // });

    return Ok(());
}
