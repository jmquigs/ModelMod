use shared_dx9::defs_dx9::UINT;
use std::time::SystemTime;
use shared_dx9::error::Result;

use constant_tracking;
pub struct AnimConstants {
    pub snapped_at: SystemTime,
    pub prim_count: UINT,
    pub vert_count: UINT,
    pub constants: constant_tracking::ConstantGroup,
    pub sequence: usize,
    pub frame: u64,
    pub capture_count: u32,
    pub player_transform: Result<String>,
    pub snap_on_count: u32,
    // currently these matrices are not captured because they are identity
    // worldmat: D3DMATRIX,
    // viewmat: D3DMATRIX,
    // projmat: D3DMATRIX,
}
use std::collections::{HashSet,HashMap}; // TODO: make sure i'm not using the slow hash function version of these
pub struct AnimSnapState {
    pub sequence_vconstants:Vec<AnimConstants>,
    pub expected_primverts: HashSet<(UINT,UINT)>,
    pub seen_primverts: HashSet<(UINT,UINT)>,
    pub capture_count_this_frame: HashMap<(UINT,UINT), u32>,
    pub seen_all: bool,
    pub next_vconst_idx: usize,
    pub sequence_start_time: SystemTime,
    pub curr_frame: u64,
    pub start_frame: u64,
    pub snap_dir: String,
}
