# ModelMod - Claude Code Notes

## Rust Build

This is a Windows-only project, but cross-compilation works from Linux.

- Workspace root: `Native/`
- To check Rust code compiles: `cd Native && cargo check --target x86_64-pc-windows-msvc`
- Always use `--target x86_64-pc-windows-msvc` — without it, cargo will target Linux and fail
- Runtime tests (`cargo test`) won't work on Linux since they call Windows APIs, but compilation checks are valid

## General Notes

- any time the interop wire protocol changes (for instance the types in InteropTypes.fs are affected or changed, 
or an import function is added or removed, or has its arguments changed), the native code versions should be 
bumped.  These are 
	- NativeCodeVersion in Interop.fs 
	- NATIVE_CODE_VERSION in dnclr.rs
- These must be set to the same value or else the managed code will not load.	
- it is sufficent to bump these once per branch that contains these kinds of changes (do not need to repeatedly bump on each commit to a branch).


