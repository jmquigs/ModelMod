## Dev notes

Its been a while since I wrote most of this code and I don't really
remember all the details of what is going on everywhere.  So I made this
document to record any new investigations into changing things
and what issues I found.

Could use github issues/wiki for this kind of note-taking but I tend to
lose track of information in there.  However I will cite any issues that
are relevant here.

### 2/21/2022: Deferred managed mod loading

As of this writing, native code will only create D3D resources for mods
that are actually used in the scene.  However once loaded, this memory
is not freed (there is no garbage collection for mods that subsequently
become unused).  Ctrl-F1 will clear all of this memory, though mods in
use by the current scene will immediately be reloaded.

Managed code still loads mesh data for all mods on startup.  I looked at
converting some of this to use Lazy loading (via F#'s `lazy` function),
but its a non-trivial change.  First issue is that primitive and vertex
counts generally come from the meshes.  This means the full mesh needs
to be loaded to get these values, which is slow.  It would have been nice
if the mmobj exporter wrote out the vert/prim counts in a comment line
at the top of the file, but I didn't do that back in the day.

I could probably get away with not knowing the mod vert/prim counts
(they are used for variant processing but maybe not needed early).
However the ref vert/prim counts are needed for native code to
determine when a given rendered primitive actually has some mod available
to replace it.  So at a minimum all the ref meshes need to be loaded.

An option would be to implement additional metadata cache files for each
`.mmobj` on disk.  So `.mmobj.cache` or something like that.  This could
store the vert/prim counts and anything else needed.  There is already
code for detecting whether mmobj files have changed on disk, so this
could just be an extension of that.  However the cache file would
probably need to write in an mtime or something so that I don't need to checksum the whole mmobj file, which would be slow.  And mtime-based
caching can be a little unreliable, esp when using other software that
"preserves" mtimes.  So I'm not going to do this now.

A related item is the `MeshRelation` objects.  These aren't needed until
the mod is actually being prepped for render.  I did a test where I
replaced the internal `VertRel` used by this class with a `Lazy`, and it
did help reduce load times especially for unused mods.  However,
because of the way the code is structured, we won't actually create the
value until fillModData is called, and then it will block the native thread for potentially 100s of ms, just waiting on managed code, which I don't like to do.  So I'd need some scheme where I can start an async
process for that and return "in-progress" to the native code.  This would
likely require a new interop API because fillModData needs fully allocated D3D data structures, which would then go unused if
the managed code didn't have any data to fill them with.  So, native code
would need to use a new api function to actually start the deferred load and check on its progress for an individual mod.  Right now there is a global mod loading API, but not one for individual mods.

For now I not going to implement deferred/incremental mod loading in the managed code.



