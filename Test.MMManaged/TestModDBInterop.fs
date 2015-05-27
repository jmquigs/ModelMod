module TestModDBInterop

open FsUnit
open FsCheck
open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod
open ModelMod.CoreTypes

let check = Check.QuickThrowOnFailure

[<Test>]
// I'm ambivalent about this test.  It would be better to rig up a native test framework and test it from there, to exercise all the 
// interop/marshalling gunk on both sides.
let ``ModDBInterop: module functions``() =
    // have to trick SetPaths because we're running without modelmod.dll
    let fakeRoot = Path.Combine(Util.TestDataDir, "dummymodelmod.dll")
    ModDBInterop.SetPaths fakeRoot "" |> ignore
    let datapath = ModDBInterop.GetDataPath()
    check (datapath <> null |@ "null data path")
    check (Path.GetFullPath(datapath) = Path.GetFullPath(Util.TestDataDir) |@ "incorrect data path")

    let () = 
        let ret = ModDBInterop.LoadFromDataPath()
        check (ret = 0 |@ "load failure")

    let mcount = ModDBInterop.GetModCount()
    check (mcount = 3 |@ "incorrect mod count")

    let () = 
        let mmod = ModDBInterop.GetModData(0) // monolith
        check (mmod.modType = (ModDBInterop.ModTypeToInt GPUReplacement) |@ sprintf "incorrect mod type: %A" mmod)
        check (mmod.primType = 4 |@ sprintf "incorrect prim type: %A" mmod)
        check (mmod.primCount = 36 |@ sprintf "incorrect prim count: %A" mmod)
        check (mmod.vertCount = 24 |@ sprintf "incorrect vert count: %A" mmod)
        check (mmod.refPrimCount = 12 |@ sprintf "incorrect ref prim count: %A" mmod)
        check (mmod.refVertCount = 8 |@ sprintf "incorrect ref vert count: %A" mmod)
        check (mmod.indexCount = 0 |@ sprintf "incorrect index count: %A" mmod)
        check (mmod.indexElemSizeBytes = 0 |@ sprintf "incorrect index size: %A" mmod)
        check (mmod.declSizeBytes = 72 |@ sprintf "incorrect decl size: %A" mmod)
        check (mmod.vertSizeBytes = 92 |@ sprintf "incorrect vert size: %A" mmod)
        check (mmod.tex0Path = "" |@ sprintf "incorrect tex0 path: %A" mmod)
        check (mmod.tex1Path = "" |@ sprintf "incorrect tex1 path: %A" mmod)
        check (mmod.tex2Path = "" |@ sprintf "incorrect tex2 path: %A" mmod)
        check (mmod.tex3Path = "" |@ sprintf "incorrect tex3 path: %A" mmod)

    let checkDelMod index pCount vCount = 
        let mmod = ModDBInterop.GetModData(index) 
        check (mmod.modType = (ModDBInterop.ModTypeToInt Deletion) |@ sprintf "incorrect mod type: %A" mmod)
        check (mmod.primType = 4 |@ sprintf "incorrect prim type: %A" mmod)
        check (mmod.primCount = 0 |@ sprintf "incorrect prim count: %A" mmod)
        check (mmod.vertCount = 0 |@ sprintf "incorrect vert count: %A" mmod)
        check (mmod.refPrimCount = pCount |@ sprintf "incorrect ref prim count: %A" mmod)
        check (mmod.refVertCount = vCount |@ sprintf "incorrect ref vert count: %A" mmod)
        check (mmod.indexCount = 0 |@ sprintf "incorrect index count: %A" mmod)
        check (mmod.indexElemSizeBytes = 0 |@ sprintf "incorrect index size: %A" mmod)
        check (mmod.declSizeBytes = 0 |@ sprintf "incorrect decl size: %A" mmod)
        check (mmod.vertSizeBytes = 0 |@ sprintf "incorrect vert size: %A" mmod)
        check (mmod.tex0Path = "" |@ sprintf "incorrect tex0 path: %A" mmod)
        check (mmod.tex1Path = "" |@ sprintf "incorrect tex1 path: %A" mmod)
        check (mmod.tex2Path = "" |@ sprintf "incorrect tex2 path: %A" mmod)
        check (mmod.tex3Path = "" |@ sprintf "incorrect tex3 path: %A" mmod)

    // del mods
    checkDelMod 1 100 200
    checkDelMod 2 150 300

    // out of range mod
    let () = 
        let mmod = ModDBInterop.GetModData(100)
        check (mmod = InteropTypes.EmptyModData |@ "expected empty mod")

    ()