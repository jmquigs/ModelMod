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
        ModDB.LoadModDB
            ({ 
                MMView.Conf.ModIndexFile = Some(mpath)
                FilesToLoad = []
                AppSettings = None
            })

    Check.QuickThrowOnFailure (mdb.Mods.Length = 1 |@ sprintf "incorrect number of mods: %A" mdb.Mods)
    Check.QuickThrowOnFailure (mdb.References.Length = 1 |@ sprintf "incorrect number of references: %A" mdb.References)
    Check.QuickThrowOnFailure (mdb.MeshRelations.Length = 1 |@ sprintf "incorrect number of meshrels: %A" mdb.MeshRelations)

    // check ref
    let mref = List.head mdb.References
    Check.QuickThrowOnFailure (mref.Name = "MonolithRef" |@ sprintf "wrong ref name: %A" mref)
    // check ref mesh, a few properties at least
    let refMesh = mref.Mesh
    Check.QuickThrowOnFailure (refMesh.Positions.Length = 8 |@ sprintf "wrong ref mesh vert count: %A" refMesh)
    Check.QuickThrowOnFailure (refMesh.Triangles.Length = 12 |@ sprintf "wrong ref mesh prim count: %A" refMesh)

    // check mod
    let mmod = List.head mdb.Mods
    Check.QuickThrowOnFailure (mmod.Name = "MonolithMod" |@ sprintf "wrong mod name: %A" mmod)
    Check.QuickThrowOnFailure (mmod.RefName = Some("MonolithRef") |@ sprintf "wrong mod ref name: %A" mmod)
    Check.QuickThrowOnFailure (mmod.Ref = Some(mref) |@ sprintf "wrong mod ref: %A" mmod)
    Check.QuickThrowOnFailure (mmod.Attributes = EmptyModAttributes |@ sprintf "wrong mod attributes: %A" mmod)
    Check.QuickThrowOnFailure (mmod.Mesh <> None |@ sprintf "wrong mod mesh: %A" mmod)
    // check mod mesh
    let modMesh = Option.get mmod.Mesh
    Check.QuickThrowOnFailure (modMesh.Positions.Length = 24 |@ sprintf "wrong ref mesh vert count: %A" refMesh)
    Check.QuickThrowOnFailure (modMesh.Triangles.Length = 36 |@ sprintf "wrong ref mesh prim count: %A" refMesh)

