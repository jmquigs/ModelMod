﻿module TestMeshTransform

open FsUnit
open FsCheck
open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod
open ModelMod.Types

let vecEq = Util.veqEqEpsilon 0.000001f

// dig up the monolith
let tdata = 
    let asmPath = Assembly.GetExecutingAssembly().CodeBase.Replace("file:///","")
    let tdata = Path.GetFullPath(Path.Combine(asmPath,@"..\..\..\..\TestData"))
    tdata

let monolith = 
    let mpath = Path.Combine(tdata,"monolith.mmobj")
    MeshUtil.ReadFrom(mpath,ModTypes.GPUReplacement)

[<Test>]
let ``MeshTransform: rotX``() =
    let v = new Vec3F(0.f,1.f,0.f)
    let res = MeshTransform.rotX false 90.f v
    let ex = Vec3F(0.f, 0.f, 1.f)
    Check.QuickThrowOnFailure (vecEq res ex |@ sprintf "rotX: %A %A" res ex)

[<Test>]
let ``MeshTransform: Go ape with the monolith``() =
    // this test is too big, but it does exercise the majority of the transforming code, at least
    let nm = monolith |> MeshTransform.applyMeshTransforms ["rot x 90"; "rot z 45"; "scale 0.5"] [] 

    // for visual debugging:    
    //MeshUtil.WriteObj newMonolith (Path.Combine(tdata, "monolith.OUT.mmobj"))

    // check first few positions
    let checkPos p1 ep1 msg = 
        Check.QuickThrowOnFailure (vecEq p1 ep1 |@ sprintf "%s: %A %A" msg p1 ep1)

    checkPos nm.Positions.[0] (Vec3F(0.f,0.f,0.f)) "mismatch on pos 0"
    checkPos nm.Positions.[1] (Vec3F(1.414214f,1.414214f,0.f)) "mismatch on pos 1"
    checkPos nm.Positions.[2] (Vec3F(-0.3535534f,0.3535534f,0.f)) "mismatch on pos 2"
    checkPos nm.Positions.[3] (Vec3F(1.06066f,1.767767f,0.f)) "mismatch on pos 3"

    
    