﻿module TestMesh

open NUnit.Framework
open System.IO

open ModelMod
open ModelMod.CoreTypes

let monolith =
    let mpath = Path.Combine(Util.TestDataDir,"monolithref.mmobj")
    MeshUtil.readFrom(mpath,CoreTypes.GPUReplacement,CoreTypes.DefaultReadFlags)

open MonoGameHelpers

[<Test>]
let ``Mesh: mono game helpers``() =
    Assert.AreEqual(500us, floatToHalfUint16(halfUint16ToFloat(500us)), "float conversion failed")

[<Test>]
let ``Mesh: write``() =
    let objPath = Path.Combine(Util.TestDataDir, "monolith.TestWrite.mmobj")
    let mtlPath = Path.Combine(Util.TestDataDir, "monolith.TestWrite.mtl")
    if File.Exists objPath then File.Delete objPath
    if File.Exists mtlPath then File.Delete mtlPath

    // fake some data
    let posTransforms = [|"scale 2"|]
    let uvTransforms = [| "flip x" |]
    let tex0 = "dummy.dds"
    let monolith = {
        monolith with
            AppliedPositionTransforms = posTransforms
            AppliedUVTransforms = uvTransforms
            Tex0Path = tex0
    }
    MeshUtil.writeObj monolith objPath

    let monolith = MeshUtil.readFrom(objPath,CoreTypes.GPUReplacement,CoreTypes.DefaultReadFlags)

    Assert.AreEqual (monolith.Positions.Length, 8, sprintf "incorrect pos count: %A" monolith)
    Assert.AreEqual (monolith.Normals.Length, 8, sprintf "incorrect nrm count: %A" monolith)
    Assert.AreEqual (monolith.UVs.Length, 3, sprintf "incorrect uv count: %A" monolith)
    Assert.AreEqual (monolith.Triangles.Length, 12, sprintf "incorrect uv count: %A" monolith)
    Assert.AreEqual (monolith.AppliedPositionTransforms, posTransforms, sprintf "incorrect pos transforms: %A" monolith)
    Assert.AreEqual (monolith.AppliedUVTransforms, uvTransforms, sprintf "incorrect uv transforms: %A" monolith)
    // tex0 path will NOT currently be read in - it comes from yaml.  this is "by design"
    Assert.AreEqual (monolith.Tex0Path, "", sprintf "incorrect tex0 path: %A" monolith)

    // check some stuff textually
    let checkHasLine (text:string[]) (x:string) =
        let found = text |> Array.tryFind (fun s -> s.Trim() = x.Trim() )
        match found with
        | None -> failwithf "line '%A' not found in text" x
        | Some s -> ()

    let objHasLine = checkHasLine (File.ReadAllLines(objPath))
    let mtlHasLine = checkHasLine (File.ReadAllLines(mtlPath))

    objHasLine "mtllib monolith.TestWrite.mtl"
    mtlHasLine "map_Kd dummy.dds"
