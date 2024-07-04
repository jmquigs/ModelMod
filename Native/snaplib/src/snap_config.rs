use serde::{Deserialize, Serialize};
use winapi::shared::minwindef::UINT;
//use fnv::FnvHashSet;
use std::collections::HashSet;
use std::fmt;
use shared_dx::util::write_log_file;
use shared_dx::error::{HookError, Result};

// Snapshotting currently stops after a certain amount of real time has passed from the start of
// the snap, specified by the config (snap_ms)
// One might expect that just snapping everything drawn within a single begin/end scene combo is
// sufficient, but this often misses data,
// and sometimes fails to snapshot anything at all.  This may be because the game is using multiple
// begin/end combos, so maybe
// present->present would be more reliable (TODO: check this)
// Using a window makes it much more likely that something useful is captured, at the expense of
// some duplicates; even though
// some objects may still be missed.  Some investigation to make this more reliable would be useful.

#[derive(Deserialize,Serialize,Eq,PartialEq,Copy,Clone,Debug,Hash)]
pub struct AutoSnapMesh {
    pub prims: UINT,
    pub verts: UINT,
}
impl AutoSnapMesh {
    #[allow(dead_code)]
    fn new(prims:UINT, verts:UINT) -> Self {
        Self {
            prims,
            verts
        }
    }
}
#[derive(Deserialize,Clone,Serialize)]
pub struct SnapConfig {
    pub snap_ms: u32,
    pub snap_anim: bool,
    pub require_gpu: Option<bool>,
    pub snap_anim_on_count: u32,
    pub vconsts_to_capture: usize,
    pub pconsts_to_capture: usize,
    pub autosnap:Option<HashSet<AutoSnapMesh>>,
    pub plugins:Option<Vec<String>>,
    #[serde(default)]
    pub clear_sd_on_reset: bool,
}
impl fmt::Display for SnapConfig {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "SnapConfig {{")?;
        writeln!(f, "  snap_ms: {}", self.snap_ms)?;
        writeln!(f, "  clear_sd_on_reset: {}", self.clear_sd_on_reset)?;
        writeln!(f, "  snap_anim: {}", self.snap_anim)?;
        if self.snap_anim {
            writeln!(f, "  require gpu: true (due to snap anim)")?;
        } else {
            writeln!(f, "  require gpu: {:?}", self.require_gpu)?;
        }
        writeln!(f, "  snap_anim_on_count: {}", self.snap_anim_on_count)?;
        writeln!(f, "  vconsts_to_capture: {}", self.vconsts_to_capture)?;
        writeln!(f, "  pconsts_to_capture: {}", self.pconsts_to_capture)?;
        if self.snap_anim {
            writeln!(f, "  max sequences: {}", self.max_const_sequences())?;
        }
        match self.autosnap.as_ref() {
            None => writeln!(f, "  no autosnap meshes")?,
            Some(hm) => {
                writeln!(f, "  autosnap meshes:")?;
                for asm in hm.iter() {
                    writeln!(f, "    {}p {}v", asm.prims, asm.verts )?;
                }
            }
        }
        writeln!(f, "  plugins: {:?}", self.plugins)?;

        writeln!(f, "}}")
    }
}

impl SnapConfig {
    pub fn new() -> Self {
        Self {
            snap_ms: 250, // TODO11: dx11 needs longer, 5 seconds?
            snap_anim: false,
            require_gpu: None,
            snap_anim_on_count: 1,
            vconsts_to_capture: 224,
            pconsts_to_capture: 224,
            autosnap: None,
            plugins: None,
            clear_sd_on_reset: false,
        }
    }
    pub fn max_const_sequences(&self) -> usize {
        let mut seqs = self.snap_ms / 1000 * 4096;
        if seqs < 4096 {
            seqs = 4096;
        }
        seqs as usize
    }

    pub fn load(rootdir:&str) -> Result<Self> {
        write_log_file("loading snap config");

        use std::path::PathBuf;
        let mut pb = PathBuf::from(&rootdir);
        pb.push("\\snapconfig.yaml");

        if !pb.is_file() {
            write_log_file(&format!("Snap confile does not exist: {:?}", pb));
            write_log_file("Using defaults");
            return Ok(SnapConfig::new())
        }

        use std::fs::File;
        use std::io::BufReader;

        let file = File::open(pb)?;
        let reader = BufReader::new(file);
        let s: SnapConfig = serde_yaml::from_reader(reader).map_err(|e| HookError::SnapshotFailed(format!("deserialize error: {}", e)))?;

        // let mut sclock = SNAP_CONFIG.write().map_err(|e| HookError::SnapshotFailed(format!("failed to lock snap config: {}", e)))?;
        // *sclock = s;
        // drop(sclock);

        // let sclock = SNAP_CONFIG.read().map_err(|e| HookError::SnapshotFailed(format!("failed to lock snap config: {}", e)))?;
        // write_log_file(&format!("loaded snap config: {}", *sclock));

        Ok(s)

    }

    // This is useful for generating a new config file.  but I am leaving it commented out because
    // since it does serde-y stuff it might increase compile times.
    // fn save(&self, rootdir:&str) -> Result<()> {
    //     write_log_file("saving snap config");

    //     use std::path::PathBuf;
    //     let mut pb = PathBuf::from(&rootdir);
    //     pb.push("\\snapconfig.yaml");
    //     // write_log_file(&format!("writing intial sc to {:?}", pb));
    //     // {
    //     //     let sc = &mut SNAP_CONFIG.write().unwrap();
    //     //     let mut autosnap = HashSet::new();
    //     //     autosnap.insert(AutoSnapMesh::new(1234,567));
    //     //     (*sc).autosnap = Some(autosnap);
    //     //     drop(sc);
    //     // }

    //     // write_log_file(&format!("reread sc for {:?}", pb));
    //     // let conf = SNAP_CONFIG.read().map_err(|e| {
    //     //     // convert error here because I don't want to make shared dx9 depend on serde
    //     //     HookError::SerdeError(format!("Serialization error: {:?}", e))
    //     // })?;

    //     write_log_file(&format!("cereal sc to {:?}", pb));
    //     let s = serde_yaml::to_string(&*self).map_err(|e| {
    //         // convert error here because I don't want to make shared dx9 depend on serde
    //         HookError::SerdeError(format!("Serialization error: {:?}", e))
    //     })?;

    //     write_log_file(&format!("writef to {:?}", pb));
    //     use std::io::Write;
    //     let mut file = std::fs::File::create(&*pb.to_string_lossy())?;
    //     file.write_all(&s.as_bytes())?;
    //     Ok(())
    // }
}