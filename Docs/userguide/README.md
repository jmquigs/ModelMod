# Introduction

# Understand the limitations

The first thing to realize about ModelMod is that may not work out of the
box with your favorite game.  ModelMod does significant work to make geometry
swappable and some portion of this work usually needs to be coded in a
game-specific way.  In the "Troubleshooting ModelMod compatibility" section
below, you will find information on how to determine if ModelMod can work
with your game.

## Troubleshooting ModelMod compatibility problems

You have some options if ModelMod doesn't work out of the box with your game.

First you should try to determine if the game is fundamentally not supported
at this time due to a known limitation, which can be one or more of the following:

1) It cannot use D3D9 for rendering.  If you like, add the game to a list in this issue: XXX

2) It uses software-animated models.  If you like, add the game to a list in this issue: XXX

3) DLL injection fails.  Please create a new issue for this and attach the modelmod log for the game.

4) CLR injection fails.  If you like, add the game to a list in this issue: XXX

If the game fails for any of these reasons, you are probably stuck for now,
but the issue report will help others work on it in the future.

If the game fails for some other reason (i.e it fails to take snapshots),
you have some more options:

If you are a programmer or want to learn programming, you can try taking a crack at making the changes yourself.  Or you can try getting a programmer friend who has the same game to work on it.

Either way, you may want to file a bug in the ModelMod source repository.  
Include as much information as you know about the failure.  For instance,
if a snapshot fails, include the relevant sections of the ModelMod log including the stack trace.  This will allow the developers to assess the difficulty of porting the game and suggest.

3) Or, you can create a ticket in the ModelMod repo noting that.


### MORE STUFF TO TALK ABOUT:

Verify that CLR is working.

Once in game, typically you want to snapshot some art.  The native render maintains a list of textures that have been since the list was last cleared
by the clear-texture input command.  Two other keys allow you to select
the next/previous texture in this list.

The Ref MMobj file in particular should always contain the geometry
originally displayed by the game.  The only valid change that you may
make to this file is to group certain

The name of a mod (or reference) is its base file name.
















































# stuff
