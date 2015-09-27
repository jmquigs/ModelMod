module TestModDB

open FsUnit
open FsCheck
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
            })

    let check = Check.QuickThrowOnFailure
    check (mdb.Mods.Length = 2 |@ sprintf "incorrect number of mods: %A" mdb.Mods)
    check (mdb.References.Length = 1 |@ sprintf "incorrect number of references: %A" mdb.References)
    check (mdb.MeshRelations.Length = 1 |@ sprintf "incorrect number of meshrels: %A" mdb.MeshRelations)

    // check ref
    let () =
        let mref = List.head mdb.References
        check (mref.Name = "MonolithRef" |@ sprintf "wrong ref name: %A" mref)
        // check ref mesh, a few properties at least
        let refMesh = mref.Mesh
        check (refMesh.Positions.Length = 8 |@ sprintf "wrong ref mesh vert count: %A" refMesh)
        check (refMesh.Triangles.Length = 12 |@ sprintf "wrong ref mesh prim count: %A" refMesh)

    // check mod
    let () =
        let mmod = List.nth mdb.Mods 0
        let mref = List.head mdb.References
        let refMesh = mref.Mesh

        check (mmod.Name = "MonolithMod" |@ sprintf "wrong mod name: %A" mmod)
        check (mmod.RefName = Some("MonolithRef") |@ sprintf "wrong mod ref name: %A" mmod)
        check (mmod.Ref = Some(mref) |@ sprintf "wrong mod ref: %A" mmod)
        check (mmod.Mesh <> None |@ sprintf "wrong mod mesh: %A" mmod)
        let attributes = {
            DeletedGeometry = []
        }
        check (mmod.Attributes = attributes |@ sprintf "wrong mod attributes: expected %A, got %A" attributes mmod.Attributes)

        // check mod mesh
        let modMesh = Option.get mmod.Mesh
        check (modMesh.Positions.Length = 24 |@ sprintf "wrong ref mesh vert count: %A" refMesh)
        check (modMesh.Triangles.Length = 36 |@ sprintf "wrong ref mesh prim count: %A" refMesh)

    // check deletion mod
    let () =
        let dmod = List.nth mdb.Mods 1
        let delGeometry = [ { PrimCount = 100; VertCount = 200 }; { PrimCount = 150; VertCount = 300 }; ]
        let attributes = {
            DeletedGeometry = delGeometry
        }
        check (dmod.Name = "DelMod" |@ sprintf "wrong mod name: %A" dmod)
        check (dmod.RefName = None |@ sprintf "wrong mod ref name: %A" dmod)
        check (dmod.Ref = None |@ sprintf "wrong mod ref: %A" dmod)
        check (dmod.Mesh = None |@ sprintf "wrong mod mesh: %A" dmod)
        check (dmod.Attributes = attributes |@ sprintf "wrong mod attributes: expected %A, got %A" attributes dmod.Attributes)

        // should be two deletion mods in the database, one for each prim/vert pair.  leave further checking for the interop test
        check (mdb.DeletionMods.Length = 2 |@ sprintf "wrong del mod count: %A" mdb.DeletionMods)

    ()

    
    

