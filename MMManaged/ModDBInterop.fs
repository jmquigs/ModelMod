namespace ModelMod

open System.IO

open Microsoft.Xna.Framework

open CoreTypes
open InteropTypes

module ModDBInterop =
    let private log = Logging.GetLogger("ModDBInterop")

    let SetPaths (mmDllPath:string) (exeModule:string) =
        let ret = {
            InputProfile = CoreTypes.DefaultRunConfig.InputProfile
            RunModeFull = CoreTypes.DefaultRunConfig.RunModeFull
        }
        try
            // check for valid paths
            if mmDllPath.Contains("..") then failwith "Illegal dll path, contains '..' : %A" mmDllPath
            if exeModule.Contains("..") then failwith "Illegal exe module, contains '..' : %A" exeModule

            // set the root path to the parent of the native ModelMod.dll.  
            State.RootDir <- Directory.GetParent(mmDllPath).ToString()
            State.ExeModule <- exeModule

            let conf = RegConfig.Load exeModule
            let conf = State.ValidateAndSetConf conf 

            let ret = 
                { ret with 
                    InputProfile = conf.InputProfile
                    RunModeFull = conf.RunModeFull
                }
            //log.Info "Returning %A" ret
            ret
        with 
        | e -> 
            log.Error "%A" e
            ret

    let GetDataPath() = 
        try
            State.getBaseDataDir()
        with 
        | e -> 
            log.Error "%A" e
            null

    let LoadFromDataPath () =
        try
            let exeDataDir = State.getExeDataDir()
            log.Info "Loading from path: %A" exeDataDir

            if not (Directory.Exists(exeDataDir)) then
                failwithf "Cannot load data, dir does not exist: %A" exeDataDir
            
            // look for ModIndex file
            let modIndexPath = Path.Combine(exeDataDir,"ModIndex.yaml")
            if not (File.Exists(modIndexPath)) then
                failwithf "Cannot load data, index file does not exist: %A" modIndexPath

            let conf = {
                MMView.Conf.ModIndexFile = Some modIndexPath
                MMView.Conf.FilesToLoad = []
                MMView.Conf.AppSettings = None
            }

            State.Moddb <- ModDB.LoadModDB conf

            Util.reportMemoryUsage()
            0
        with
        | e -> 
            log.Error "%A" e
            InteropTypes.GenericFailureCode

    let GetModCount() = State.Moddb.MeshRelations.Length + State.Moddb.DeletionMods.Length

    let ModTypeToInt modType = 
        match modType with
        | CPUReplacement -> 2
        | GPUReplacement -> 3
        | Deletion -> 5
        | Reference -> failwith "A mod has type set to reference"

    let private getMeshRelationMod i = 
        let moddb = State.Moddb
        let meshrel = List.nth (moddb.MeshRelations) i
        let refm = meshrel.RefMesh
        let modm = meshrel.ModMesh

        let declElements,declSize = 
            match meshrel.GetVertDeclaration() with
            | None -> failwith "A vertex declaration must be set here, native code requires it."
            | Some (data,elements) -> elements,data.Length

        let vertSize = MeshUtil.GetVertSize declElements
                
        let modType = ModTypeToInt modm.Type

        let primType = 4 //D3DPT_TRIANGLELIST
        let vertCount = modm.Positions.Length
        let primCount = modm.Triangles.Length
        let indexCount = 0
        let refVertCount = refm.Positions.Length
        let refPrimCount = refm.Triangles.Length
        let declSizeBytes = declSize
        let vertSizeBytes = vertSize
        let indexElemSizeBytes = 0

        { 
            InteropTypes.ModData.modType = modType
            primType = primType
            vertCount = vertCount
            primCount = primCount
            indexCount = indexCount
            refVertCount = refVertCount
            refPrimCount = refPrimCount
            declSizeBytes = declSizeBytes
            vertSizeBytes = vertSizeBytes
            indexElemSizeBytes = indexElemSizeBytes
            tex0Path = modm.Tex0Path
            tex1Path = modm.Tex1Path
            tex2Path = modm.Tex2Path
            tex3Path = modm.Tex3Path
        }
       
    let GetModData(i) = 
        // emptyMod is used for error return cases.  Doing this allows us to keep the ModData as an F# record,
        // which does not allow null.  Can't use option type here because native code calls this.
        let emptyMod = InteropTypes.EmptyModData

        try
            let moddb = State.Moddb

            let maxMods = GetModCount()

            let ret = 
                match i with 
                | n when n >= maxMods ->
                    log.Error "Mod index out of range: %d" i
                    emptyMod
                | n when n < 0 ->
                    log.Error "Mod index out of range: %d" i
                    emptyMod
                | n when n < moddb.MeshRelations.Length ->
                    getMeshRelationMod n
                | n when n >= moddb.MeshRelations.Length ->
                    let delIdx = (n - moddb.MeshRelations.Length)
                    List.nth moddb.DeletionMods delIdx 
                | n -> failwith "invalid mod index: %A" i

            //log.Info "Returning mod %A for index %A" ret i
            ret
        with
            | e -> 
                log.Error "%s" e.Message
                log.Error "%s" e.StackTrace
                emptyMod

    let private getBinaryWriter (p:nativeptr<byte>) size = 
        let bw = 
            if size > 0 
            then
                let stream = new UnmanagedMemoryStream(p, int64 size, int64 size, FileAccess.Write)
                let bw = new BinaryWriter(stream)
                bw
            else
                // write to /dev/null: mostly useless, but allows this method to always return a valid writer
                let stream = new System.IO.MemoryStream()
                let bw = new BinaryWriter(stream)
                bw
        bw

    let write4ByteVector (v:Vector3) (bw:BinaryWriter) =
        let x = uint8 (v.X * 128.f + 127.f)
        let y = uint8 (v.Y * 128.f + 127.f)
        let z = uint8 (v.Z * 128.f + 127.f)
        let w = uint8 0.f

        //if debugLogEnabled() then debugLog (sprintf "computed vec: %A %A %A %A from %A" x y z w v)

        // write in reverse order (is this valid for all games?)
        bw.Write(z)
        bw.Write(y)
        bw.Write(x)
        bw.Write(w)

    let writeF3Vector (v:Vector3) (bw:BinaryWriter) =
        bw.Write(v.X)
        bw.Write(v.Y)
        bw.Write(v.Z)        

    module DataWriters =
        let rbBlendIndex (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Ubyte4 ->
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Usage)
                let idx = br.ReadBytes(4)
                bw.Write(idx) 
            | _ -> failwithf "Unsupported type for raw blend index: %A" el.Type
        let rbBlendWeight (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.UByte4N -> 
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Usage)
                bw.Write(br.ReadBytes(4))
            | _ -> failwithf "Unsupported type for raw blend weight: %A" el.Type

        let private writeMeshBI (mesh:Mesh) (idx:int) (bw:BinaryWriter) =
            if (idx > mesh.BlendIndices.Length) then
                failwithf "oops: invalid blend-index index: %A of %A" idx mesh.BlendIndices.Length
            let bi = mesh.BlendIndices.[idx]
            let buf = [| byte (bi.X :?> int32); byte (bi.Y :?> int32) ; byte (bi.Z :?> int32); byte (bi.W :?> int32) |]
            bw.Write(buf)            

        let private round (x:float32) = System.Math.Round (float x)
        let private writeMeshBW (mesh:Mesh) (idx:int) (bw:BinaryWriter) =
            if (idx > mesh.BlendWeights.Length) then
                failwithf "oops: invalid blend-weight index: %A of %A" idx mesh.BlendWeights.Length
            let wgt = mesh.BlendWeights.[idx]

            let buf = [| byte (round(wgt.X * 255.f)); byte (round (wgt.Y * 255.f)) ; byte (round (wgt.Z * 255.f)); byte (round (wgt.W * 255.f)) |]
            // weights must sum to 255
//            let sum = Array.sum buf
//            if (sum <> (byte 255)) then
//                log.Error "weights do not sum to 255 for idx %A: %A, %A; basevec: %A" idx buf sum wgt
            bw.Write(buf)

        let private writeMeshBWF4 (mesh:Mesh) (idx:int) (bw:BinaryWriter) =
            if (idx > mesh.BlendWeights.Length) then
                failwithf "oops: invalid blend-weight index: %A of %A" idx mesh.BlendWeights.Length
            let wgt = mesh.BlendWeights.[idx]
            bw.Write(wgt.X)
            bw.Write(wgt.Y)
            bw.Write(wgt.Z)
            bw.Write(wgt.W)

        let modmBlendIndex (vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Ubyte4 -> writeMeshBI modm modVertIndex bw
            | _ -> failwithf "Unsupported type for mod blend index: %A" el.Type

        let refmBlendIndex (vertRels:MeshRelation.VertRel[]) (refm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 ->
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBI refm refVertIndex bw
            | _ -> failwithf "Unsupported type for ref blend index: %A" el.Type

        let modmBlendWeight (vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.UByte4N -> writeMeshBW modm modVertIndex bw
            | _ -> failwithf "Unsupported type for mod blend weight: %A" el.Type

        let refmBlendWeight (vertRels:MeshRelation.VertRel[]) (refm:Mesh) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.UByte4N -> 
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBW refm refVertIndex bw
            | SDXVertexDeclType.Float4 ->
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBWF4 refm refVertIndex bw
            | _ -> failwithf "Unsupported type for ref blend weight: %A" el.Type

        let modmNormal (modm:Mesh) (modNrmIndex: int) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 ->
                // convert normal to 4 byte rep
                let srcNrm = modm.Normals.[modNrmIndex]
                write4ByteVector srcNrm bw
            | SDXVertexDeclType.Float3 ->
                let srcNrm = modm.Normals.[modNrmIndex]
                writeF3Vector srcNrm bw
            | _ -> failwithf "Unsupported type for mod normal: %A" el.Type

        let rbNormal (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (_: int) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 ->
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Usage)
                bw.Write(br.ReadBytes(4))                
            | _ -> failwithf "Unsupported type for raw normal: %A" el.Type

        let modmBinormalTangent (modm:Mesh) (modNrmIndex: int) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            // This isn't the most accurate way to compute these, but its easier than the mathematically correct method, which
            // requires inspecting the triangle and uv coordinates.  Its probably worth implementing that at some point, 
            // but this produces good enough results in most cases.
            // see: http://www.geeks3d.com/20130122/normal-mapping-without-precomputed-tangent-space-vectors/
            let srcNrm = modm.Normals.[modNrmIndex]
            let v1 = Vector3.Cross(srcNrm, Vector3(0.f, 0.f, 1.f))
            let v2 = Vector3.Cross(srcNrm, Vector3(0.f, 1.f, 0.f))
            let t = if (v1.Length() > v2.Length()) then v1 else v2
            t.Normalize()
            let vec = 
                if (el.Usage = SDXVertexDeclUsage.Binormal) then
                    let b = Vector3.Cross(srcNrm,t)
                    b.Normalize()
                    b
                else
                    t
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 ->
                write4ByteVector vec bw  
            | SDXVertexDeclType.Float3 ->
                writeF3Vector vec bw
            | _ -> failwithf "Unsupported type for mod binormal/tangent: %A" el.Type

        let rbBinormalTangent (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (_: int) (modVertIndex: int) (el:SDXVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 ->             
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Usage)
                bw.Write(br.ReadBytes(4))                
            | _ -> failwithf "Unsupported type for raw binormal/tangent: %A" el.Type

    let private fillModDataInternalHelper 
        (modIndex:int) 
        (destDeclBw:BinaryWriter) (destDeclSize:int) 
        (destVbBw:BinaryWriter) (destVbSize:int) 
        (destIbBw:BinaryWriter) (destIbSize:int) =
        try
            let moddb = State.Moddb

            let md = GetModData modIndex
            if md.modType <> 3 then // TODO: maybe mod type should be an enum after all
                failwithf "unsupported mod type: %d" md.modType

            // grab more stuff that we'll need
            let meshrel = List.nth (moddb.MeshRelations) modIndex
            let refm = meshrel.RefMesh
            let modm = meshrel.ModMesh
            let vertRels = meshrel.VertRelations

            // extract vertex declaration
            let srcDeclData,declElements = 
                match meshrel.GetVertDeclaration() with
                | None -> failwith "A vertex declaration must be set here, native code requires it."
                | Some (data,elements) -> data,elements
            
            // copy declaration data to destination
            if (destDeclSize > 0) then
                if destDeclSize <> srcDeclData.Length then
                    failwith "Decl src/dest mismatch: src: %d, dest: %d" srcDeclData.Length destDeclSize

                destDeclBw.Write(srcDeclData)

            // copy index data...someday
            if (destIbSize > 0) then
                failwith "Filling index data is not yet supported"

            // copy vertex data.  this is where most of the work happens.
            if (destVbSize > 0) then
                // we aren't using an index list, so we'll fill the buffer with vertices represnting all the primitives.

                // walk the mod triangle list: for each point on each triangle, write a unique entry into the vb.
                // these bools control where data comes from (normally we don't want to change these except
                // when debugging something)

                // true: obtain blend indices and weights from the ref
                // false: obtain it from the mod.  mod author must ensure that all verts are propertly 
                // weighted in the 3d tool (PITA, usually; esp with symmetric stuff).
                let useRefBlendData = true 
                // true: obtain the blend data from the raw binary ref data.  usually don't do this, because
                // it prevents use of the annotation groups feature.
                // false: obtain the blend data from the ref mmobj.
                let rawBinaryRefMesh = false 

                // true: use normals from the mod
                // false: copy normals from nearest ref vert in raw binary data.  possibly useful for debugging but
                // will otherwise produces screwy results if the mod mesh is different enough.
                let useModNormals = true

                // true: compute the binormal and tangent from the mod normal
                // false: copy bin/tan from the nearest ref in raw binary data.  mostly for debug.
                let computeBinormalTangent = true
                                                
                let srcVbSize = md.primCount * 3 * md.vertSizeBytes
                if (destVbSize <> srcVbSize) then
                    failwith "Decl src/dest mismatch: src: %d, dest: %d" srcVbSize destVbSize
   
                let bw = destVbBw
                // sort vertex elements in offset order ascending, so that we don't have to reposition the memory stream
                // as we go
                let declElements = declElements |> List.sortBy (fun el -> el.Offset)

                let srcPositions = modm.Positions
                let srcTex = modm.UVs

                // log some of the vectors we compute, but don't log too much because then loading will be slooooow...
//                let debugLogVectors = false
//                let maxLog = 20
//
//                let loggedVectorsCount = ref 0
//                let debugLogEnabled() = debugLogVectors && loggedVectorsCount.Value < maxLog
//                let debugLog (s:string) = 
//                    log.Info "%A" s
//                    incr loggedVectorsCount

                let useRefBlendData,rawBinaryRefMesh = 
                    let needsBlend = MeshUtil.HasBlendElements declElements
                    match needsBlend,useRefBlendData,rawBinaryRefMesh with
                    | true,false,_ -> // using mod data, component data must be present
                        if (modm.BlendIndices.Length = 0 || modm.BlendWeights.Length = 0) then
                            log.Warn "Mod mesh does not have blend indices and/or blend weights, must use ref for blend data"
                            true,true
                        else
                            useRefBlendData,rawBinaryRefMesh
                    | true,true,false -> // using component data from ref
                        if (refm.BlendIndices.Length = 0 || refm.BlendWeights.Length = 0) then
                            log.Warn "Ref mesh does not have blend indices and/or blend weights, must use binary ref data"
                            true,true
                        else
                            useRefBlendData,rawBinaryRefMesh
                    | _,_,_ -> useRefBlendData,rawBinaryRefMesh

                let refBinDataLookup = 
                    match refm.BinaryVertexData with
                    | None -> None
                    | Some bvd -> Some (new ModDB.BinaryLookupHelper(bvd,declElements))
                    
                let blendIndexWriter,blendWeightWriter = 
                    if useRefBlendData then
                        if rawBinaryRefMesh then 
                            let binDataLookup = 
                                match refBinDataLookup with
                                | None -> failwith "Binary vertex data is required to write blend index,blend weight" 
                                | Some bvd -> bvd

                            let biw = DataWriters.rbBlendIndex binDataLookup vertRels modm
                            let bww = DataWriters.rbBlendWeight binDataLookup vertRels modm
                            
                            biw,bww
                        else
                            let biw = DataWriters.refmBlendIndex vertRels refm
                            let bww = DataWriters.refmBlendWeight vertRels refm
                            
                            biw,bww 
                    else
                        let biw = DataWriters.modmBlendIndex vertRels modm                        
                        let bww = DataWriters.modmBlendWeight vertRels modm
                        biw,bww

                let normalWriter = 
                    if useModNormals then
                        let nrmw = DataWriters.modmNormal modm
                        nrmw
                    else
                        let binDataLookup = 
                            match refBinDataLookup with
                            | None -> failwith "Binary vertex data is required to write normal" 
                            | Some bvd -> bvd
                        let nrmw = DataWriters.rbNormal binDataLookup vertRels
                        nrmw

                let binormalTangentWriter =
                    if computeBinormalTangent then
                        DataWriters.modmBinormalTangent modm
                    else
                        let binDataLookup = 
                            match refBinDataLookup with
                            | None -> failwith "Binary vertex data is required to write binormal" 
                            | Some bvd -> bvd
                        DataWriters.rbBinormalTangent binDataLookup vertRels                    

                let writeElement (v:PTNIndex) (el:SDXVertexElement) =
                    let modVertIndex = v.Pos
                    let modNrmIndex = v.Nrm

                    match el.Usage with
                        | SDXVertexDeclUsage.Position ->
                            match el.Type with 
                            | SDXVertexDeclType.Unused -> ()
                            | SDXVertexDeclType.Float3 -> 
                                let srcPos = srcPositions.[modVertIndex]
                                bw.Write(srcPos.X)
                                bw.Write(srcPos.Y)
                                bw.Write(srcPos.Z)
                            | _ -> failwithf "Unsupported type for position: %A" el.Type
                        | SDXVertexDeclUsage.TextureCoordinate ->
                            match el.Type with
                            | SDXVertexDeclType.Float2 -> 
                                let srcTC = srcTex.[v.Tex]
                                bw.Write(srcTC.X)
                                bw.Write(srcTC.Y)
                            | SDXVertexDeclType.HalfTwo ->
                                let srcTC = srcTex.[v.Tex]
                                bw.Write(MonoGameHelpers.floatToHalfUint16 srcTC.X)
                                bw.Write(MonoGameHelpers.floatToHalfUint16 srcTC.Y)
                            | _ -> failwithf "Unsupported type for texture coordinate: %A" el.Type
                        | SDXVertexDeclUsage.Normal -> normalWriter modNrmIndex modVertIndex el bw
                        | SDXVertexDeclUsage.Binormal 
                        | SDXVertexDeclUsage.Tangent -> binormalTangentWriter modNrmIndex modVertIndex el bw
                        | SDXVertexDeclUsage.BlendIndices -> blendIndexWriter modVertIndex el bw
                        | SDXVertexDeclUsage.BlendWeight -> blendWeightWriter modVertIndex el bw
                        | SDXVertexDeclUsage.Color ->
                            // TODO: if/when snapshot & import/export write this out, will need to populate it here
                            match el.Type with
                            | SDXVertexDeclType.Float4 ->
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                            | _ -> failwith "Unsupported type for Color: %A" el.Type
                        | _ -> failwithf "Unsupported usage: %A" el.Usage

                let writeVertex (v:PTNIndex) = 
                    let writeToVert = writeElement v
                    declElements |> List.iter writeToVert
                    if (bw.BaseStream.Position % (int64 md.vertSizeBytes) <> 0L) then
                        failwith "Wrote an insufficient number of bytes for the vertex"

                let writeTriangle (tri:IndexedTri) = tri.Verts |> Array.iter writeVertex

                modm.Triangles |> Array.iter writeTriangle
            0
        with
            | e -> 
                log.Error "%s" e.Message
                log.Error "%s" e.StackTrace
                InteropTypes.GenericFailureCode

    let FillModData 
        (modIndex:int) 
        (destDeclData:nativeptr<byte>) (destDeclSize:int) 
        (destVbData:nativeptr<byte>) (destVbSize:int) 
        (destIbData:nativeptr<byte>) (destIbSize:int) =
            fillModDataInternalHelper 
                modIndex 
                (getBinaryWriter destDeclData destDeclSize) destDeclSize 
                (getBinaryWriter destVbData destVbSize) destVbSize
                (getBinaryWriter destIbData destIbSize) destIbSize

    let TestFill (modIndex:int,destDecl:byte[],destVB:byte[],destIB:byte[]) = 
        fillModDataInternalHelper 
            modIndex
            (new BinaryWriter(new MemoryStream(destDecl))) destDecl.Length
            (new BinaryWriter(new MemoryStream(destVB))) destVB.Length
            (new BinaryWriter(new MemoryStream(destIB))) destIB.Length       




