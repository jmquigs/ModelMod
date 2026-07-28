## Dev notes

Its been a while since I wrote most of this code and I don't really
remember all the details of what is going on everywhere.  So I made this
document to record any new investigations into changing things
and what issues I found.

## Build Stuff

### Paket install issue

Newer versions of paket seem to break the old visual studio project files (any project file that doesn't have ".dotnet." in the name) when 
`paket install` is run.  The project will load in VS,
but most of the references seem broken (many types will be undefined).  Only workaround I have 
found is to just revert the changes paket makes 
to these files.

Generally there seems to be a conflict between how I have 
specified FSharp Core versions. I used a somewhat older version for compatibility with FsPickler, because it was throwing errors at run time.  That version (4.4.3.0) is committed to the repo, but some other deps seem to want a slightly newer version (4.6+).  I have not yet figured out how to untangle this.

### Managed code dotnet/VisualStudio build switching

After building with `dotnet` (which isn't required but is handy for rebuilds on, for instance, linux), VS will no longer be able to build the managed code because the files left behind by `dotnet` in the "obj" intermediate directories confuse it.  The obj directories need to be cleared out manually to fix this.

Here is a git bash command that can do this (the cd is to make sure it is run from the correct directory)

    cd ../ModelMod && rm -rf `find . -type d -name "obj"`

Similarly after switching back to dotnet from VS,
it is advisable to rebuild the MMAll solution, though it doesn't seem like any manual clean step is needed when going in that direction:

`dotnet build MMAll.dotnet.sln -c Release`

When using Ionide in VSCode, configuration to to load MMAll.

MMAll notably does not include the gui projects (mmview, mmlaunch, and wpfinterp) since I don't yet know if I can build these on linux.

### Dot net hell

I tried updating Nunit to version 3 to fix a warning the test project produces, but this quickly got me into a hell state of 
also needing to upgrade fake, fsunit, etc and paket rampaging in the project files, producing new versions that VS2019 
couldn't build.  Note that at one point the github action build was succeeding even though the test subtask was actually failing,
so that is something to watch out for if I try this again.

Might be worth trying again with local claude code someday, but the web version isn't helpful for this since it can't build dotnet.

## Code Stuff

### Static mut

The rust code makes heavy use of obtaining references to a global static mut (GLOBAL_STATE).  For years this was not warned 
about by the compiler at all, but is now considered a very serious problem.  These days you can get an LLM to generate a 
sample that will show the failure in rust playground - often only shows up in release build there, since it stems from LLVM
optimization.  

I haven't seen any cases where this causes problems in modelmod, and I do try to lock around GLOBAL_STATE with GLOBAL_STATE_LOCK, except in the draw indexed path where I thought it would be too slow.  However I'm in the process of slowly untangling and fixing this.

For now the warning is disabled in most places, but it still comes up now and again.  As I fix areas in the code that have 
this problem I plan to remove the warning.

### MeshRelation building

This was slow for a long time but I had claude put in an optimization which speeds it up pretty dramatically (c9a8fd2).  
I also had claude generate a simple spatial database to avoid the exponential loop in there, but it wasn't a clear win - on some mods its faster but on others it is slower.  Rather than dig into that I just left that out.

Mesh relations are also cached now as noted below.  

### Deferred mod loading

As of this writing, native code will only create D3D resources for mods
that are actually used in the scene.  However once loaded, this memory
is not freed (there is no garbage collection for mods that subsequently
become unused).  Ctrl-F1 will clear all of this memory, though mods in
use by the current scene will immediately be reloaded.

At one time the code loaded all mod data up front (and even did d3d resource creation on the game's main d3d draw thread, causing a visible stutter in game).  Fortunately all that is deferred now.
The YAML files (refs in particular) now contain the counts for prims and verts that could be modded.
Since all the YAML is loaded at startup these counts are available to native code from the start, 
so it tracks what is actually used by a mod and only requests those mods complete loading after some draw call would try to render them.

The managed code will now load those in separate threads, so its possible for it to be loading multiple mods at once.  The mesh relation is also built inside one of these threads, so no longer blocks loading of everything as it did in olden times.  Building the mesh relation is still an extremely slow n-squared-ish process since there is no spatial database available, and the use of 
include/exclude groups (which are practically required for any significant mod) adds further 
slowness.

On that topic, the code now also has a cache for the mesh relation data itself, so its only rebuilt if one of the two underlying meshes (either ref or mod) changes.  This substantially speeds up loading up since most mods aren't changing most of the time.

The native code will also now query the device (at least on DX11) to see if it supports multithreading and if so, will create the modelmod d3d resources on a separate thread.  This eliminates most in game pauses when a mod is loaded.

There are still some parts of loading that are slow.  If tangents/normals need to be updated, that data isn't 
cached so is redone every time.  And also filling the d3d mod data needs to happen each time, and that can get slow (like on my laptop which I keep in a low power state generally).  There probably isn't a whole lot that can be done to speed up the mod data fill other than rewriting it all for speed or even moving the whole thing to rust.  Claude has added hot-reload (ctrl-F10) back in for the 
managed code so this diminishes the utility of moving this code to rust, since the native code can't 
be hot reloaded.  The ability to reload the managed code is very useful during the process of adding support for a new game, since often tiny tweaks need to be made to formats and such and its very 
tedious to restart the whole game just for those.

### Dynamic buffer snapshots (the `snapshot-dynamic-buffers` feature)

Some DX11 games don't give each mesh its own vertex/index buffer.  Instead they pack many meshes into a few large buffers (a "megabuffer") which are created empty and filled in later via `Map`/`Unmap` (often with `WRITE_NO_OVERWRITE`) or `UpdateSubresource`.  `hook_CreateBuffer` never sees that data, since there is no `pInitialData` at creation time, so snapshotting such a mesh fails with "failed to get vertex buffer data, was not previously saved".  And even when the data is there, the snapshot code required that the bound buffers contain exactly one mesh: it checked that the index buffer was exactly `prim_count * 3` indices and the vertex buffer exactly `num_vertices` verts, and bailed out otherwise.

The `snapshot-dynamic-buffers` Cargo feature is an experimental attempt at handling this.  It lives in `hook_core` and transitively enables the same-named feature in `hook_snapshot`.  It is off by default.

When it is on, `hook_core` hooks `Map`, `Unmap` and `UpdateSubresource` on the context vtable (see `hook_dynamic_buffers.rs`) and copies the buffer bytes out whenever the game writes them.  `hook_CreateBuffer` also starts recording `(is_index_buffer, byte_width)` for every VB/IB in `device_buffer_meta`, so those hooks can cheaply tell a tracked mesh buffer apart from the far more common constant buffer maps without doing real work in the hot path.

The feature also gates the three `real_map`/`real_unmap`/`real_update_subresource` fields on `HookDirect3D11Context` (and so `shared_dx` has a `snapshot-dynamic-buffers` feature of its own, which the other two enable transitively).  That struct is `Copy` and `get_hook_context()` returns it *by value* on every `DrawIndexed` and every `IASet*`/`PSSetShaderResources` call, so the three extra pointers grow a per-draw copy from 112 to 136 bytes.  That is far too small to actually be measurable, but gating the fields keeps the default build's hot path byte-for-byte what it was before the feature existed, which means the feature can't be blamed for a frame rate change when it's off.

The other half is in `hook_snapshot::set_buffers_d3d11`, which stops requiring the one-mesh-per-buffer layout and instead slices the drawn sub-region out of whatever is bound.  It walks the `prim_count * 3` indices starting at `start_index`, finds the min and max vertex they reference, copies just that vertex range out, re-bases the indices to zero, and rewrites `num_vertices`, `base_vertex_index`, `min_vertex_index` and `start_index` in the snapshot data so that managed code sees a self-contained mesh starting at offset 0.  Any bind offsets from `IASetIndexBuffer`/`IASetVertexBuffers` get folded in.  Without the feature that function keeps the original behavior, strict size checks and all.

The reason this isn't on by default is cost.  The Map/Unmap path copies these buffers, which are usually large, every time the game touches them, and that slows the game down a lot.  It's only worth paying for occasional manual snapshotting of a game that needs it.  The slicing path also throws away the old strict size checks, which are a useful sanity check on games that do use one buffer per mesh.

To build with it, pass `--features=snapshot-dynamic-buffers` from `Native/hook_core`; the `rb.sh` and `r2026g1.sh` helper scripts take a feature name as their first argument.

One subtlety: the hooks are installed whenever the feature is compiled in, but their bodies do nothing unless `run_conf.precopy_data` is set.  That is deliberate.  The context vtable is copied once at hook time, so gating on the runtime precopy flag inside the hook bodies lets the existing precopy toggle (the `SnapPreCopyData` registry value, or pressing the clear-texture-lists key, which force-enables it) work without having to re-hook anything.

Related but not gated by the feature, `hook_snapshot::take` now warns when `num_vertices` is larger than `prim_count * 3`, which is the most verts an indexed triangle list draw can possibly touch.  When that fires, `num_vertices` almost certainly came from `vb_size / vert_size` (i.e. the whole bound buffer) rather than the draw's actual range, which is a decent hint that you're looking at a shared buffer, or at CPU/software animated geometry that isn't modable anyway.


