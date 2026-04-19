# ModelMod - Claude Code Notes

## Rust Build

This is a Windows-only project, but cross-compilation works from Linux, although you won't be able to run the linker as it requires MSVC.

- Workspace root: `Native/`
- To check Rust code compiles: `cd Native && cargo check --target x86_64-pc-windows-msvc`
- Always use `--target x86_64-pc-windows-msvc` — without it, cargo will target Linux and fail
- Runtime tests (`cargo test`) won't work on Linux since they call Windows APIs, but compilation checks are valid

## F# Build (MMManaged.sln)

- If you are running in linux container you probably won't be able to build this code, since it requires some version of visual studio 
(2019 or 2022) to be installed and this is generally not feasible on linux.  It you have access to the "dotnet" tool you could try building with that, though this isn't how the code is normally built, it may be sufficient to check if it compiles at least.

## Interop notes

### versions

- any time the interop wire protocol changes (for instance the types in InteropTypes.fs are affected or changed, 
or an import function is added or removed, or has its arguments changed), the native code versions should be 
bumped.  These are 
	- NativeCodeVersion in Interop.fs 
	- NATIVE_CODE_VERSION in dnclr.rs
- These must be set to the same value or else the managed code will not load.	
- it is sufficent to bump these once per branch that contains these kinds of changes (do not need to repeatedly bump on each commit to a branch).

### call strategy

The general pattern is the native code drives the managed code via the managed callbacks.  The managed code can call back into native code,
and this happens for instance for logging and requesting textures be saved.  
But I am trying to limit this due to potential for undefined behavior if, for instance, global state needs to be locked mutably in both the initial call and the re-entrant call.  For new code 
a pattern should be preferred where if native code needs something from managed, a new managed callback should be added that native can call to 
obtain that data, rather than managed code calling a function to "push" it to native.  
