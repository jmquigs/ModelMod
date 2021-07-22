
extern crate winres;

fn main() {
  if cfg!(target_os = "windows") {
    let res = winres::WindowsResource::new();
    // can't set an icon because its a DLL, but winres will still pull
    // values out of cargo.toml and stick them in the resource.
    res.compile().unwrap();
  }
}