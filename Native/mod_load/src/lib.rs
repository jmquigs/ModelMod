// wow you think maybe there is some undocumented unsafe stuff going on here?
#![allow(clippy::missing_safety_doc)]

#![allow(static_mut_refs)]

mod mod_load;
mod mod_vector;
pub use crate::mod_load::*;