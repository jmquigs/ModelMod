/*!
 * Contains type declarations for DX11 and DX9.
*/
#![allow(non_snake_case)]

//#[macro_use]
#[cfg(windows)]
#[macro_use]
extern crate winapi;

#[macro_use]
extern crate lazy_static;

pub mod defs_dx11;
pub mod types_dx11;
pub mod defs_dx9;
pub mod types_dx9;
pub mod error;
pub mod state;
pub mod util;
pub mod types;