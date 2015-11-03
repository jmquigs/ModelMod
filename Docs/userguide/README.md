## Introduction

This document outlines how to use ModelMod.  First it covers some
important game-compatibility information.  Later sections go into more detail
about the types of mods you can do, and some of the limitations of the system.

## Game compatibility

ModelMod is not compatible with all games.  This is because it makes
assumptions about how games render data, and right now these assumptions
are fairly specific.  To see if a game you are interested in will work, you can use this video as a guide:

https://www.youtube.com/watch?v=3Mvqcv3-OPs

Here is a summary of that video:
* ModelMod uses DLL injection to get its renderer into the game.  Some games
are not compatible with the injection method used.  
* Is the game using D3D9 for rendering?  You can check this by starting the game under modelmod and checking the log.  If you see "Direct3DCreate9 called"
there is a good chance that it is using D3D9 rendering.
* Did the CLR initialize successfully?  In the log, Look for "Starting CLR",
followed by "Initializing managed code"; if you see this, the CLR is ok.
* When in game, are the texture selection keys working?  You may need to switch the input profile (and restart the game).  If neither works, the game is
probably not compatible.
* Can you take a snapshot?  
* Can you see the snapshot in the preview window?  If not, that may just mean the snapshot is out of view, but will still be editable.  Or it could mean the game is using CPU-animated meshes; if so, the game is not compatible.  

## The Launcher

The ModelMod launcher allows you to create a profile for each game you
want to run, and change the settings for each.

Some tips for using the launcher:

* Make sure the exe path is the actual game
executable and not some its own launcher app.  It may take some
trial and error to determine this.  
* If the game uses its own launcher, you may need to
increase the "Launch Time" setting in the ModelMod profile so that
the injector has time to attach to the game.
* DLL injection can fail intermittently because it is sensitive to race conditions;
if it fails once, try running from ModelMod again. Repeated failures likely
indicate that the game is not compatible.
* With some games, the F key layout is incompatible.  Try the punctuation key
layout if you have problems.  In the future, input layouts will hopefully
be customizable from the UI.

## Mesh animation

If you've done 3D animation before, you may be used to rigging a skeleton
to your mesh in order to animate it.

ModelMod animation doesn't quite work like this.  We don't have access to the
actual skeleton; we only have indirect access to it, via the geometry that the
game is rendering.  When you make a mod and use "Ref" weight mode
(the default), ModelMod will determine how to weight each mod vertex at load
time by using weighting data from the nearest ref vert.  This works well for
many cases, but it isn't as flexible as using a full rigging.  

Some immediate consequences of this are

1) The animation data you have available depends on the the geometry being
rendered.  For example, if you have snapshotted a piece of chest armor that
doesn't include
geometry for the hands, you won't be able to add gloves to that model, because
you don't have hand weights in the ref.  You would need to snapshot the gloves
and model them separately.

2) Your mod verts may be weighted incorrectly.  For example, if you have a large
armor pauldron piece that extends far out from the mesh, the nearest vert for
weighting purposes may not be the shoulder; it may, for instance, be somewhere
on the elbow.  This can lead the the piece twisting incorrectly in game.  
ModelMod provides some control over this process using named vertex groups,
discussed below.

### Named vertex groups

This feature allows you to gain some control over the nearest-vert selection
process.  You can use this feature in two ways:

1) If there are some vertices in the ref that are causing problems, you can
completely exclude them by adding them to a vertex group called "Exclude".
Alternatively, you can add the vertices to a named group in the Ref, "RightElbow",
then create a corresponding group in the mod containing verts that you don't
want affected, and add those to a group called "Exclude.RightElbow".

2) Similarly, you can force inclusion of certain verts.  Suppose you have a long
pauldron as described above that is being influenced by verts on the elbow.  
You can define a group in the ref called "UpperBody", then define
a group in the Mod called "Include.UpperBody" and add all the pauldron verts
to that group.  Now the pauldron will only be affected by upper body verts.

This feature requires some concentration to use properly, but it can produce
good results.  An important note is that after you change the Ref groups,
you must remember to re-export the Ref file so that the group names will be
available to the modelmod loader.  The process generally involves going back
and forth between the ref and the mod, updating the groups and re-exporting
both as needed.

### "Mod" weight mode

You can configure a mod to use "mod" weight mode instead of relying on a Ref.
In this instance, all the blend data comes from the mod, which means that every
mod vert must be assigned to an appropriate "Index.XX" vertex group.

While this mode doesn't require a Ref, it is much more difficult to use, for
the following reasons:
1) You generally can't use symmetry tools provided by blender, because the
weight groups are usually not symmetric (e.g left arm might be in Index.15, and
right arm might be in Index.9).
2) You must make sure that each new vert you add is properly weighted.

For these reasons, this weight mode is not recommended for most users.
