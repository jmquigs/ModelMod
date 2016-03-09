// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace ModelMod

open System

open Microsoft.Xna.Framework

open CoreTypes

/// Helpers for transforming meshes.
module MeshTransform = 
    let private log = Logging.getLogger("MeshTransform")

    let rotX (isNormal:bool) (angleDeg) =
        let mat = Matrix.CreateRotationX(MathHelper.ToRadians angleDeg)
        let rotate (vec:Vector3) = if isNormal then Vector3.TransformNormal(vec,mat) else Vector3.Transform(vec,mat)
        rotate
    let rotY (isNormal:bool) (angleDeg) =
        let mat = Matrix.CreateRotationY(MathHelper.ToRadians angleDeg)
        let rotate (vec:Vector3) = if isNormal then Vector3.TransformNormal(vec,mat) else Vector3.Transform(vec,mat)
        rotate
    let rotZ (isNormal:bool) (angleDeg) =
        let mat = Matrix.CreateRotationZ(MathHelper.ToRadians angleDeg)
        let rotate (vec:Vector3) = if isNormal then Vector3.TransformNormal(vec,mat) else Vector3.Transform(vec,mat)
        rotate

    let uniformScale (isNormal:bool) (amount:float32) =
        let mat = Matrix.CreateScale(amount)
        let scale (vec:Vector3) = 
            if isNormal then 
                let nrm = Vector3.TransformNormal(vec,mat) 
                nrm.Normalize()
                nrm
            else 
                Vector3.Transform(vec,mat)
        scale

    /// Flips the UV Y coordinate (Y <- 1 - Y)
    let uvFlipY (unused:float32) = 
        let flip(vec:Vector2) = new Vector2(vec.X, 1.f - vec.Y)
        flip
    /// Flips the uv X coordinate (X <- 1 - x)
    let uvFlipX (unused:float32) = 
        let flip(vec:Vector2) = new Vector2(1.f - vec.X, vec.Y)
        flip

    let private recenterHelper (normal:bool) (mesh:Mesh) (unused:float32) =
        if normal then failwith "recenter a normal? you crazy?"

        let lowerL,upperR,center = MeshUtil.getBoundingBox(mesh)

        let center = Vector3.Multiply(center,-1.f)
        let recenterAtZero pos =
            Vector3.Add(pos, center)
        recenterAtZero

    /// Recenter a mesh at zero.  NOTE: this function is currently not invertible, 
    /// so is generally unused.
    let recenter (mesh:Mesh) = recenterHelper false mesh 0.f

    let noop x y = y

    let private extractFN (xname:string) = 
        let parts = xname.Trim().Split(' ')
        let fnName = parts.[0].ToLowerInvariant()
        fnName, parts

    /// Parse a string representing a position or normal transform function, and return a three-tuple of the fn name, the F# function 
    /// that the transform and the associated quantity required to do it.
    /// Calling code has an opportunity to change the amount, if needed (for example, to reverse the transform).
    // TODO: would like to be able to generalize this so that the vec2 and vec3 implementions could be combined, but something is forcing
    // a specialization for the vector types.  This doesn't happen with reverseFunc below, though.
    let parseVec3XformFunc (isNormal:bool) (mesh:Mesh) (xname:string) = 
        let dummyRet = "",noop,0.f
        let fnName, parts = extractFN xname
        match fnName with
        | "recenter" -> 
            if isNormal then 
                log.Warn "Recenter transform ignored on normal"
                dummyRet
            else
                fnName,recenterHelper false mesh,0.f
        | "scale" ->
            if parts.Length <> 2 
                then log.Error "Illegal scale, separate args by spaces(ex: 'scale 0.1'): supplied value: %A" xname; dummyRet
                else
                    let amount = parts.[1].Trim() |> Convert.ToSingle
                    fnName,uniformScale isNormal,amount
        | "rot" ->
            if parts.Length <> 3 
                then log.Error "Illegal rotation, separate axis and angle by spaces (ex: 'rot x 90'): supplied value: %A" xname; dummyRet
                else
                    let axis = parts.[1].Trim().ToLowerInvariant()
                    let angDeg = parts.[2].Trim() |> Convert.ToSingle
                    match axis with 
                    | "x" -> fnName,rotX isNormal,angDeg
                    | "y" -> fnName,rotY isNormal,angDeg
                    | "z" -> fnName,rotZ isNormal,angDeg
                    | _ -> log.Error "Unknown rotation axis: %A in value: %A" axis xname; dummyRet
        | "" -> log.Error "Empty string is an invalid transform function"; dummyRet
        | _ -> log.Error "Unrecognized vec3 transform function: %s" fnName; dummyRet

    /// As for parseVec3XformFunc, but for 2d data.
    let parseVec2XformFunc (isNormal:bool) (mesh:Mesh) (xname:string) = 
        let dummyRet = "",noop,0.f
        let fnName, parts = extractFN xname
        match fnName with
        | "flip" ->
            if parts.Length <> 2 
                then log.Error "Illegal flip, separate axis and angle by spaces (ex: 'flip x'): supplied value: %A" xname; dummyRet
                else
                    let axis = parts.[1].Trim().ToLowerInvariant()
                    match axis with 
                    | "x" -> fnName,uvFlipX,0.f
                    | "y" -> fnName,uvFlipY,0.f
                    | _ -> log.Error "Unknown flip axis: %A in value: %A" axis xname; dummyRet
        | _ -> log.Error "Unrecognized vec2 transform function: %s" fnName; dummyRet

    /// Reverse the specified transform function.  The input amount is the amount to be reversed;
    /// The details of the reversal depend on the function.
    let reverseFunc (fnName:string,fn,amount:float32) = 
        let dummyRet = "",noop,0.f
        match fnName with
        | "recenter" -> failwith "doh, I'm too stupid to reverse this."
        | "scale" ->
            let amount = 1.f / amount
            fnName,fn,amount
        | "rot" ->
            let amount = -amount
            fnName,fn,amount
        | "flip" ->
            // No change here since the operation is the same both normal and reverse; just assume parse has set it up correctly
            // and return the inputs
            fnName,fn,amount
        | "" -> log.Error "Empty string is an invalid transform function"; dummyRet
        | _ -> log.Error "Unrecognized reversed transform function: %s" fnName; dummyRet

    /// Given transform function parts, return a function that applies it to 
    /// a single input value.
    let buildInvocation (fnName:string, fn, amount:float32) = fn amount

    /// Given transform function parts, return a function that reverse the transform
    /// and applies it to a single input value.
    let buildReverseInvocation (fnName:string, fn, amount:float32) = 
        let fnName,fn,amount = reverseFunc(fnName,fn,amount)
        buildInvocation(fnName,fn,amount)

    /// Apply the list of named transforms to the specified mesh.  The invokeBuilder function allows you 
    /// to change the order in which they are applied (see buildInvocation,buildReverseInvocation).
    /// This function operates on spatial mesh data (positions and normals).
    // NOTE: this implementation has some good points, but I'm sure it could be done more simply.
    let private applyMeshTransformsInternal (xforms:string list) (invokeBuilder) (mesh:Mesh) =
        if xforms.IsEmpty then
            mesh
        else
            // helper function to allow the mesh to be passed to parseXformFunc
            let getMeshXformFunc xname = parseVec3XformFunc false mesh xname

            // convert all the names (and arguments) into functions
            let funcs = xforms |> List.map getMeshXformFunc
            // reduce to apply the functions in order with a single function
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (>>)

            let mesh = MeshUtil.applyPositionTransformation compositeTransform mesh

            // also need to do normals
            let getMeshXformFunc xname = parseVec3XformFunc true mesh xname
            let funcs = xforms |> List.map getMeshXformFunc
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (>>)
            let mesh = MeshUtil.applyNormalTransformation compositeTransform mesh
            mesh

    /// As for applyMeshTransformsInternal, but for uv coordinates.
    let private applyUVTransformsInternal (uv_xforms:string list) (invokeBuilder) (mesh:Mesh) =
        if uv_xforms.IsEmpty then
            mesh
        else
            let getMeshXformFunc xname = parseVec2XformFunc false mesh xname
            let funcs = uv_xforms |> List.map getMeshXformFunc
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (>>)

            let mesh = MeshUtil.applyUVTransformation compositeTransform mesh
            mesh

    /// Apply the specified list of transforms to the mesh.
    let applyMeshTransforms (xforms:string list) (uvXforms:string list) (mesh:Mesh) = 
        let mesh = applyMeshTransformsInternal xforms buildInvocation mesh 
        let mesh = applyUVTransformsInternal uvXforms buildInvocation mesh 
        mesh

    /// Reverse each supplied transform, then apply them to the mesh.
    let reverseMeshTransforms (xforms:string list) (uvXforms:string list) (mesh:Mesh) = 
        let mesh = applyMeshTransformsInternal (List.rev xforms) buildReverseInvocation mesh 
        let mesh = applyUVTransformsInternal (List.rev uvXforms) buildReverseInvocation mesh 
        mesh