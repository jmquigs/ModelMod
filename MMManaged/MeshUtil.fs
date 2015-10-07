// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace ModelMod

open System
open System.IO
open System.Collections.Generic
open System.Text.RegularExpressions
open System.Text
open System.Reflection

open Microsoft.Xna.Framework
open Microsoft.Xna.Framework.Graphics

open CoreTypes

type SDXDT = SharpDX.Direct3D9.DeclarationType

/// Wrapper module for utilities imported from monogame.
module MonoGameHelpers =
    // "import" some private methods from monogame for working with half-precision floats.  
    // This is the second-worst way to do this (worst being copy paste).  
    let halfTypeHelper = typeof<Microsoft.Xna.Framework.Vector2>.Assembly.GetType("Microsoft.Xna.Framework.Graphics.PackedVector.HalfTypeHelper")

    let private mgHalfUint16ToFloat = halfTypeHelper.GetMethod("Convert", BindingFlags.Static ||| BindingFlags.NonPublic, null, [| typeof<uint16> |], null)
    let private mgfloatToHalfUint16 = halfTypeHelper.GetMethod("Convert", BindingFlags.Static ||| BindingFlags.NonPublic, null, [| typeof<System.Single> |], null)

    /// Convert a half-precision float represented by a uint16 into a float.
    let halfUint16ToFloat (u:uint16) =
        if mgHalfUint16ToFloat = null then failwith "mgHalfUint16ToFloat is null; failed to import private method from monogame?"
        mgHalfUint16ToFloat.Invoke(null, [| u |]) :?> float32

    /// Convert a float into a half-precision float represented by a uint16.
    let floatToHalfUint16  (f:float32) =
        if mgfloatToHalfUint16 = null then failwith "mgfloatToHalfUint16 is null; failed to import private method from monogame?"
        mgfloatToHalfUint16.Invoke(null, [| f |]) :?> uint16

module MeshUtil =
    let private log = Logging.getLogger("Mesh")

    type MtlLib = {
        MapKd: string
    }

    /// Returns a string representation of a face in obj format (PTN; indices are 1-based,
    /// e.g.: "1/4/10")
    let faceToString(face: PTNIndex[]) =
        let inc x = x + 1

        let sb = new StringBuilder()
        Array.iter (fun (v: PTNIndex) ->
            if sb.Length <> 0 then ignore(sb.Append(" "))
            ignore(sb.Append(sprintf "%d/%d/%d" (inc v.Pos) (inc v.Tex) (inc v.Nrm)))
        ) face
        sb.ToString()

    /// Read an obj mttlib file.
    let readMtlLib filename = 
        let lines = File.ReadAllLines(filename)
        let mutable mtllib = {
            MapKd = ""
        }

        let makeName (s:string[] option) =
            match s with 
            | None -> None
            | Some n -> Some (n.[0])
        let (|StringValue|_|) pattern str = REUtil.checkGroupMatch pattern 2 str |> REUtil.extract 1 (fun s -> s) |> makeName

        for line in lines do
            match line with 
            | StringValue @"map_Kd\s+(\S+).*" (texFile) ->
                // store absolute path in case the renderer doesn't have the obj path information to reconstruct it (e.g.: preview window)
                let texFile = Path.Combine(Path.GetDirectoryName filename, texFile)
                mtllib <- { mtllib with MapKd = texFile }
            | _ -> ()
        mtllib
            
    /// Read an obj (or MMObj) file, and return the 
    /// constructed Mesh object.
    let readObj(filename,modType,flags:MeshReadFlags): Mesh =
        // The basic strategy here is to use a bunch of active patterns 
        // that use regexps to recognize and convert various types of data.  
        // We define a bunch of helper functions, pack them up into 
        // active patterns, then run a match with all the patterns on each line.

        //use sw = new Util.StopwatchTracker("read obj: " + filename)
        let lines = File.ReadAllLines(filename)

        // obj indices are 1 based, this makes them zero-based
        let sub1 (components:int32[] option) = 
            match components with
            | Some v -> Some (Array.map (fun x -> x - 1 ) v)
            | _ -> None

        let makeVec2f (components:float32[] option) =
            match components with
            | Some v when v.Length = 2 -> Some (Vec2F(v.[0], v.[1]))
            | _ -> None

        let makeVec3f (components:float32[] option) =
            match components with
            | Some v when v.Length = 3 -> Some (Vec3F(v.[0], v.[1], v.[2])) 
            | _ -> None

        let make3PTNIndex (components:int32[] option) =
            match components with
            | Some v when v.Length = 9 -> 
                Some( [| {Pos=v.[0]; Tex=v.[1]; Nrm=v.[2]}; {Pos=v.[3]; Tex=v.[4]; Nrm=v.[5]}; {Pos=v.[6]; Tex=v.[7]; Nrm=v.[8]} |] ) 
            | _ -> None

        let makeBlendVectors (components:(int32 * float32)[] option) =
            match components with
            | Some v when v.Length = 4 ->
                let indices = new Vec4X(fst v.[0], fst v.[1], fst v.[2], fst v.[3])
                let weights = new Vec4F(snd v.[0], snd v.[1], snd v.[2], snd v.[3])

                // TODO: hack fix: the weights MUST sum to 1.0, or else bad shit happens in game.  
                // I think I have some bad rounding going on somewhere
                // in the conversion/capture of these; either in snapshotting, or in blender, 
                // since the differences are small.
                let sum = weights.X + weights.Y + weights.Z + weights.W 
                let weights = 
                    if ((1.f - sum) > 0.f) then
                        new Vec4F(weights.X + (1.f - sum), weights.Y, weights.Z, weights.W)
                    else
                        weights

                Some (indices,weights)
            | _ -> None

        let extractBlendPair (s:string) =
            let parts = s.Split('/')
            if parts.Length <> 2 then failwithf "Illegal blend pair: %A" s
            let idx = parts.[0].Trim()
            let weight = parts.[1].Trim()
            int32 idx,float32 weight

        let extractTransform (s:string) =
            // all the transforms will be space-separated in the first RE group
            let parts = s.Split(' ')
            parts |> Array.map Util.replaceUnderscoreWithSpace

        let makeTransform (s:string [] [] option) =
            match s with 
            | None -> None
            | Some xs -> Some (List.ofArray xs.[0]) // all transforms bundled in first array index since we only have one RE group that matches all of them

        let extractVGroups (s:string) =
            let parts = s.Split(' ')
            Array.map int parts 

        let makeVGroupList (s:int [] [] option) =
            match s with 
            | None -> None
            | Some xs -> Some (List.ofArray xs.[0])

        let makeName (s:string[] option) =
            match s with 
            | None -> None
            | Some n -> Some (n.[0])

        let stringStartsWithAny (prefixes:string list) (s:string) =
            let found = 
                prefixes |> List.tryFind (fun prefix ->
                    s.Trim().StartsWith(prefix.Trim(), StringComparison.InvariantCultureIgnoreCase)
                )
            match found with
            | None -> None
            | Some prefix -> Some (s)
            
        let (|Vec2f|_|) pattern str = REUtil.checkGroupMatch pattern 3 str |> REUtil.extract 1 float32 |> makeVec2f
        let (|Vec3f|_|) pattern str = REUtil.checkGroupMatch pattern 4 str |> REUtil.extract 1 float32 |> makeVec3f
        let (|BlendPairs|_|) pattern str = REUtil.checkGroupMatch pattern 5 str |> REUtil.extract 1 extractBlendPair |> makeBlendVectors
        let (|VertexGroupName|_|) pattern str = REUtil.checkGroupMatch pattern 2 str |> REUtil.extract 1 (fun s -> s) |> makeName
        let (|TransformFunctionList|_|) pattern str = REUtil.checkGroupMatch pattern 2 str |> REUtil.extract 1 extractTransform |> makeTransform
        let (|PTNIndex3|_|) pattern str = REUtil.checkGroupMatch pattern 10 str |> REUtil.extract 1 int32 |> sub1 |> make3PTNIndex
        let (|VertexGroupList|_|) pattern str = REUtil.checkGroupMatch pattern 2 str |> REUtil.extract 1 extractVGroups |> makeVGroupList
        let (|MtlLib|_|) pattern str = 
            if flags.ReadMaterialFile then
                REUtil.checkGroupMatch pattern 2 str |> REUtil.extract 1 (fun s -> s) |> makeName
            else
                None
        let (|SpecialGroup|_|) = stringStartsWithAny ["Index.";"PosTransform.";"UVTransform."]
        let (|DoubleDotAnnotation|_|) (str:string) = 
            let idx = str.IndexOf('.')
            if idx <> -1 then
                let idx = str.IndexOf('.', idx+1)
                if idx <> -1 then
                    Some (str.Substring(idx+1).Trim())
                else
                    None
            else
                None
        let (|VgroupAnnotation|_|) (str:string) = 
            // an annotated group is any group whose name matches one of the following
            // - it doesn't start with one of the special group names (Index., etc)
            // - if it does start with one of those names, then it is of the form "something.data.annotation"; the annotation is everything after the 
            // second period, including other periods.
            match str with 
            | SpecialGroup sgroup ->
                match sgroup with 
                | DoubleDotAnnotation annt -> Some(annt)
                | _ -> None
            | _ -> Some(str)
        
        let positions = new ResizeArray<Vec3F>()
        let normals = new ResizeArray<Vec3F>()
        let uvs = new ResizeArray<Vec2F>()

        let blendindices = new ResizeArray<Vec4X>()
        let blendweights = new ResizeArray<Vec4F>()

        let postransforms = new ResizeArray<string>()
        let uvtransforms = new ResizeArray<string>()

        let vgnames = new ResizeArray<string>()
        let avgnames = new ResizeArray<string>()

        let groupsForVertex = new ResizeArray<int list>()
        let posAt i = positions.[i]

        let triangles = new ResizeArray<IndexedTri>()

        let mutable mtllib = {
            MapKd = ""
        }
           
        // walk the file lines to store component data in the resize arrays
        // performance note: this method generates lots of regexp "misses".  
        // restructuring the code so that the last-successfully matched pattern is run
        // first (using an MRU array or whatever) dramatically reduces the misses and 
        // improves performance by about 10%.  However, it made this code a lot uglier,
        // so I didn't commit it.
        for line in lines do
            match line with 
                | Vec2f @"vt\s+(\S+)\s+(\S+).*" vt ->
                    uvs.Add(vt)
                | Vec3f @"v\s+(\S+)\s+(\S+)\s+(\S+).*" v ->
                    positions.Add(v) 
                | Vec3f @"vn\s+(\S+)\s+(\S+)\s+(\S+).*" vn ->
                    normals.Add(vn)
                | VertexGroupName @"#vgn\s+(\S+).*" (vgroup) ->
                    if not (vgnames.Contains(vgroup)) then 
                        vgnames.Add(vgroup)
                        // we only care about the annotations (if any) so extract them now and store them as the group name.
                        // store groups without any annotations with an empty string to preserve indices.
                        let annt = 
                            match vgroup with
                            | VgroupAnnotation annt -> annt
                            | _ -> ""
                        avgnames.Add(annt)
                | VertexGroupList @"#vg\s+(.*)$" (vgroups) ->
                    groupsForVertex.Add(vgroups)
                | BlendPairs @"#vbld\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+).*" (bi,bw) ->
                    blendindices.Add(bi)
                    blendweights.Add(bw)
                | TransformFunctionList @"#pos_xforms\s+(.*)$" (xforms) ->
                    xforms |> List.iter (fun xf -> if not (postransforms.Contains(xf)) then  postransforms.Add(xf))
                | TransformFunctionList @"#uv_xforms\s+(.*)$" (xforms) ->
                    xforms |> List.iter (fun xf -> if not (uvtransforms.Contains(xf)) then uvtransforms.Add(xf))
                | PTNIndex3 @"f\s+(\S+)/(\S+)/(\S+)\s+(\S+)/(\S+)/(\S+)\s+(\S+)/(\S+)/(\S+).*" v ->
                    triangles.Add({Verts=v})
                | MtlLib @"mtllib\s+(\S+).*" (mtlFile) ->
                    let path = Path.Combine(Path.GetDirectoryName(filename), mtlFile)
                    mtllib <- readMtlLib path
                | _ -> () //printfn "unknown line: %s" line

        let triangles = triangles.ToArray()

        if triangles.Length = 0 then
            failwithf "Error load meshing file %s: no faces found; check that normals and texture coordinates are present" filename

        // dereference the vertex group annotations for each vertex.  Filter out any empty annotations, so that each vert that has no annotations
        // has an empty list element in the resulting array.
        let groupsForVertex = 
            groupsForVertex.ToArray() 
            |> Array.map (fun vgList -> 
                List.foldBack (fun idx acc -> 
                    let annt = if idx >= 0 then avgnames.[idx] else ""
                    let acc = if annt = "" then acc else annt::acc
                    acc
                ) vgList []
            )
        //groupsForVertex |> Array.iteri (fun i vlst -> if not (List.isEmpty vlst) then printfn "vert %d has annotated groups: %A" i vlst )

        log.Info "Loaded %s:" filename
        log.Info "  %d triangles, %d positions, %d uvs, %d normals" triangles.Length positions.Count uvs.Count normals.Count 
        log.Info "  %d blend indices, %d blend weights" blendindices.Count blendweights.Count
        log.Info "  %d position transforms; %d uv transforms" postransforms.Count uvtransforms.Count
        log.Info "  %d named vertex groups; %d vertex/group associations " vgnames.Count groupsForVertex.Length

        if groupsForVertex.Length > 0 && positions.Count > groupsForVertex.Length then
            // this may be an art bug
            let diff = positions.Count - groupsForVertex.Length
            log.Warn "Found %d vertices that are not assigned to any group; these verts will not participate in exclusion checking.  \
                To avoid this message, make sure that all verts are assigned to a named group" diff
    
        let ret = { 
            Mesh.Type = modType
            Triangles = triangles
            Positions = positions.ToArray()
            UVs = uvs.ToArray()
            Normals = normals.ToArray()
            BlendIndices = blendindices.ToArray()
            BlendWeights = blendweights.ToArray()
            Declaration = None
            BinaryVertexData = None
            AppliedPositionTransforms = postransforms.ToArray()
            AppliedUVTransforms = uvtransforms.ToArray()
            AnnotatedVertexGroups = groupsForVertex
            // During normal game load, we don't read the mtl file, so MapKd will be blank here.  
            // Override texture paths, if any, must currently come from the yaml file.
            Tex0Path = mtllib.MapKd
            Tex1Path = ""
            Tex2Path = ""
            Tex3Path = ""
        }

        ret

    let private MaterialFileTemplate = """# ModelMod material file
newmtl (null)
map_Kd $$filename
"""
    

    /// Write a mesh to the specified path in MMObj format.
    let writeObj (md:Mesh) outpath =
        let lines = new ResizeArray<string>()
    
        // currently we only write materials for texture 0
        match md.Tex0Path.Trim() with
        | "" -> ()
        | p -> 
            let dir = Path.GetDirectoryName(outpath)
            let file = Path.GetFileNameWithoutExtension(outpath) + ".mtl"
            // omit dir to use same directory as obj for mlt and texture file (makes files easily movable)
            lines.Add ("mtllib " + file)
            let fileDat = MaterialFileTemplate.Replace("$$filename", p) 
            File.WriteAllText(Path.Combine(dir,file), fileDat)
        
        lines.Add("o MMSnapshot")

        // game may have indices but no weights; warn in this case, but 
        // write out the indices anyway with zero weights.
        let blendindices,blendweights = 
            match (md.BlendIndices.Length > 0, md.BlendWeights.Length > 0) with
                | true,true -> 
                    // should be same length
                    if md.BlendIndices.Length <> md.BlendWeights.Length
                        then failwithf "blend data length array mismatch: indices: %A, weights: %A" md.BlendIndices.Length md.BlendWeights.Length
                    md.BlendIndices,md.BlendWeights
                | true,false -> 
                    log.Warn "Mesh has blend indices but no weights, using fixed weights"
                    let blendweights = md.BlendIndices |> Array.map (fun idx ->
                        // Assign fake weights for each index.  Right now we assume that the first component of the index (X) is the only 
                        // valid one.  We could look for non-zero indicies and only assign weights to them, but we have no way to differentiate
                        // index 0 (a valid index which would either be completely excluded or assigned to everything).
                        // Assigning a fake weight allows at least allows the weighted verts to be viewed in the 3d tool.
                        // TODO: really we should try to find where the weights are stashed in this case during snapshot; they are probably in a
                        // vertex shader constant.
                        Vec4F(1.f,0.f,0.f,0.f)
                    )
                    md.BlendIndices, blendweights
                | false,true ->
                    // this is weird
                    failwithf "Mesh has blend weights but no indices"
                | false,false ->
                    log.Warn "No blend data detected"
                    [||],[||]                

        md.Positions |> Array.iteri (fun i pos ->
            let line = sprintf "v %f %f %f" pos.X pos.Y pos.Z 
    
            lines.Add(line)
        )

        md.UVs |> Array.iter (fun uv ->
            lines.Add(sprintf "vt %f %f" uv.X uv.Y)
        )

        md.Normals |> Array.iter (fun nrm -> 
            lines.Add(sprintf "vn %f %f %f" nrm.X nrm.Y nrm.Z)
        )

        lines.Add("usemtl (null)")
        lines.Add("s off")

        md.Triangles |> Array.iter (fun tri ->
            lines.Add("f " + (faceToString tri.Verts))
        )

        Array.iter2 (fun (indices:Vec4X) (weights:Vec4F) ->
            let line = 
                sprintf "#vbld %A/%2.6f %A/%2.6f %A/%2.6f %A/%2.6f" 
                    indices.X weights.X 
                    indices.Y weights.Y 
                    indices.Z weights.Z 
                    indices.W weights.W
            lines.Add(line)
        ) blendindices blendweights
            
        let objTransformList x = String.concat " " (Array.map Util.replaceSpaceWithUnderscore x)

        if not (Array.isEmpty md.AppliedPositionTransforms) then
            let line = "#pos_xforms " + (objTransformList md.AppliedPositionTransforms)
            lines.Add(line)
           
        if not (Array.isEmpty md.AppliedUVTransforms) then
            let line = "#uv_xforms " + (objTransformList md.AppliedUVTransforms)
            lines.Add(line)

        File.WriteAllLines(outpath, lines.ToArray())

    /// Returns the bounding box of a mesh.
    let getBoundingBox(mesh:Mesh) =
        let maxFloat = System.Single.MaxValue

        let lowerL = new Vector3(System.Single.MaxValue,System.Single.MaxValue,System.Single.MaxValue)
        let upperR = new Vector3(-System.Single.MaxValue,-System.Single.MaxValue,-System.Single.MaxValue)

        let lowerL, upperR = 
            Array.fold 
                (fun (acc:Vector3*Vector3) (elem:Vector3) ->
                    let lowerL, upperR = fst acc, snd acc
                    let lowX = if elem.X < lowerL.X then elem.X else lowerL.X
                    let lowY = if elem.Y < lowerL.Y then elem.Y else lowerL.Y
                    let lowZ = if elem.Z < lowerL.Z then elem.Z else lowerL.Z

                    let upX = if elem.X > upperR.X then elem.X else upperR.X
                    let upY = if elem.Y > upperR.Y then elem.Y else upperR.Y
                    let upZ = if elem.Z > upperR.Z then elem.Z else upperR.Z
                    (Vector3(lowX,lowY,lowZ), Vector3(upX,upY,upZ))) (lowerL,upperR) mesh.Positions

        let center = Vector3.Multiply(Vector3.Add(lowerL,upperR), 0.5f)
        lowerL,upperR,center

    /// Returns the total vertex size (in bytes), using the specified declaration
    /// list.
    let getVertSize (elements:SDXVertexElement list) =
        // find the element with the highest offset
        let hElement = elements |> List.maxBy (fun el -> el.Offset)
        // figure out how big its field is 
        let sizeBytes = 
            match hElement.Type with
            | SDXDT.Float1 -> 4
            | SDXDT.Float2 -> 8
            | SDXDT.Float3 -> 12
            | SDXDT.Float4 -> 16
            | SDXDT.Short4 -> 8
            | SDXDT.Short2 -> 4
            | SDXDT.UByte4N -> 4
            | SDXDT.Ubyte4 -> 4
            | SDXDT.Color -> 4
            | SDXDT.HalfTwo -> 4
            | _ -> failwithf "Some lazy person didn't fill in the size of type %A" hElement.Type
        int hElement.Offset + sizeBytes

    /// Returns true if the declaration list contains blend data, false otherwise.
    /// Note, both indices and weights must be present for this to return true.
    let hasBlendElements (elements:SDXVertexElement list) =
        let found = elements |> List.tryFind (fun el -> 
            match el.Usage with 
            | SDXVertexDeclUsage.BlendIndices 
            | SDXVertexDeclUsage.BlendWeight -> true
            | _ -> false
        )
        found <> None

    /// Read a mesh file, determining the file type by extension.  Currently
    /// only obj/mmobj are supported.
    let readFrom(filename,modType,flags) =
        let ext = Path.GetExtension(filename).ToLower()
        let readFn = 
            match ext with 
            | ".obj" -> readObj
            | ".mmobj" -> readObj
            | _ -> failwithf "Don't know how to read file type: %s" ext
        let md = readFn(filename,modType,flags)
        md

    /// Write a mesh file, determining the file type by extension.  Currently
    /// only obj/mmobj are supported.  The output is always in mmobj format, 
    /// since mmobj is backwards compatible with obj.
    let writeTo(filename,mesh:Mesh) = 
        let ext = Path.GetExtension(filename).ToLower()
        let writeFn = 
            match ext with
            | ".obj" -> writeObj
            | ".mmobj" -> writeObj
            | _ -> failwithf "Don't know how to write file type: %s" ext
        writeFn mesh filename
    
    /// Apply the specified position transformation func.  Calling code should ensure
    /// that the function name is stored in the AppliedPositionTransforms of the mesh.
    let applyPositionTransformation func (mesh:Mesh) =
        let newPositions = mesh.Positions |> Array.map func
        { mesh with Positions = newPositions }

    /// Apply the specified normal transformation func.  Calling code should ensure
    /// that the function name is stored in the AppliedPositionTransforms of the mesh.
    let applyNormalTransformation func (mesh:Mesh) =
        let newNormals = mesh.Normals |> Array.map func
        { mesh with Normals = newNormals }

    /// Apply the specified uv transformation func.  Calling code should ensure
    /// that the function name is stored in the AppliedUVTransforms of the mesh.
    let applyUVTransformation func (mesh:Mesh) = 
        let newUVs = mesh.UVs |> Array.map func
        { mesh with UVs = newUVs }
