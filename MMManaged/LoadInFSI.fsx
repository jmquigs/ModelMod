(*
This file can be used to test this assembly in F# Interactive.  
Create a file called "TestFSI.fsx" with these lines:

#load "LoadInFSI.fsx"
open ModelMod
... // your code here

You can then select all text in TestFSI.fsx (Ctrl-A) and hit Alt-enter to 
send it to FSI to run it.  The advantage of doing it this way is that 
TestFSI.fsx is ignored by source control, so you can put whatever you want
in there without the risk of it accidentally getting committed/conflicted.  

This file contains the basic structure of the project, so it is in 
version control - don't put test code here.
*)

#I @"..\packages\MonoGame.Framework.WindowsDX.3.3.0.0\lib\net40\"
//#I @"..\packages\YamlDotNet.3.5.1\lib\portable-net45+netcore45+wpa81+wp8+MonoAndroid1+MonoTouch1\"
#r @"SharpDX.dll"
#r @"SharpDX.Direct3D9.dll"
#r @"MonoGame.Framework.dll"
//#r @"YamlDotNet.dll"

#load "Logging.fs"
#load "Util.fs"
#load "CoreTypes.fs"

(*
To speed up iteration time, the rest of the project files are omitted - 
use additional #load lines for what you need in TestFSI.fsx.  You may need
to add more "open ModelMod" lines if you get errors there, because the modules
assume that everything in ModelMod is in scope.
*)
