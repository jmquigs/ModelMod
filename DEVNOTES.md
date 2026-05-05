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


