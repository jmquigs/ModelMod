module Main 

open System
open System.IO
open System.Windows

open ModelMod

// This module is useful for running the managed code under a profiler or debugger.  
// To use it, change the project type
// to a "console application", then uncomment the entry point below.  You can then run it under 
// the visual studio profiler or standlone.

let targetDataDir = @"\E2ETestData"
let searchPaths = ["."; ".."; @"..\.."; ] 
let testPath = 
    searchPaths
    |> List.tryPick
        (fun p -> 
            let p = Path.GetFullPath(p) + targetDataDir
            if Directory.Exists(p) then Some(p) else None)

let load() =
    let testPath = 
        match testPath with
        | None -> failwithf "can't find %A in any of these paths: %A" targetDataDir searchPaths
        | Some(p) -> p
    if not (Directory.Exists testPath) then
        failwithf "dir does not exist: %s" testPath

    let mpath = Path.Combine(testPath, "ModIndex.yaml")
    let mdb = 
        ModDB.loadModDB
            ({ 
                StartConf.Conf.ModIndexFile = Some(mpath)
                FilesToLoad = []
                AppSettings = None
            })
    mdb

let timeLoads(allowCache) = 
    let eTimes = Array.zeroCreate 10
    [0..(eTimes.Length - 1)] |> List.iter
        (fun i -> 
            if not allowCache then MemoryCache.clear()
            let sw = new Util.StopwatchTracker("foo")
            sw.SW.Start()
            load() |> ignore
            let e = sw.SW.ElapsedMilliseconds
            eTimes.[i] <- e
        )
    let eTimes = eTimes |> Array.map double |> Array.sort
    let mean = eTimes |> Array.average
    let median = eTimes |> fun a -> (a.[5] + a.[4]) / 2.0
    let p90 = eTimes.[int (float eTimes.Length * 0.9)]
    let stddev = 
        eTimes |> Array.map (fun t -> (t - mean) ** 2.0) |> Array.average |> Math.Sqrt
        
    printf "load times (caching: %A): mean: %f, median: %f, 90%%: %f, stddev: %f " allowCache mean median p90 stddev

let testFill() =
    let mdb = load() 

    let destDecl:byte [] = Array.zeroCreate (64)
    let destVB:byte[] = Array.zeroCreate (833760)
    let destIB:byte[] = [||]

    State.Data.Moddb <- mdb
    ModDBInterop.testFill(0, destDecl, destVB, destIB) |> ignore

//[<EntryPoint>]
let main argv = 
    //load()
    //timeLoads(true)
    0 