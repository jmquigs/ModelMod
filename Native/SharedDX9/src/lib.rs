#[macro_use]
#[cfg(windows)]
extern crate winapi;

#[macro_use]
extern crate lazy_static;

pub mod defs;
pub mod state;
pub mod types;
pub mod util;
pub mod error;