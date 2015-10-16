# The Dev Guide

## Development environment setup

Install the following:

* June 2010 DirectX SDK: http://www.microsoft.com/en-us/download/details.aspx?id=6812
(I hope to eliminate this eventually, but right now it is required for some legacy d3dx code)
* Visual Studio 2015 Community (or any 2015 variant):
https://www.visualstudio.com/en-us/products/vs-2015-product-editions.aspx
* Visual F# power tools (optional but recommended) https://fsprojects.github.io/VisualFSharpPowerTools/
(can be installed via Visual Studio Extensions menu)

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

## Injection
## Object Selection
## Snapshot
## Mod file creation
## Blender import
## Blender export
## Mod loading
