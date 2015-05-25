module TestMesh

open FsUnit
open FsCheck
open NUnit.Framework
open System.IO

open ModelMod
open ModelMod.CoreTypes

let monolith = 
    let mpath = Path.Combine(Util.TestDataDir,"monolithref.mmobj")
    MeshUtil.ReadFrom(mpath,CoreTypes.GPUReplacement)

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
    MeshUtil.WriteObj monolith objPath

    let monolith = MeshUtil.ReadFrom(objPath,CoreTypes.GPUReplacement)

    let check = Check.QuickThrowOnFailure

    check (monolith.Positions.Length = 8 |@ sprintf "incorrect pos count: %A" monolith)
    check (monolith.Normals.Length = 8 |@ sprintf "incorrect nrm count: %A" monolith)
    check (monolith.UVs.Length = 3 |@ sprintf "incorrect uv count: %A" monolith)
    check (monolith.Triangles.Length = 12 |@ sprintf "incorrect uv count: %A" monolith)
    check (monolith.AppliedPositionTransforms = posTransforms |@ sprintf "incorrect pos transforms: %A" monolith)
    check (monolith.AppliedUVTransforms = uvTransforms |@ sprintf "incorrect uv transforms: %A" monolith)
    // tex0 path will NOT currently be read in - it comes from yaml.  this is "by design"
    check (monolith.Tex0Path = "" |@ sprintf "incorrect tex0 path: %A" monolith)

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
