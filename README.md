![example workflow](https://github.com/jmquigs/ModelMod/actions/workflows/dotnet-desktop.yml/badge.svg)

# Intro

ModelMod is a system for modifying art in games.
It works by replacing 3D models (and textures, optionally) at the renderer level.

You start by selecting and snapshotting a model in the game.
This snapshot can then be edited in a 3D modeling tool
and re-exported.  Then you load it back into the game, where it will be automatically
rendered in place of the original.

# Requirements

* **Warning: Using ModelMod may violate the terms of service of a game.  The
developer might ban your account.  Use at your own risk.**
  * I'm not aware of anyone that has been banned for ModelMod, and my
  accounts have not been banned.  However there is always risk so
  you should be careful.
* Windows.
* A supported D3D9 game.  On windows 10 you may need to install the D3D9 runtime to capture textures
(https://www.microsoft.com/en-us/download/details.aspx?displayLang=en&id=35.)
  * Only a few games have been tested.  Programming effort is usually required.  [game compatibility list](https://github.com/jmquigs/ModelMod/wiki/Game-Compatibility-List)

* Blender 2.79b is the only 3D tool supported.  Later versions changed the python API and this has broken
ModelMod's custom importer/exporter.
  * Download 2.79b here.  https://download.blender.org/release/Blender2.79/
* .Net Runtime 4.5 or newer (this may already be installed on your machine): https://www.microsoft.com/en-us/download/details.aspx?id=30653.  Developers can skip this.
* For animated models, the target game
must use GPU based animation.  Support for CPU animation, common in older games, is
known to be possible but is not currently implemented.

<!--
[comment]: [![appveyor](https://ci.appveyor.com/api/projects/status/gqsf2f001h46q1tn?svg=true)](https://ci.appveyor.com/project/jmquigs/modelmod)
-->


Installation
------------

Non-programmers should use the [release package](https://github.com/jmquigs/ModelMod/releases).

I used to have tutorial docs and videos but they were deleted.  You can
check out the [User's Guide](Docs/userguide/README.md).

Development
-----------

For install & build instructions, [check out the Dev Guide](Docs/devguide/README.md).

### Experimental port

This is an experimental port of the C++ portion of modelmod to Rust.
A large portion of this has already been completed but
some functionality is still missing.  The old C++ code is no longer present in this branch but
can be found in the "1.0.0.13-pre-rust-merge" tag.

Notably, the front end UI (MMLaunch) does not yet
understand how to launch games since the injection method has changed.

The new injection method uses the established "proxy d3d9.dll" method, where the MM output
dll is renamed to that and copied into the game's directory prior to launch.  Until the launcher
is updated, these steps must be done manually, though there are some helper shell scripts in
ModelMod/Native that can be run under bash (including in git for windows).

This repository also contains some experimental new capturing systems such as shader constants
and shaders.  These are intended to capture larger portions of scenes, though there is no modding
capability for them at this time.


### Note for Rust language safety police

This isn't a traditional Rust project in that there is a lot of unsafe code.  The thread model of the game is unknown,
so the Rust compiler cannot do all the lifetime and ownership checking that you might normally
expect.  There are also a lot of raw pointers because of all the use
of low level windows APIs.

Think of this project as being written in the
subset of Rust known as "Unsafe Rust".

If you don't like `unsafe` in Rust code, I recommend you close this browser tab now and forget you ever found this.

That being said, I _am_ very interested in fixing any Undefined Behavior (UB) that might exist in
this code, and I welcome comments/PRs from people interested in the code from this perspective.

### Contributing

Pull Requests are welcome!  In particular, these things are desirable:

* Support for new games, or improved support for existing games

* Improvements to the DLL injection and related initialization code

* Support for recent versions of D3D, especially 11/12.

* Support for other 3D tools (port MMObj format code, new Snapshot profiles)

#### DMCA & Copy Protection

Due to the [Digital Millennium Copyright Act](https://en.wikipedia.org/wiki/Digital_Millennium_Copyright_Act), I cannot accept code contributions that circumvent copy-protection systems built into any game.

License
-------

Unless otherwise noted here, ModelMod code is licensed under the terms of the
GNU LGPL version 2.1.

The included c++ format library (format.h/format.cc) is licensed under its
author's license - see format.h for details.

ModelMod references various third party .NET libraries which have their own
licenses.

The following components of ModelMod are in the Public Domain.  These programs are distributed WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

* The MMLoader injection utility (files in "MMLoader" subdirectory)
* All unit/integration tests (files in the "Test.MMManaged" subdirectory)
