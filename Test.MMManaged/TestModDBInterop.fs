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

    let () = 
        let mmod = ModDBInterop.getModData(0) // monolith
        Assert.AreEqual (mmod.modType, (ModDBInterop.modTypeToInt GPUReplacement) , sprintf "incorrect mod type: %A" mmod)
        Assert.AreEqual (mmod.primType, 4 , sprintf "incorrect prim type: %A" mmod)
        Assert.AreEqual (mmod.primCount, 36 , sprintf "incorrect prim count: %A" mmod)
        Assert.AreEqual (mmod.vertCount, 24 , sprintf "incorrect vert count: %A" mmod)
        Assert.AreEqual (mmod.refPrimCount, 12 , sprintf "incorrect ref prim count: %A" mmod)
        Assert.AreEqual (mmod.refVertCount, 8 , sprintf "incorrect ref vert count: %A" mmod)
        Assert.AreEqual (mmod.indexCount, 0 , sprintf "incorrect index count: %A" mmod)
        Assert.AreEqual (mmod.indexElemSizeBytes, 0 , sprintf "incorrect index size: %A" mmod)
        Assert.AreEqual (mmod.declSizeBytes, 72 , sprintf "incorrect decl size: %A" mmod)
        Assert.AreEqual (mmod.vertSizeBytes, 92 , sprintf "incorrect vert size: %A" mmod)
        Assert.AreEqual (mmod.tex0Path, "" , sprintf "incorrect tex0 path: %A" mmod)
        Assert.AreEqual (mmod.tex1Path, "" , sprintf "incorrect tex1 path: %A" mmod)
        Assert.AreEqual (mmod.tex2Path, "" , sprintf "incorrect tex2 path: %A" mmod)
        Assert.AreEqual (mmod.tex3Path, "" , sprintf "incorrect tex3 path: %A" mmod)

    let checkDelMod index pCount vCount = 
        let mmod = ModDBInterop.getModData(index) 
        Assert.AreEqual (mmod.modType, (ModDBInterop.modTypeToInt Deletion) , sprintf "incorrect mod type: %A" mmod)
        Assert.AreEqual (mmod.primType, 4 , sprintf "incorrect prim type: %A" mmod)
        Assert.AreEqual (mmod.primCount, 0 , sprintf "incorrect prim count: %A" mmod)
        Assert.AreEqual (mmod.vertCount, 0 , sprintf "incorrect vert count: %A" mmod)
        Assert.AreEqual (mmod.refPrimCount, pCount , sprintf "incorrect ref prim count: %A" mmod)
        Assert.AreEqual (mmod.refVertCount, vCount , sprintf "incorrect ref vert count: %A" mmod)
        Assert.AreEqual (mmod.indexCount, 0 , sprintf "incorrect index count: %A" mmod)
        Assert.AreEqual (mmod.indexElemSizeBytes, 0 , sprintf "incorrect index size: %A" mmod)
        Assert.AreEqual (mmod.declSizeBytes, 0 , sprintf "incorrect decl size: %A" mmod)
        Assert.AreEqual (mmod.vertSizeBytes, 0 , sprintf "incorrect vert size: %A" mmod)
        Assert.AreEqual (mmod.tex0Path, "" , sprintf "incorrect tex0 path: %A" mmod)
        Assert.AreEqual (mmod.tex1Path, "" , sprintf "incorrect tex1 path: %A" mmod)
        Assert.AreEqual (mmod.tex2Path, "" , sprintf "incorrect tex2 path: %A" mmod)
        Assert.AreEqual (mmod.tex3Path, "" , sprintf "incorrect tex3 path: %A" mmod)

    // del mods
    checkDelMod 1 100 200
    checkDelMod 2 150 300

    // out of range mod
    let () = 
        let mmod = ModDBInterop.getModData(100)
        Assert.AreEqual (mmod, InteropTypes.EmptyModData , "expected empty mod")

    ()