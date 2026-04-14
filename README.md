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
* Windows or possibly Linux with Proton (have not tried)
* A supported D3D9 or DX11 game.  On windows 10 you may need to install the D3D9 runtime to capture textures
(https://www.microsoft.com/en-us/download/details.aspx?displayLang=en&id=35.)

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

# Installation

This project is intended for developers or technical users willing to get the source and build it.  I no longer provide release packages.
Only a few games have been tested.  Programming effort is usually required to get new games working.  [game compatibility list](https://github.com/jmquigs/ModelMod/wiki/Game-Compatibility-List)


License
-------

Unless otherwise noted here, ModelMod code is licensed under the terms of the
GNU LGPL version 2.1.

The included c++ format library (format.h/format.cc) is licensed under its
author's license - see format.h for details.

ModelMod references various third party .NET libraries which have their own
licenses.

