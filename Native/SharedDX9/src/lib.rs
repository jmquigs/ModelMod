#![allow(non_snake_case)]

//#[macro_use]
#[cfg(windows)]
#[macro_use]
extern crate winapi;

#[macro_use]
extern crate lazy_static;

pub mod defs;
pub mod state;
pub mod types;
pub mod util;
pub mod error;