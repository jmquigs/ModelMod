# ModelMod - Claude Code Notes

## General notes

- Commit messages: put the one-line summary on the first line, then a blank line, then an
"Authored by Claude" attribution line which if possible includes your model name and version,
then a blank line, then the body.  Do not put the attribution in the summary line.  For example:

```
publish d3d11 context hook table via atomic ptr

Authored by Claude (claude-opus-5)

get_hook_context took a DEVICE_STATE read lock and returned
HookDirect3D11Context by value, so every hooked context fn paid an RwLock
acquire plus a memcpy of the whole fn-pointer table.
```

## Rust Build

This is a Windows-only project, but cross-compilation works from Linux, although you won't be able to run the linker as it requires MSVC.

- Workspace root: `Native/`
- To check Rust code compiles: `cd Native && cargo check --target x86_64-pc-windows-msvc`
- Always use `--target x86_64-pc-windows-msvc` — without it, cargo will target Linux and fail
- Runtime tests (`cargo test`) won't work on Linux since they call Windows APIs, but compilation checks are valid

## F# Build (MMManaged.sln)

Normally built with visual studio (2019 or 2022) on windows.  The `*.dotnet.fsproj` variants
(and `MMAll.dotnet.sln`) also build and test on linux, which is enough to catch compile errors
in F# changes.  This isn't how the code is normally built, so treat a windows build as the
source of truth, but don't skip the linux build just because there's no visual studio.

Setup, on ubuntu 24.04 (this worked in the claude code web container; the ubuntu archive and
nuget.org were reachable even though `builds.dotnet.microsoft.com` was blocked, so use the
distro package rather than the dotnet-install.sh script):

```
apt-get install -y dotnet-sdk-8.0
```

The projects reference third party assemblies by HintPath into `packages/`, which paket would
normally populate.  Paket's restore target shells out to mono and will fail the build before
the compiler runs, so disable it and fetch the packages directly:

```
dotnet build MMManaged/MMManaged.dotnet.fsproj -p:PaketRestoreDisabled=True
```

Packages needed under `packages/` (download the .nupkg from
`https://api.nuget.org/v3-flatcontainer/<id-lowercased>/<ver>/<id-lowercased>.<ver>.nupkg`
and unzip it into the named dir):

- `packages/MonoGame.Framework.WindowsDX` <- MonoGame.Framework.WindowsDX 3.3.0 (also supplies SharpDX)
- `packages/YamlDotNet` <- YamlDotNet 5.1.0
- `packages/FsPickler.5.3.2` <- FsPickler 5.3.2 (MMManaged.Engine only)

The NUnit console runner used below comes from the same place: NUnit.ConsoleRunner 3.16.3,
`tools/nunit3-console.exe` inside the nupkg.  It doesn't belong in `packages/`; unzip it
somewhere scratch.

To run the NUnit tests on linux: `dotnet test` does *not* work, because the distro SDK ships no
net-framework test host.  Install `mono-complete`, copy `FSharp.Core.4.4.3.0/FSharp.Core.dll`
into the build output dir (it is referenced with Private=False so it isn't copied there), and
run NUnit's own console runner:

```
dotnet build Test.MMManaged/Test.MMManaged.dotnet.fsproj -p:PaketRestoreDisabled=True
cp FSharp.Core.4.4.3.0/FSharp.Core.dll Debug/
cd Debug && mono /path/to/nunit3-console.exe Test.MMManaged.dll
```

Known: 6 tests fail on linux regardless of the change under test (TestMesh, TestMeshTransform
x2, TestModDB, TestModDBInterop, TestYaml).  They all die in `Util.TestDataDir`'s static
constructor, which searches windows-style relative paths for TestData.  Compare against a
master build before assuming a failure is yours.

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
