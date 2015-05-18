module TestMeshTransform

open FsUnit
open FsCheck
open NUnit.Framework

open ModelMod
open ModelMod.Types

let vecEq = Util.veqEqEpsilon 0.000001f

[<Test>]
let ``MeshTransform.rotX``() =
    let v = new Vec3F(0.f,1.f,0.f)
    let res = MeshTransform.rotX false 90.f v
    let ex = Vec3F(0.f, 0.f, 1.f)
    Check.QuickThrowOnFailure (vecEq res ex |@ sprintf "rotX: %A %A" res ex)