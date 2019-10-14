# NOTE
This project is unmaintained.  It may still work with old games that are not being updated.  It is unlikely to work with newer games or recently updated games.

# Intro

**ModelMod** is a system for modifying art in games.  
It works by replacing 3D models (and textures, optionally) at the renderer level.

You start by selecting and snapshotting a model in the game.
This snapshot can then be edited in a 3D modeling tool and re-exported.  Then you load it back into the game, where it will be automatically
rendered in place of the original.

[![appveyor](https://ci.appveyor.com/api/projects/status/gqsf2f001h46q1tn?svg=true)](https://ci.appveyor.com/project/jmquigs/modelmod)


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
