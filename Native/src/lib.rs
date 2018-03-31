#![allow(non_snake_case)]
// this is here to silence the spammy warnings from the COM macro definitions in dnclr.
// need to turn this on periodically to find the try dead code.
#![allow(dead_code)]
#![feature(test)]
extern crate test;

#[macro_use]
extern crate lazy_static;

extern crate fnv;

#[macro_use]
#[cfg(windows)]
extern crate winapi;

//#[cfg(test)]
mod test_e2e;

#[macro_use]
mod profile;

mod input;
mod dnclr;
mod hookd3d9;
mod util;
mod interop;

pub use interop::{OnInitialized, SaveTexture};
pub use interop::{LogError, LogInfo, LogWarn};

pub use hookd3d9::Direct3DCreate9;
