module TestModDBInterop

open FsUnit
open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod
open ModelMod.CoreTypes

[<Test>]
// I'm ambivalent about this test.  It would be better to rig up a native test framework and test it 
// from there, to exercise all the interop/marshalling gunk on both sides.
let ``ModDBInterop: module functions``() =
    RegConfig.initForTest()
    RegConfig.setGlobalValue RegKeys.DocRoot Util.TestDataDir |> ignore

    // have to trick SetPaths because we're running without modelmod.dll
    let fakeRoot = Path.Combine(Util.TestDataDir, "dummymodelmod.dll")
    ModDBInterop.setPaths fakeRoot "" |> ignore
    let datapath = State.getBaseDataDir()
    Assert.IsTrue (datapath <> null, "null data path")
    Assert.AreEqual (Path.GetFullPath(datapath), Path.GetFullPath(Util.TestDataDir), "incorrect data path")

    let () = 
        let ret = ModDBInterop.loadFromDataPath()
        Assert.AreEqual (ret, 0, "load failure")

    let mcount = ModDBInterop.getModCount()
    Assert.AreEqual (mcount, 3 , "incorrect mod count")

    [0..2] |> List.iter (fun modidx -> 
        let mmod = ModDBInterop.getModData(modidx)
        printfn "Mod %A: type %A, pc %A, vc %A" modidx mmod.ModType mmod.PrimCount mmod.VertCount
    )

    let () = 
        let mmod = ModDBInterop.getModData(0) // monolith
        
        Assert.AreEqual (mmod.ModType, (ModDBInterop.modTypeToInt GPUReplacement) , sprintf "incorrect mod type: %A" mmod)
        Assert.AreEqual (mmod.PrimType, 4 , sprintf "incorrect prim type: %A" mmod)
        Assert.AreEqual (mmod.PrimCount, 36 , sprintf "incorrect prim count: %A" mmod)
        Assert.AreEqual (mmod.VertCount, 24 , sprintf "incorrect vert count: %A" mmod)
        Assert.AreEqual (mmod.RefPrimCount, 12 , sprintf "incorrect ref prim count: %A" mmod)
        Assert.AreEqual (mmod.RefVertCount, 8 , sprintf "incorrect ref vert count: %A" mmod)
        Assert.AreEqual (mmod.IndexCount, 0 , sprintf "incorrect index count: %A" mmod)
        Assert.AreEqual (mmod.IndexElemSizeBytes, 0 , sprintf "incorrect index size: %A" mmod)
        Assert.AreEqual (mmod.DeclSizeBytes, 72 , sprintf "incorrect decl size: %A" mmod)
        Assert.AreEqual (mmod.VertSizeBytes, 92 , sprintf "incorrect vert size: %A" mmod)
        Assert.AreEqual (mmod.Tex0Path, "" , sprintf "incorrect tex0 path: %A" mmod)
        Assert.AreEqual (mmod.Tex1Path, "" , sprintf "incorrect tex1 path: %A" mmod)
        Assert.AreEqual (mmod.Tex2Path, "" , sprintf "incorrect tex2 path: %A" mmod)
        Assert.AreEqual (mmod.Tex3Path, "" , sprintf "incorrect tex3 path: %A" mmod)

    let checkDelMod index pCount vCount = 
        let mmod = ModDBInterop.getModData(index) 
        Assert.AreEqual (mmod.ModType, (ModDBInterop.modTypeToInt Deletion) , sprintf "incorrect mod type: %A" mmod)
        Assert.AreEqual (mmod.PrimType, 4 , sprintf "incorrect prim type: %A" mmod)
        Assert.AreEqual (mmod.PrimCount, pCount, sprintf "incorrect prim count, want %A, got %A" pCount mmod.PrimCount)
        Assert.AreEqual (mmod.VertCount, vCount, sprintf "incorrect vert count: want %A, got %A" vCount mmod.VertCount)
        Assert.AreEqual (mmod.RefPrimCount, pCount , sprintf "incorrect ref prim count: %A" mmod)
        Assert.AreEqual (mmod.RefVertCount, vCount , sprintf "incorrect ref vert count: %A" mmod)
        Assert.AreEqual (mmod.IndexCount, 0 , sprintf "incorrect index count: %A" mmod)
        Assert.AreEqual (mmod.IndexElemSizeBytes, 0 , sprintf "incorrect index size: %A" mmod)
        Assert.AreEqual (mmod.DeclSizeBytes, 0 , sprintf "incorrect decl size: %A" mmod)
        Assert.AreEqual (mmod.VertSizeBytes, 0 , sprintf "incorrect vert size: %A" mmod)
        Assert.AreEqual (mmod.Tex0Path, "" , sprintf "incorrect tex0 path: %A" mmod)
        Assert.AreEqual (mmod.Tex1Path, "" , sprintf "incorrect tex1 path: %A" mmod)
        Assert.AreEqual (mmod.Tex2Path, "" , sprintf "incorrect tex2 path: %A" mmod)
        Assert.AreEqual (mmod.Tex3Path, "" , sprintf "incorrect tex3 path: %A" mmod)

    // del mods
    checkDelMod 1 100 200
    checkDelMod 2 150 300

    // out of range mod
    let () = 
        let mmod = ModDBInterop.getModData(100)
        Assert.AreEqual (mmod, InteropTypes.EmptyModData , "expected empty mod")

    ()