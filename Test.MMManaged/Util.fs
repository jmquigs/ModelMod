module Util

open System
open System.Reflection
open System.IO

open ModelMod.CoreTypes

let veqEqEpsilon (ep:float32) (v1:Vec3F) (v2:Vec3F) =
    let dx = Math.Abs(v1.X - v2.X) 
    let dy = Math.Abs(v1.Y - v2.Y)
    let dz = Math.Abs(v1.Z - v2.Z)
    dx < ep && dy < ep && dz < ep

let TestDataDir =
    // walk up from the assembly dir looking for TestData; the output layout differs between the
    // VS and dotnet builds.  avoid literal path separators, they aren't portable.
    let asmDir = Path.GetDirectoryName(Uri(Assembly.GetExecutingAssembly().CodeBase).LocalPath)

    let rec search (dir:DirectoryInfo) levels =
        if isNull (box dir) || levels = 0 then
            None
        else
            let cand = Path.Combine(dir.FullName, "TestData")
            if Directory.Exists cand then Some(cand) else search dir.Parent (levels-1)

    match search (DirectoryInfo(asmDir)) 6 with
    | None -> failwithf "Failed to locate test data directory at or above: %s" asmDir
    | Some path -> path
