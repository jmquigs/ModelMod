// at some point after spewing enough warnings, clippy should just say "have you considered
// taking up python?"
#![allow(clippy::all)]

#![allow(non_snake_case)]
mod hook_snapshot;
pub use crate::hook_snapshot::*;

#[macro_use]
extern crate lazy_static;