
extern crate winres;
extern crate rustc_version;
extern crate chrono;

fn main() {

  use std::process::Command;

  if cfg!(target_os = "windows") {
    let res = winres::WindowsResource::new();
    // can't set an icon because its a DLL, but winres will still pull
    // values out of cargo.toml and stick them in the resource.
    res.compile().unwrap();
  }

  // https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program
  let output = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap();
  let git_hash = String::from_utf8(output.stdout).unwrap();
  println!("cargo:rustc-env=GIT_HASH={}", git_hash);

  // build timestamp
  let build_ts = chrono::offset::Local::now();
  println!("cargo:rustc-env=BUILD_TS={}", build_ts);

  println!("cargo:rustc-env=RUSTCVER={}", rustc_version::version().unwrap());
  println!("cargo:rustc-env=RUSTCDATE={}", rustc_version::version_meta().unwrap().commit_date.unwrap());
}