// at some point after spewing enough warnings, clippy should just say "have you considered
// taking up python?"
#![allow(clippy::all)]

#![allow(static_mut_refs)]

#![allow(non_snake_case)]
mod hook_snapshot;
mod snap_extdll;
pub use crate::hook_snapshot::*;

#[macro_use]
extern crate lazy_static;