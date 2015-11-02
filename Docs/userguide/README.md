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
