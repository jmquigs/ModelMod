ModelMod is a system for modifying art in games.  
It works by replacing 3D models (and textures, optionally) at the renderer level.

You start by selecting and snapshotting some model in the target game.
This snapshot can then be edited in a 3D modeling tool.  When the game runs
and draws the original model, ModelMod transparently swaps in your modified
version, and renders that instead.

Demo Videos
-----------
TBD

Requirements
------------

* Windows PC.
* Only a few games have been tested.  Programming effort is usually required
to get new games to work.
* ModelMod uses DLL injection to override the renderer in the target game;
the launcher app requires administrative privileges (via UAC escalation) to do this.
* The target game must use D3D9 for rendering.  Support for other versions is possible,
but has not been implemented.
* For animated models, the target game
must use GPU based animation.  Support for CPU animation, common in older games, is
known to be possible but is not currently implemented.
* Blender is the only 3D modeling tool supported at this time.  
If you want to write an exporter for something else, See Contributing (link).

Installation
------------


Non-programmers should install the binary version: link


Development
-----------

ModelMod is written in a combination of C++ and F#.  Most of the core code
is in F#, so hacking on it can be easier than you might think, assuming you
know or are willing to learn F#.

Install the following:

* June 2010 DirectX SDK: http://www.microsoft.com/en-us/download/details.aspx?id=6812
(I hope to eliminate this eventually, but right now it is required for some legacy d3dx code)
* Visual Studio 2013 or 2015 Community:
https://www.visualstudio.com/en-us/products/vs-2015-product-editions.aspx
* Visual F# power tools (optional but recommended) https://fsprojects.github.io/VisualFSharpPowerTools/
