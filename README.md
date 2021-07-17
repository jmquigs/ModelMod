# Intro

ModelMod is a system for modifying art in games.  
It works by replacing 3D models (and textures, optionally) at the renderer level.

You start by selecting and snapshotting a model in the game.
This snapshot can then be edited in a 3D modeling tool and re-exported.  Then you load it back into the game, where it will be automatically
rendered in place of the original.


[comment]: [![appveyor](https://ci.appveyor.com/api/projects/status/gqsf2f001h46q1tn?svg=true)](https://ci.appveyor.com/project/jmquigs/modelmod)

# DEVELOP BRANCH

This is an experimental port of the C++ portion of modelmod to Rust.  The intention is to delete 
all the C++ code when the port is done.  A large portion of this has already been completed but 
some functionality is still missing.  Notably, the front end UI (MMLaunch) does not yet 
understand how to launch games since the injection method has changed.

The new injection method uses the established "proxy d3d9.dll" method, where the MM output 
dll is renamed to that and copied into the game's directory prior to launch.  Until the launcher 
is updated, these steps must be done manually, though there are some helper shell scripts in 
ModelMod/Native that can be run under bash (including in git for windows).

This repository also contains some experimental new capturing systems such as shader constants 
and shaders.  These are intended to capture larger portions of scenes, though there is no modding
capability for them at this time.

For legal reasons, due to the state of (horrible) related laws in the US, I do not explicitly 
name the games that work with ModelMod.  Its a pretty small number of games, though.  Use it at your own risk 
and with the full knowledge that you may be banned by the game's developer, 
if that is possible and they discover your usage and are mad about it.  
There also are no official releases of this branch at this time.

## Note for Rust language safety police

This isn't a traditional Rust project in that there is a lot of unsafe code.  It must be this way
because the thread model of the game is unknown (and unknowable, since it changes with each game),
so the Rust compiler cannot do all the lifetime and ownership checking that you might normally 
expect.  Think of this project as being written in the 
subset of Rust known as "Unsafe Rust".

Since there has been some drama in the Rust community when the subfaction of Safety Zealots 
discover a Rust project has "too much" unsafe code, I'm writing this a helpful signpost to suggest 
those people just buzz off.  If your eyes bleed and you whip out a holy symbol every time you 
see `unsafe` in Rust code, I recommend you close this browser tab now and forget you ever found this.

That being said, I _am_ very interested in fixing any Undefined Behavior (UB) that might exist in 
this code, and I welcome comments/PRs from people interested in the code from this perspective.

## Old Readme follows (somewhat out of date)

Requirements
------------

* Windows.  
* 32-bit only games at the moment.
* The game must use D3D9 for rendering.  Make sure you have the D3D9 runtime installed.  This is an especially true for Windows 10 systems.  Developers can skip this since you need the DX SDK instead.  https://www.microsoft.com/en-us/download/details.aspx?displayLang=en&id=35.  
* .Net Runtime 4.5 or newer (this may already be installed on your machine): https://www.microsoft.com/en-us/download/details.aspx?id=30653.  Developers can skip this.
* Only a few games have been tested.  Programming effort is usually required
to get new games to work.
* For animated models, the target game
must use GPU based animation.  Support for CPU animation, common in older games, is
known to be possible but is not currently implemented.
* Blender is the only 3D modeling tool supported at this time.  
If you want to write an exporter for something else, See [Contributing](#Contributing).

Installation
------------

Non-programmers should use the [release package](https://github.com/jmquigs/ModelMod/releases).

Also, check out the [game compatibility list](https://github.com/jmquigs/ModelMod/wiki/Game-Compatibility-List), and [look at the User's Guide](Docs/userguide/README.md).

Development
-----------

ModelMod is written in a combination of C++ and [F#](http://fsharp.org/).  Most of the core code
is in F#, so hacking on it can be easier than you might think, assuming you
know or are willing to learn F#.

For install & build instructions, [check out the Dev Guide](Docs/devguide/README.md).

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
