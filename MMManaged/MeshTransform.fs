namespace ModelMod

open System

open Microsoft.Xna.Framework

open ModTypes

module MeshTransform = 
    let log = Logging.GetLogger("MeshTransform")

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

    let uvFlipY (unused:float32) = 
        let flip(vec:Vector2) = new Vector2(vec.X, 1.f - vec.Y)
        flip
    let uvFlipX (unused:float32) = 
        let flip(vec:Vector2) = new Vector2(1.f - vec.X, vec.Y)
        flip

    let private recenterHelper (normal:bool) (mesh:Mesh) (unused:float32) =
        if normal then failwith "recenter a normal? you crazy?"

        let lowerL,upperR,center = MeshUtil.FindBox(mesh)

        let center = Vector3.Multiply(center,-1.f)
        let recenterAtZero pos =
            Vector3.Add(pos, center)        
        recenterAtZero

    let recenter (mesh:Mesh) = recenterHelper false mesh 0.f

    let noop x y = y

    let private extractFN (xname:string) = 
        let parts = xname.Trim().Split(' ')
        let fnName = parts.[0].ToLowerInvariant()
        fnName, parts

    // Parse a string representing a position or normal transform function, and return a three-tuple of the fn name, the F# function 
    // that the transform and the associated quantity required to do it.  
    // Calling code has an opportunity to change the amount, if needed (for example, to reverse the transform).  
    // TODO: would like to be able to generize this so that the vec2 and vec3 implementions could be combined, but something is forcing
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

    let buildInvocation (fnName:string, fn, amount:float32) = fn amount

    let buildReverseInvocation (fnName:string, fn, amount:float32) = 
        let fnName,fn,amount = reverseFunc(fnName,fn,amount)
        buildInvocation(fnName,fn,amount)