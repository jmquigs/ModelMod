## Introduction

This document outlines how to use ModelMod.  First it covers some
important game-compatibility information.  Later sections go into more detail
about the types of mods you can do, and some of the limitations of the system.

## Caution / Disclaimer

ModelMod is a back-door method of getting alternate art into a game.
Many games specifically prohibit use of this kind of program in
their terms of service.  That's the one that you "agreed" to when you
installed the game.  

The risk is greatest with MMOs; depending
on the attitude of the game developer and/or publisher, using
ModelMod may be totally fine, or it may [get your account banned](http://arstechnica.com/gaming/2015/08/is-it-just-a-game-mod-or-is-it-facilitating-piracy/).  The developer can detect the
use of ModelMod, provided they are willing to make changes to their
game to do so.

In other words, use at your own risk.  I don't recommend using ModelMod with any game that has onerous T.O.S., which includes most MMOs, without prior consent of the copyright holder.

## Game compatibility

ModelMod is not compatible with all games.  This is because it makes
assumptions about how games render data, and right now these assumptions
are fairly specific.  

The top three limiting factors are as follows, all of these may improve over
time:

1) It requires GPU-based animated meshes.  This is common in modern
games, but older games primarily use CPU-animation.  

2) It requires D3D9 for rendering.  

3) It uses DLL injection, which can be a temperamental process.

#### Testing compatibility

To see if a game you are interested in will work, you can use this video as a guide:

https://www.youtube.com/watch?v=3Mvqcv3-OPs

Here is a checklist from that video:
* Did the DLL inject properly? Some games
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
want to run, and change the settings for each.  It also is the easiest way to
create mods from snapshots, though this can be done manually.

Some tips for using the launcher:

* Make sure the exe path is the actual game
executable and not some launcher app.  It may take some
trial and error to determine; the actual game process is usually the one
consuming the most CPU (as measured by Task Manager) after the game starts.
* If the game uses its own launcher, you may need to
increase the "Launch Time" setting in the ModelMod profile so that
the injector has time to attach to the game.  If the game requires an
intermediate process (e.g. Steam), its best to start that process first to
reduce the chance that injection times out due to slow startup.
* DLL injection can fail intermittently because it is sensitive to race conditions;
if it fails once, try stopping the game and starting from ModelMod again.
Repeated failures likely
indicate that the game is not compatible.
* With some games, the F key layout is incompatible.  Try the punctuation key
layout if you have problems.  In the future, input layouts will hopefully
be customizable from the UI.

## Snapshots and Mods

ModelMod writes out several files with each snapshot.  The launcher tool has a
"Create Mod" button that lets you turn these into mods.  This tool also adds
the mod to the "ModIndex" file for the game, so that it will be loaded.

The CreateMod tool provides a rudimentary preview of the snapshot files; however
you can also import them into blender for a better view.

Once a mod is created, the two most important files of a mod are the XXMod.mmobj and XXRef.mmobj files.
Here, "XX" is the name you give the mod when creating it in the launcher.

For basic mods, you only edit the XXMod.mmobj file.  Import it into blender, then
export out the same file.  Use the reload key in game to load the mod.  

#### Using Mods in game

ModelMod will load all mods that are listed in the ModIndex.yaml file in the game's
mod directory.  An easy way to prevent load is to simply comment a line out
by adding a '#' to the beginning of the line.

The program uses the base primitive and vertex count of the original snapshotted
data in order to figure out when to display the mod.  For example, if your
original snapshot (the ref file) had 500 primitives and 1000 verts,
and you load a mod for it,
_any_ time something with that primitive and vert count is rendered, your mod
will be rendered instead.

Beware that this means low poly count geometry may have false positives.  Also,
you don't have control over instancing; e.g. if the game draws all character
heads with the same base model, you can't modify just one head.  Future versions
of ModelMod may add support for additional constraints, such as texture
checksums, which should allow for some control over instancing.  

At any time during the game, you can use the input keys to toggle mod display.
This is helpful if some mod is causing a rendering glitch.

You can reload mods at any time using the CTRL-F1 key (in the F key layout).
To speed up reload time, this only reloads mods whose mmobj timestamps have been
updated since the last load.  Keep this in mind if you are renaming files to
  test something: renaming doesn't change the timestamp, so the cache can get stale.
    To force a full reload, or to reload configuration
changes made in the launcher, use CTRL-F10.

#### Textures

ModelMod will attempt to snapshot the textures in use so that they are available
in Blender.
However, it assumes that you don't normally want to change the
texture for the mod.  So changing the texture in blender won't change it in game.

If you want to use different textures, you can edit the XXMod.yaml
file and set textures as follows:
```
Type: Mod
...
Tex0Path: mybasetex.dds
Tex1Path: mynormalmap.dds
```

The number in each path is the "texture stage" on which the texture should be
set.  These should match what was originally produced by the snapshot, because
the shader expects certain textures on certain stages.

In some games, ModelMod is currently unable to snapshot textures.

#### Shaders

ModelMod does not currently support shader modding.  There are a lot of issues
that need to be dealt with here; see the Dev guide for more details.

## Mesh animation

If you've done 3D animation before, you may be used to rigging a skeleton
to your mesh in order to animate it.

ModelMod animation doesn't quite work like this.  We don't have access to the
actual skeleton; we only have indirect access to it, via the mesh that the
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
discussed below, and also demonstrated in the intro video.

### Named vertex groups

This feature allows you to gain some control over the nearest-vert selection
process.  You can use this feature in two ways:

1) If there are some vertices in the ref that are causing problems, you can
completely exclude them by adding them to a vertex group called "Exclude".
Alternatively, you can add the vertices to a named group in the Ref, "RightElbow",
then create a corresponding group in the mod containing verts that you don't
want to be affected by the RightElbow, and name that group
"Exclude.RightElbow".

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

It is an error to have a mod "Include" a group that doesn't exist in the ref.
The mod will fail to load.  

### "Mod" weight mode

You can configure a mod to use "mod" weight mode instead of relying on a Ref.
In this instance, all the blend data comes from the mod, which means that every
mod vert must be assigned to an appropriate "Index.XX" vertex group.

While this mode doesn't require a Ref, it is much more difficult to use, for
the following reasons:

1) You generally can't use symmetry tools provided by blender, because the
weight groups are usually not symmetric (e.g left arm might be in Index.15, and
right arm might be in Index.9).

2) You must make sure that each new vert you add is assigned to one of the
Index.NN groups with a valid weight.

For these reasons, this weight mode is not recommended for most users.  The
main reason its documented here is to explain the problems with it.  
