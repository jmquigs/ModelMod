module Util

open System

open ModelMod.Types

let veqEqEpsilon (ep:float32) (v1:Vec3F) (v2:Vec3F) =
    let dx = Math.Abs(v1.X - v2.X) 
    let dy = Math.Abs(v1.Y - v2.Y)
    let dz = Math.Abs(v1.Z - v2.Z)
    dx < ep && dy < ep && dz < ep
