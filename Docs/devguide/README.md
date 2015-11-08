# Dev Guide

## Demo video

This video is a live coding exercise showing how to make some (admittedly simple) changes to ModelMod to support a new game.  

(link)

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
  * ModelMod.sln: contains native library and injection tool.  


  If you are not
  modifying the interop layer, you may not need to build ModelMod.sln, and can probably
  just use the binaries (MMLoader.exe,ModelMod.dll) from a release package.
  Build the MMDotNet.sln first, then copy the native binaries into the
  "Release" directory.  Be certain that your binary distribution is
  compatible with the source code you have checked out, otherwise the game
  may crash in the interop layer.  It is recommended that you build the
  native code if you are able, even if you don't intend to modify it.

  Generally you should build both projects in "Release" configuration.  
  The two projects must be set to the same configuration.  Debug is mostly
  useful for hunting memory problems in C++ code, or attaching the debugger
  to the native or managed code.

### Test cases

There are test cases for the core Managed code.  These have some fairly decent coverage of the core, but don't delve into
game compatibility much.  Over time I want to make these tests more
authoritative in that area, to avoid the need of doing a big regression test across M people and N games each time there are major changes.

There are no test cases for the UI, or the native code.  Both could use them.

To run the tests, install the Nunit 2 test runner for visual studio from the
Extensions menu.  You should then be able to run with Test->Run->All Tests.
Use the "Test Explorer" window to view status.

## Technical Overview

This section describes some of the technical details of ModelMod for
programmers and advanced users.  If you haven't already done so, its a
good idea to read the User's Guide first.

You can think of ModelMod as a "sed" or "find/replace" program for artwork.
The ModelMod renderer watches all the geometry that is being drawn by the game;
when a particular draw call triggers a match with a loaded mod, the system
substitutes the mod for the original geometry.  

The program is essentially an alternate asset load pipeline.  Other than
input processing, it does not do any per-frame operations.  Some future
mod types may require that, however, specifically support for software-animated
meshes (common in older games.)

Currently a match is anything
that matches the exact primitive and vertex count of the reference used by the mod; this can lead
to some false-positives, however, for things like character meshes,
vertex & primitive count can be fairly unique.  However, this does have some consequences:
* Anything that matches the vert/prim count will be replaced.  Therefore
all instances of that thing in the game (if drawn multiple times) will be
replaced, and there is no way to control this right now.
* It is not possible to modify a single object that is some basic primitive,
like a cube.  You wouldn't be able to avoid changing all the cubes in the game.
* Objects that have very regular mesh patterns (such as particle emitters)
are difficult to mod, because the basic mesh is reused for many kinds of
particle effects.

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

## Renderer & input hook

One injected, the ModelMod.dll will look for certain imported functions in the PE executable table and patch them.  It will patch any
D3D9 related calls so that it can inject the modelmod hook device.  
It will also create and initialize DirectInput so that modelmod can
receive input commands.

While modelmod tracks certain kinds of renderer state changes as they happen, most processing occurs when the game calls beginScene() on the device, which should happen once per frame at least.

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

In some games, CLR initialization of the CLR 4.0
will fail with an E_FAIL ("unknown catastrophic failure") return code.  The
CLR 2.0 works, but is not compatible with F#, and so cannot be used.
It is currenty unknown whether this failure is due to a bug in the code, or some aspect of game configuration.  

The .Net CoreCLR includes a new COM
interface for starting the CLR (ICLRRuntimeHost2), and preliminary testing
shows that it is not affected by this issue.  It may be worth supporting
the CoreCLR as an alternate runtime once it reaches 1.0 and has full support
for F#.

## Object Selection

To facilitate object selection, modelmod maintains a list of recently used textures in the hook renderer, and allows the user to page through them.
Whenever the game renders something with the selected texture, modelmod
substitutes a green texture so that the art is (usually) highlighted
on the display.  

Although selection is done by texture, actual mod snapshotting (and replacement) is done by _geometry_; that is, ModelMod does not support replacing textures based on a checksum or other texture-specific information.

## Snapshot

Snapshotting is initiated by native code when the snapshot key is pressed,
but the core of the logic is in managed code.  One benefit of this approach
is that it is possible to interactively work on the snapshot code without
restarting the game.

The snapshot function (Snapshot.take) is passed the device and some other
snapshot data created by native code.  Certain buffers need to be created
from native code because the managed device interface does not have them.

The snapshot function is very careful about managing COM reference counts for any resources it requires; given the interop layer, this is tricky
business.  Be careful when adding code to the snapshot module that acquires new D3D resources from the device.  It is easy to introduce leaks or crashes.

Snapshot will apply a series of transforms to the model data prior to saving it to disk.  The intention here is to get the geometry into a reasonable
format for editing in a 3D tool.  Unless otherwise specified, all extant
profiles assume Blender as the 3D tool; additional profiles will be needed
for other tools.  Currently this requires editing the F# code; ultimately
it would be nice to have them be specified in a data file so that
non-programmers can edit them.

One case that the current snapshot system doesn't handle is game-specific profiles.  For instance, if a game uses 4 byte normals, the snapshot system assumes that they are in a certain format; if different games use the
same 4 byte format but different semantics (for instance, different
XYZW ordering), snapshot cannot handle it.  This issue has not yet
been observed in practice, but if it occurs, a different game-specific profile system will need to be implemented to capture these semantic variations.

#### Output format

Snapshot produces various types of files.  

".dat" files are binary files
that are intended to be loaded directly by modelmod later; the vertex
declaration format is a prime example of this.  These files are not
intended to be modded.

".dds" files are snapshotted textures.  ModelMod can only read
texture data if the texture data is stored in an accessible memory location
in the device; some games do not do this by default.  For these games, it
may be worth implementing an option that forces all textures to be created
in managed memory so that they can be accssed during snapshot.

".mmobj" files contain geometry.  These are basically .obj files, but
contain additional ModelMod specific data that is required to set up
animation properly.  The additional data is written in comment lines
(e.g: "#vbld ..."), so a regular .obj import can still import the file.  However an exported .obj file will not work.  The use of this nonstandard
format requires an import/exporter pair for each supported 3D tool.

#### Shaders

ModelMod currently does not capture shaders or shader-related data; it
would be nice to support this at some point so that custom shaders can be used.  This may present some technical challenges since only the shader
disassembly can be captured (the original HLSL code is lost), and modding
that is not exactly simple.  The shader author may be able to simply drop
in a new shader based on new HLSL code, but care must be taken to make
sure the shade uses the same inputs, and produces the same output,
as the original shader, which again requires inspecting the disassembly.

Similarly, you cannot simply snapshot a set of shader constants and then
inject them back in for mod playback; the original constants will contain
data specific to the render state at time of snapshot
(such as world/projection matrices and bone animation data), that data
will cause the mod to display incorrectly if reused.  Only a set
of "whitelisted" constants from the original snapshot can be used or modded;
the rest must come from the game's render state.

## Mod file creation

The "raw" files that are written to the snapshot directory
need some post-processing is needed to get them in to a usable format and
give them a name.  

The launcher tool has a Create Mod command that automates this process.  
The tool moves and renames the mod files, and produces the ".yaml"
metadata files that ModelMod will use later to load the raw data.  It can
also add the ModIndex.yaml file. see "Mod Loading" below for more information about these files.

This tool works well enough for most use cases, but notably
it only produces one type of mod (GPUReplacement)
If you want a different type you must hand-edit the output yaml files.

## Blender import & export

The MMOBJ blender import/exporter is simply a modification of the existing .obj importer.  These scripts are copies of the originals, so the MMObj
code is not dependant on a particular version of the .obj scripts in the
same blender install.  It may, however, be affected by changes in the
blender python API.

At some point the mmobj format will be documented so that one can write
an importer/exporter from the spec.  For the moment, writing a new
importer/exporter requires spelunking in the blender scripts and/or
github history to see what needs to be done.

## Mod loading

ModelMod maintains a game "data" directory each game.  Each mod usualy is located in its own directory within the data directory.  The ModIndex.yaml file controls which mods are actually loaded by the game; each game has
its own ModIndex file located in that game's directory.

Each mod consists of both "Mod" and "Ref" files.  Reference files contain data that is not usually changed as part of the modding process, but must
be present in order to load and display the mod correctly.  

# Miscellanea

### This can't be the best way to make art mods.

Right, it isn't.  If a game supplies a built-in asset pipeline, that is
almost certainly superior to what ModelMod lets you do.  ModelMod is good for cases where such a pipeline doesn't exist, is too hard to use or requires expensive tools.

### Why F#?

You may wonder why this project use a split-language
implementation (C++/F#), rather than all C++.  

TL;DR: it mostly reflects the author's preferences.

ModelMod was
entirely c++ in the beginning.  At some point it was badly in need of refactor, so I did a spike to investigate whether F#/CLR integration was
feasible (including the overhead of the requisite "interop hell").  This
worked out pretty well, so I decide to rewrite the whatever I could in F#.

Despite the overhead of interop, this has turned out to be a good decision.  
The F# code is much easier to modify than the original C++; its safer since its not as easy to trash process memory; and its easier to get the code working.  Its also a lot more fun to code in, and hot reload is a plus.  
There more reasons, but suffice to say that without F#, this project would be a half-broken and forgotten unfinished mess sitting in some directory on the author's hard drive.
