![ModelMod github action](https://github.com/jmquigs/ModelMod/actions/workflows/dotnet-desktop.yml/badge.svg)

# Intro

ModelMod is a system for modifying art in games.
It works by replacing 3D models (and textures, optionally) at the renderer level.

You start by selecting and snapshotting a model in the game.
This snapshot can then be edited in a 3D modeling tool
and re-exported.  Then you load it back into the game, where it will be automatically
rendered in place of the original.

ModelMod is intended for small-scale replacements, such as character armor/body types, and deletion of unwanted meshes.  It has some limited ability to replace textures, but normally does not.  It can't replace shaders at this time, 
though it's possible that a program that does that (such as reshade) could chain load ModelMod.

# Installation

**Warning: Using ModelMod may violate the terms of service of a game.  The developer might ban your account.  Use at your own risk.**
  * I'm not aware of anyone that has been banned for ModelMod, and my
  accounts have not been banned.  However there is always risk so you should be careful.

This project is intended for developers or technical users willing to get the source and build it.  I no longer provide release packages.
Only a few games have been tested.  Programming effort is usually required to get new games working.  A small sample of games that once worked is here: [game compatibility list](https://github.com/jmquigs/ModelMod/wiki/Game-Compatibility-List)

If you want to try it with a particular game, first check the requirements below to see if the game looks compatible.  If so, I recommend you get a subscription to an agentic coding service like OpenAI Codex or (preferably) Claude Code, and have it
make all the changes needed to get your game working.  

The web version of Claude Code can successfully work on ModelMod - you don't need to set up a local agentic sandbox.  You will still need to build its changes locally to run them on your system, or use the github action in your fork to build executables for you.

That agent can also likely convert the Blender scripts for mmobj into versions that work with newer Blender or whatever tool you prefer.


# Requirements

* Windows or Linux with Proton
* A D3D9 or DX11 game.  
* Blender 2.79b is the only 3D tool supported.  Later versions changed the python API and this has broken
ModelMod's custom importer/exporter.
  * Download 2.79b here.  https://download.blender.org/release/Blender2.79/
* .Net Framework 4.6.2 or newer - this may already be installed on your machine, if not here is a direct link: https://dotnet.microsoft.com/en-us/download/dotnet-framework/net48.  This is quite an old runtime that may not be installed by default on newer Windows.  On Linux, it is not needed as wine's mono is sufficient.
* The Docs in `Docs` are woefully out of date, check their contents with an agentic coding tool before relying on anything in there.  `DEVNOTES.md` is relatively up to date though.
* ModelMod makes use of a `TPLib` directory to load various support libraries as described hereafter.  This is a subdirectory of ModelMod root which you manually create and populate (once).  I cannot provide a premade `TPLib` with all these files due to redistribution restrictions from Microsoft.  If you are unsure how to set this up, ask an agentic coding tool for details.
* Some games require regeneration of tangent-space vectors (binormal/bitangent, tangent, normal).  The repo https://github.com/jmquigs/DirectXMesh/tree/changes-for-mm contains a fork of DirectXMesh that ModelMod can use to do this.  This should be built separately and dropped into `TPLib`.
  * Within `TPLib` the file should be named: `DirectXMesh_x64.dll, DirectXMesh_x86.dll`, for the 64 and 32 bit versions respectively (if your game is only 64 bit you don't need the 32 bit version.)
  * Visual Studio Community C++ can build these files - if you don't have it, consider forking my `DirectXMesh` repo and getting a LLM agent to set up a github action to build them.
* Texture capture:
  * On Windows 10 or newer, you may need to install the D3D9 runtime to capture textures
(https://www.microsoft.com/en-us/download/details.aspx?displayLang=en&id=35.)
    * This will create the files in the Windows system32 directory but you must copy them to `TPLib` as described below for MM to use them.
  * On Linux you should use the Windows d3dx library to capture most textures; the proton/wine version is missing support for some formats.  
  * On both Linux and Windows the d3dx libs must be copied into `TPLib` even if installed in the system (mostly for historic reasons).  Within `TPLib` the files should be named like so: `D3DX9_43_x86_64.dll, D3DX9_43_x86.dll, d3dx11_43_x86_64.dll, d3dx11_43_x86.dll`
  * DX10+ textures must be converted to a DX9-compatible format for Blender 2.79b to be able to use them.  `texconv.exe` and `texdiag.exe` from the DirectXTex library can be used to do this.  The MMLaunch tool's create mod utility will automatically use these if they are available in `TPLib`.  The prebuilt versions of those executables from the DirectXTex repository will likely work.
  `texdiag.exe` is optional, MM doesn't use it, but you can use it to inspect a texture to see what its format is.

* For animated models, the target game
must use GPU based animation.  Essentially this means that when a snapshot is done, the mesh-to-be-modded captures in a reference pose in object space, and then the skinning (animation) is done in the vertex shader.
CPU animation is when the capture is in some random frame of animation (and probably also transformed into world space).  The latter is common in very old games (mostly DX9, but even many of the late-era DX9 games use GPU animation).  Support for CPU animation is known to be possible but requires a completely different technique that is not currently implemented.

<!--
[comment]: [![appveyor](https://ci.appveyor.com/api/projects/status/gqsf2f001h46q1tn?svg=true)](https://ci.appveyor.com/project/jmquigs/modelmod)
-->


License
-------

Unless otherwise noted here, ModelMod code is licensed under the terms of the
GNU LGPL version 2.1.


ModelMod references various third party .NET libraries which have their own
licenses.

