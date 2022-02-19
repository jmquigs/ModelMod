module TestModDB

open FsUnit
open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod
open ModelMod.CoreTypes

[<Test>]
let ``ModDB: load mod db``() =
    let mpath = Path.Combine(Util.TestDataDir, "ModIndex.yaml")
    let mdb = 
        ModDB.loadModDB
            ({ 
                StartConf.Conf.ModIndexFile = Some(mpath)
                FilesToLoad = []
                AppSettings = None
            }, None)

    Assert.AreEqual (mdb.Mods.Length, 2, sprintf "incorrect number of mods: %A" mdb.Mods)
    Assert.AreEqual (mdb.References.Length, 1, sprintf "incorrect number of references: %A" mdb.References)
    Assert.AreEqual (mdb.MeshRelations.Length, 1, sprintf "incorrect number of meshrels: %A" mdb.MeshRelations)

    // check ref
    let () =
        let mref = List.head mdb.References
        Assert.AreEqual (mref.Name, "MonolithRef", sprintf "wrong ref name: %A" mref)
        // check ref mesh, a few properties at least
        let refMesh = mref.Mesh
        Assert.AreEqual (refMesh.Positions.Length, 8, sprintf "wrong ref mesh vert count: %A" refMesh)
        Assert.AreEqual (refMesh.Triangles.Length, 12, sprintf "wrong ref mesh prim count: %A" refMesh)

    // check mod
    let () =
        let mmod = List.nth mdb.Mods 0
        let mref = List.head mdb.References
        let refMesh = mref.Mesh

        Assert.AreEqual (mmod.Name, "MonolithMod", sprintf "wrong mod name: %A" mmod)
        Assert.AreEqual (mmod.RefName, Some("MonolithRef"), sprintf "wrong mod ref name: %A" mmod)
        Assert.AreEqual (mmod.Ref, Some(mref), sprintf "wrong mod ref: %A" mmod)
        Assert.IsTrue (mmod.Mesh <> None, sprintf "wrong mod mesh: %A" mmod)
        let attributes = {
            DeletedGeometry = []
        }
        Assert.AreEqual (mmod.Attributes, attributes, sprintf "wrong mod attributes: expected %A, got %A" attributes mmod.Attributes)

        // check mod mesh
        let modMesh = Option.get mmod.Mesh
        Assert.AreEqual (modMesh.Positions.Length, 24, sprintf "wrong ref mesh vert count: %A" refMesh)
        Assert.AreEqual (modMesh.Triangles.Length, 36, sprintf "wrong ref mesh prim count: %A" refMesh)

    // check deletion mod
    let () =
        let dmod = List.nth mdb.Mods 1
        let delGeometry = [ { PrimCount = 100; VertCount = 200 }; { PrimCount = 150; VertCount = 300 }; ]
        let attributes = {
            DeletedGeometry = delGeometry
        }
        Assert.AreEqual (dmod.Name, "DelMod", sprintf "wrong mod name: %A" dmod)
        Assert.AreEqual (dmod.RefName, None, sprintf "wrong mod ref name: %A" dmod)
        Assert.AreEqual (dmod.Ref, None, sprintf "wrong mod ref: %A" dmod)
        Assert.AreEqual (dmod.Mesh, None, sprintf "wrong mod mesh: %A" dmod)
        Assert.AreEqual (dmod.Attributes, attributes, sprintf "wrong mod attributes: expected %A, got %A" attributes dmod.Attributes)

        // should be two deletion mods in the database, one for each prim/vert pair.  leave further checking for the interop test
        Assert.AreEqual (mdb.DeletionMods.Length, 2, sprintf "wrong del mod count: %A" mdb.DeletionMods)

    ()

    
    

