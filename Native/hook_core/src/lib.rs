#![allow(static_mut_refs)]

#![allow(non_snake_case)]
// this is here to silence the spammy warnings from the COM macro definitions in dnclr.
// need to turn this on periodically to find the try dead code.
#![allow(dead_code)]
//#![feature(test)]
//extern crate test;

//build.rs sets these, we log them to the logfile on startup.
const RUSTCVER:&str = env!("RUSTCVER");
const RUSTCDATE:&str = env!("RUSTCDATE");
const GIT_HASH:&str = env!("GIT_HASH");
const BUILD_TS:&str = env!("BUILD_TS");

extern crate fnv;

#[cfg(windows)]
extern crate winapi;

extern crate lazy_static;

extern crate shared_dx;
extern crate global_state;
extern crate util;
extern crate input;
extern crate constant_tracking;
extern crate d3dx;
extern crate types;

#[macro_use]
extern crate profiler;

pub const ENABLE_DEBUG_SPAM:bool = false;
pub const DEBUG_SPAM_TO_STDERR:bool = false;

mod debugmode;
mod hook_render;
mod hook_render_d3d11;
mod input_commands;
mod hook_device;
//mod hook_constants;
mod mod_render;
mod hook_device_d3d11;

pub use interop::{LogError, LogInfo, LogWarn};
pub use interop::{OnInitialized, SaveTexture};

pub use hook_render::Direct3DCreate9;
pub use hook_render::D3DPERF_BeginEvent;
pub use hook_render::D3DPERF_EndEvent;
pub use hook_render::D3DPERF_SetMarker;
pub use hook_render::D3DPERF_SetRegion;
pub use hook_render::D3DPERF_QueryRepeatFrame;
pub use hook_render::D3DPERF_SetOptions;
pub use hook_render::D3DPERF_GetStatus;
