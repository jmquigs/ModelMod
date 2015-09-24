module Main 

open System
open System.IO
open System.Windows

open ModelMod

// This module useful for running the managed code under a profiler.  To use it, change the project type
// to a "console application", then uncomment the entry point below.  You can then run it under 
// the vs2013 profiler.

let testPath = @"PathToDirectoryContainingModIndex.yaml"


//[<EntryPoint>]
let main argv = 
    if not (Directory.Exists testPath) then
        failwithf "dir does not exist: %s" testPath

    let mpath = Path.Combine(testPath, "ModIndex.yaml")
    let mdb = 
        ModDB.loadModDB
            ({ 
                MMView.Conf.ModIndexFile = Some(mpath)
                FilesToLoad = []
                AppSettings = None
            })
                
    0 