# The Dev Guide

## Development environment setup

### Install

* June 2010 DirectX SDK: http://www.microsoft.com/en-us/download/details.aspx?id=6812
(I hope to eliminate this eventually, but right now it is required for some legacy d3dx code)
* Visual Studio 2015 Community (or any 2015 variant):
https://www.visualstudio.com/en-us/products/vs-2015-product-editions.aspx
* Visual F# power tools (optional but recommended) https://fsprojects.github.io/VisualFSharpPowerTools/
(can be installed via Visual Studio Extensions menu)

### Setup
* Run "installdeps.bat" to install nuget packages
* Choose a solution:
  * MMDotNot.sln: contains managed code and UI tools.  
  * ModelMod.sln: contains native library and injection tool.  If you are not
  modifying the interop layer, you may not need to build this, and can probably
  just use the binaries (MMLoader.exe,ModelMod.dll) from a release package.
  Build the MMDotNet.sln first, then copy the native binaries into the
  "Release" directory.

## Technical Overview

This section describes some of the technical details of ModelMod for
programmers and advanced users.  If you haven't already done so, its a
good idea to read the User's Guide first.

You can think of ModelMod as a "sed" or "find/replace" program for artwork.
The ModelMod renderer watches all the geometry that is being drawn by the game;
when a particular draw call triggers a match with a loaded mod, the system
substitutes the mod for the original geometry.  

Currently a match is anything
that matches the defined primitive and vertex count of the mod; this can lead
to some false-positives, however, for things like character meshes, the
vertex + primitive is fairly unique.  However, this does have some consequences:
* Anything that matches the vert/prim count will be replaced.  Therefore
all instances of that thing in the game (if drawn multiple times) will be
replaced, and there is no way to control this.
* It is not possible to modify a single object that is some basic primitive,
like a cube.  You wouldn't be able to avoid changing all the cubes in the game.
* Objects that have very regular geometry patterns (such as particle emitters)
are difficult to mod, because the basic geometry is reused for many kinds of
particle effects.

The program is essentially an alternate asset load pipeline.  Other than
input processing, it does not do any per-frame operations.  Some future
mod types may require that, however, specifically support for software-animated
meshes (common in older games.)

The remainder of this document describes the different phases of ModelMod.

## Components

ModelMod has the following binary components:
* MMManaged.dll: contains the core F# snapshot and mod loading code
* ModelMod.dll: contains c++ native code, including wrappers for D3D COM,
and code for initializing the CLR and loading MMManaged.dll.
* MMLoader.exe: watches for appearance of the target program in OS process
list, and ensures that ModelMod.dll is loaded into that program.
* MMLaunch: WPF application that allows you to configure ModelMod for use
with a target game.  It directly executes MMLoader and manages the loader
life cycle.

## Injection

The first phase of ModelMod is injection.  The native code in ModelMod.dll must
be inserted into the target process.  It used to be that one could simply
launch the target process directly with all threads suspended, and then patch
up certain core functions so that they load your target DLL.  With the advent of
Steam games and other self-updating technologies, this approach no longer works,
because the game will often re-launch itself and you lose your patched code.

The MMLoader program works around this problem.  It
continuously monitors the OS process
list, looking for a target process by executable name.  Once found, the monitor
immediately suspends the program.  It them patches the code to load ModelMod.dll
and resumes thread execution.  Usually this works, but since the process is
inherently racey, it can fail; in the event of failure, ModelMod just lets the
target run unpatched to avoid data corruption.  The user must restart the game
to try again.

Another way to do this style of DLL injection is to rename the injected dll
to something used by the game (e.g, "D3D9.dll"), export certain key functions
from the DLL, and make sure that modified DLL is on the load path, usually by
copying it into the game directory.  ModelMod does not currently support this
method, but it would be useful to add it, since certain games require it.
Specifically, any game that creates D3D9 from a secondary DLL, not the main
executable, cannot currently be patched by the hook code in ModelMod.dll.

## CLR Initialization

When the game creates a graphics device, ModelMod.dll will perform
"Lazy initialization".  The primary task here is to load a .Net CLR and
load the MMManaged assembly.

In order to control the CLR, ModelMod requires a custom app domain host.  
The managed code for this file is copied into the game directory
(it has a very unique name, so is unlikely to collide with any game files).
Note however that this does mean that the user running the game must have
write-access to the game folder.  The PrepCLR() function handles this copy.

Once this is complete, the Interop::InitCLR function will attempt to
initialize a CLR.  Upon success, the MMManaged dll will be loaded into the new CLR and its "Main" entry point will be called.

It has be observed that in some games, CLR initialization of the CLR 4.0
will fail with an E_FAIL ("unknown catastrophic failure") return code.  The
CLR 2.0 works, but is not compatible with F#, and so cannot be used.
It is currenty unknown whether this failure is due to a bug in the code, or some aspect of game configuration.  

The .Net CoreCLR includes a new COM
interface for starting the CLR (ICLRRuntimeHost2), and preliminary testing
shows that it is not affected by this issue.  It may be worth supporting
the CoreCLR as an alternate runtime once it reaches 1.0 and has full support
for F#.

## Object Selection
## Snapshot
## Mod file creation
## Blender import
## Blender export
## Mod loading
