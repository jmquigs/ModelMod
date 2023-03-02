#[macro_use]
extern crate lazy_static;

extern crate types;
extern crate fnv;

mod global_state;
/// Contains DX11 render state
pub mod dx11rs;
pub use global_state::*;
