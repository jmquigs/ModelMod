module Util

open System
open System.Reflection
open System.IO

open ModelMod.CoreTypes

let veqEqEpsilon (ep:float32) (v1:Vec3F) (v2:Vec3F) =
    let dx = Math.Abs(v1.X - v2.X) 
    let dy = Math.Abs(v1.Y - v2.Y)
    let dz = Math.Abs(v1.Z - v2.Z)
    dx < ep && dy < ep && dz < ep

let TestDataDir = 
    let asmPath = Assembly.GetExecutingAssembly().CodeBase.Replace("file:///","")
    let tdata = Path.GetFullPath(Path.Combine(asmPath,@"..\..\..\..\TestData"))
    tdata