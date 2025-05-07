module TestMeshTransform

open NUnit.Framework
open System.IO


open ModelMod
open ModelMod.CoreTypes

let vecEq = Util.veqEqEpsilon 0.000001f

let monolith =
    let mpath = Path.Combine(Util.TestDataDir,"monolithref.mmobj")
    MeshUtil.readFrom(mpath,CoreTypes.GPUReplacement,CoreTypes.DefaultReadFlags)

[<Test>]
let ``MeshTransform: basic rotation``() =
    let v = new Vec3F(0.f,1.f,0.f)
    let res = MeshTransform.rotX false 90.f v
    let ex = Vec3F(0.f, 0.f, 1.f)
    Assert.IsTrue (vecEq res ex, sprintf "rotX: %A %A" res ex)

[<Test>]
let ``MeshTransform: go ape with the monolith``() =
    // this test is too big, but it does exercise the majority of the transforming code, at least
    let nm = monolith |> MeshTransform.applyMeshTransforms [|"rot x 90"; "rot z 45"; "scale 0.5"|] [||]

    // for visual debugging:
    //MeshUtil.WriteObj nm (Path.Combine(Util.TestDataDir, "monolith.OUT.mmobj"))

    // check first few positions
    let checkPos p1 ep1 msg =
        Assert.IsTrue (vecEq p1 ep1, sprintf "%s: got: %A; expected: %A" msg p1 ep1)

    checkPos nm.Positions.[0] (Vec3F(1.414214f,1.414214f,0.f)) "mismatch on pos 0"
    checkPos nm.Positions.[1] (Vec3F(1.06066f,1.767767f,0.f)) "mismatch on pos 1"
    checkPos nm.Positions.[2] (Vec3F(-0.3535534f,0.3535534f,0.f)) "mismatch on pos 2"
    checkPos nm.Positions.[3] (Vec3F(0.f,0.f,0.f)) "mismatch on pos 3"


