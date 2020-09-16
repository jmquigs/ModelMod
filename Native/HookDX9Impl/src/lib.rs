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

extern crate shared_dx9;
extern crate snaplib;

//#[cfg(test)]
mod test_e2e;

#[macro_use]
mod profile;

mod dnclr;
mod hookd3d9;
mod input;
mod interop;
mod util;
//mod constant_tracking; // disabled for now, using the one in snaplib
mod shader_capture;
mod d3dx;

pub use interop::{LogError, LogInfo, LogWarn};
pub use interop::{OnInitialized, SaveTexture};

pub use hookd3d9::Direct3DCreate9;
pub use hookd3d9::D3DPERF_BeginEvent;
pub use hookd3d9::D3DPERF_EndEvent;
pub use hookd3d9::D3DPERF_SetMarker;
pub use hookd3d9::D3DPERF_SetRegion;
pub use hookd3d9::D3DPERF_QueryRepeatFrame;
pub use hookd3d9::D3DPERF_SetOptions;
pub use hookd3d9::D3DPERF_GetStatus;
