
extern crate winres;
extern crate rustc_version;

fn main() {
  if cfg!(target_os = "windows") {
    let res = winres::WindowsResource::new();
    // can't set an icon because its a DLL, but winres will still pull
    // values out of cargo.toml and stick them in the resource.
    res.compile().unwrap();
  }
  println!("cargo:rustc-env=RUSTCVER={}", rustc_version::version().unwrap());
  println!("cargo:rustc-env=RUSTCDATE={}", rustc_version::version_meta().unwrap().commit_date.unwrap());
}