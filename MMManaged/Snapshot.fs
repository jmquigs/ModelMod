namespace ModelMod

open System.IO
open System.Runtime.InteropServices

open SharpDX.Direct3D9 

open CoreTypes
     
module Extractors = 
    type SourceReader = BinaryReader

    let byteToFloat (b:byte) = float32 b / 255.f

    let xPosFromFloat3 (br:SourceReader) = br.ReadSingle(), br.ReadSingle(), br.ReadSingle()
    let xTexFromFloat2 (br:SourceReader) = br.ReadSingle(), br.ReadSingle()
    let xTexFromHalfFloat2 (br:SourceReader) = MonoGameHelpers.halfUint16ToFloat(br.ReadUInt16()), MonoGameHelpers.halfUint16ToFloat(br.ReadUInt16())
    let xNrmFromFloat3 (br:SourceReader) = br.ReadSingle(), br.ReadSingle(), br.ReadSingle()
    let xNrmFromUbyte4 (br:SourceReader) = 
        // not sure if all 4 byte normals will be encoded the same way...will warn about these
        let x,y,z,_ = byteToFloat (br.ReadByte()), byteToFloat (br.ReadByte()), byteToFloat (br.ReadByte()), br.ReadByte()
        x,y,z
    let xBlendIndexFromUbyte4 (br:SourceReader) = 
        let a,b,c,d = int (br.ReadByte()), int (br.ReadByte()), int (br.ReadByte()), int (br.ReadByte())
        a,b,c,d
    let xBlendWeightFromUbyte4 (br:SourceReader) =
        let a,b,c,d = byteToFloat (br.ReadByte()), byteToFloat (br.ReadByte()), byteToFloat (br.ReadByte()), byteToFloat (br.ReadByte())
        a,b,c,d

    let xBlendWeightFromFloat4 (br:SourceReader) =
        let a,b,c,d = br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle()
        a,b,c,d

module SnapshotTransforms = 
    let Position = 
        Map.ofList [ 
            // Transforms are specified as strings; this lets them be written out during the snapshot.  These transforms
            // need to be undone on model load; prior to establishing the mesh relation.  It is assumed that the .mmobj file will preserve this
            // list, which means the exporter/import needs to pass them through appropriately.
            SnapshotProfiles.Profile1, ["rot x 90"; "rot y 180"; "scale 0.1"] 
            SnapshotProfiles.Profile2, ["rot x 90"; "rot z 180"; "scale 0.1"] 
        ]
    let UV =
        Map.ofList [
            SnapshotProfiles.Profile1, ["flip y"]
            SnapshotProfiles.Profile2, ["flip y"]
        ]

module private SSInterop =
    [< DllImport("ModelMod.dll") >]
    extern void SaveTexture(int index, [<MarshalAs(UnmanagedType.LPWStr)>]string filepath)

module Snapshot =

    let log = Logging.GetLogger("Snapshot")

    let snapshotNum = ref 0

    // for use with Snapshot.readElement
    type ReadOutputFunctions = {
        Pos: float32 * float32 * float32 -> unit
        Normal: float32 * float32 * float32 -> unit
        Binormal: float32 * float32 * float32 -> unit
        Tangent: float32 * float32 * float32 -> unit
        TexCoord: float32 * float32 -> unit
        BlendIndex: int32 * int32 * int32 * int32 -> unit
        BlendWeight: float32 * float32 * float32 * float32 -> unit
    }

    let readElement (fns:ReadOutputFunctions) reader (el:SDXVertexElement) =
        let handleVector name outputFn = 
            match el.Type with
            | SDXVertexDeclType.Float3 -> 
                outputFn (Extractors.xNrmFromFloat3 reader)
            | SDXVertexDeclType.Color 
            | SDXVertexDeclType.Ubyte4 -> 
                outputFn (Extractors.xNrmFromUbyte4 reader)
            | _ -> failwithf "Unsupported type for %s: %A" name el.Type        
                
        match el.Usage with
            | SDXVertexDeclUsage.Position ->
                match el.Type with 
                | SDXVertexDeclType.Unused -> ()
                | SDXVertexDeclType.Float3 -> 
                    fns.Pos (Extractors.xPosFromFloat3 reader)
                | _ -> failwithf "Unsupported type for position: %A" el.Type
            | SDXVertexDeclUsage.TextureCoordinate ->
                match el.Type with
                | SDXVertexDeclType.Float2 -> 
                    fns.TexCoord (Extractors.xTexFromFloat2 reader)
                | SDXVertexDeclType.HalfTwo ->
                    fns.TexCoord (Extractors.xTexFromHalfFloat2 reader)
                | _ -> failwithf "Unsupported type for texture coordinate: %A" el.Type
            | SDXVertexDeclUsage.Normal -> handleVector "normal" fns.Normal
            | SDXVertexDeclUsage.Binormal  handleVector "binormal" fns.Binormal
            | SDXVertexDeclUsage.Tangent -> handleVector "tangent" fns.Tangent
            | SDXVertexDeclUsage.BlendIndices ->
                match el.Type with
                | SDXVertexDeclType.Color ->
                    // TODO: not sure if its valid to use the ubyte4 extractor; byte size is same but format may be different
                    fns.BlendIndex (Extractors.xBlendIndexFromUbyte4 reader)
                | SDXVertexDeclType.Ubyte4 ->
                    fns.BlendIndex (Extractors.xBlendIndexFromUbyte4 reader)
                | _ -> failwithf "Unsupported type for blend index: %A" el.Type
            | SDXVertexDeclUsage.BlendWeight ->
                match el.Type with
                | SDXVertexDeclType.Color 
                | SDXVertexDeclType.UByte4N -> 
                    fns.BlendWeight (Extractors.xBlendWeightFromUbyte4 reader)
                | SDXVertexDeclType.Float4 ->
                    fns.BlendWeight (Extractors.xBlendWeightFromFloat4 reader)
                | _ -> failwithf "Unsupported type for blend weight: %A" el.Type
            | SDXVertexDeclUsage.Color ->
                match el.Type with
                | SDXVertexDeclType.Float4 ->
                    // TODO: currently ignored, but should probably keep this as baggage.
                    reader.ReadSingle() |> ignore
                    reader.ReadSingle() |> ignore
                    reader.ReadSingle() |> ignore
                    reader.ReadSingle() |> ignore
                | _ -> failwithf "Unsupported type for color: %A" el.Type
                ()
            | _ -> failwithf "Unsupported usage: %A" el.Usage

    let makeLoggedDisposable (disp:System.IDisposable) (message:string) = 
        { new System.IDisposable with member x.Dispose() = log.Info "%s" message; disp.Dispose() }

    let Take (device: nativeint) (sd:InteropTypes.SnapshotData) =
        try 
            incr snapshotNum

            log.Info "Snapshot: number %d" snapshotNum.Value
            log.Info "  Capturing %d primitives composed of %d vertices with primitive type %d" sd.primCount sd.numVertices sd.primType
            log.Info "  MinVertexIndex: %d, BaseVertexIndex: %d, StartIndex: %d" sd.minVertexIndex sd.baseVertexIndex sd.startIndex

            if sd.baseVertexIndex <> 0 || sd.minVertexIndex <> 0u then
                // need a test case for these
                log.Warn "One or more of baseVertexIndex, minVertexIndex is not zero.  Snapshot may not handle this case"

            // check primitive type
            let primType = enum<PrimitiveType>(sd.primType)
            if primType <> PrimitiveType.TriangleList then failwith "Cannot snap primitives of type: %A; only triangle lists are supported" primType

            // create the device from the native pointer.
            // note: creating a new sharpdx wrapper object from a native pointer does not increase the com ref count.
            // however, disposing that object will decrease the ref count, which can lead to a crash.  Therefore,
            // we must only dispose objects that are allocated from scratch or via a d3d device call, such as 
            // GetStreamSource below.
            let device = new Device(device)

            // get active stream information for stream 0.  currently we ignore other streams (will log a warning below if the declaration 
            // uses data from non-stream 0).
            let mutable vb:VertexBuffer = null
            let mutable offsetBytes = 0
            let mutable strideBytes = 0

            device.GetStreamSource(0,&vb,&offsetBytes,&strideBytes)
            if (vb = null) then failwith "Stream 0 VB is null, cannot snap"

            // need to dispose the vb
            use dVB = makeLoggedDisposable vb "disposing stream 0 vb"
            
            log.Info "Stream 0: offset: %d, stride: %d" offsetBytes strideBytes

            // check the divider
            let mutable divider = 0
            device.GetStreamSourceFrequency(0, &divider)
            if divider <> 1 then failwith "Divider must be 1" // this code doesn't handle other cases right now

            // index buffer
            if sd.ib = 0n then failwith "Index buffer is null"
            let ib = new IndexBuffer(sd.ib) // do not dispose, native code owns it
            let ibDesc = ib.Description
            log.Info"IndexBuffer: Format: %A, Usage: %A, Pool: %A, Size: %d" ibDesc.Format ibDesc.Usage ibDesc.Pool ibDesc.Size

            // check format
            if ibDesc.Format <> Format.Index16 then failwith "Cannot snap indices of type: %A; only index16 are supported" ibDesc.Format

            // vertex declaration
            if sd.vertDecl = 0n then failwith "Vertex declaration is null"
            let decl = new VertexDeclaration(sd.vertDecl) // do not dispose, native code owns it

            let elements = decl.Elements 
            log.Info "Declaration: %d elements" elements.Length
            for el in elements do
                log.Info "   Stream: %d, Offset: %d, Type: %s, Usage: %s, UsageIndex: %d, Method: %s"
                    el.Stream el.Offset (el.Type.ToString()) (el.Usage.ToString()) el.UsageIndex (el.Method.ToString()) 
                // warn if stream > 0 is used
                if el.Stream <> 255s && el.Stream > 0s then
                    log.Warn "Stream %d is not supported" el.Stream
                if el.Usage = SDXVertexDeclUsage.Color then
                    log.Warn "Vertex uses color usage; this data is currently ignored"

            // store raw vertex elements in byte array
            let declMS = new MemoryStream()
            let declWriter = new BinaryWriter(declMS)
            elements |> Array.iter (fun el -> ModDB.writeVertexElement el declWriter)
            declWriter.Close()
            let declBytes = declMS.ToArray()

            // lock vb and ib
            let vbDS = vb.Lock(0, vb.Description.SizeInBytes, LockFlags.ReadOnly)
            if not vbDS.CanRead then failwith "Failed to lock vertex buffer for reading"
            use vbReader = new BinaryReader(vbDS) // disposable

            let ibDS = ib.Lock(0, ib.Description.Size, LockFlags.ReadOnly) 
            if not ibDS.CanRead then failwith "Failed to lock index buffer for reading"
            use ibReader = new BinaryReader(ibDS) // disposable

            // sort elements ascending by offset to avoid seeking the reader
            let declElements = decl.Elements |> List.ofArray |> List.sortBy (fun el -> el.Offset)

            // create arrays for storage
            let positions = new ResizeArray<Vec3F>()
            let normals = new ResizeArray<Vec3F>()
            let uvs = new ResizeArray<Vec2F>()
            let blendIndices = new ResizeArray<Vec4X>()
            let blendWeights = new ResizeArray<Vec4F>()

            // create visitor functions to be used with readElement 
            let readOutputFns = {
                ReadOutputFunctions.Pos = (fun (x,y,z) -> positions.Add(Vec3F(x,y,z)) )
                Normal = (fun (x,y,z) -> normals.Add(Vec3F(x,y,z)))
                TexCoord = (fun (u,v) -> uvs.Add(Vec2F(u,v)))
                Binormal = (fun (x,y,z) -> () ) // currently ignored
                Tangent = (fun (x,y,z) -> () ) // currently ignored
                BlendIndex = (fun (a, b, c, d) -> blendIndices.Add(Vec4X(a,b,c,d)))
                BlendWeight = (fun (a, b, c, d) -> blendWeights.Add(Vec4F(a,b,c,d)))
            }

            // create per-element read function bound to the reader
            let readVertElement = readElement readOutputFns vbReader

            // start at minIndex and write out numVerts (we only write the verts used by the DIP call)
            let vbStartOffset = int64 offsetBytes + ((int64 sd.baseVertexIndex + int64 sd.minVertexIndex) * int64 strideBytes)
            ignore (vbDS.Seek(vbStartOffset, SeekOrigin.Begin) )
            // walk the verts to populate data arrays.
            // elements are sorted in offset order, so we only need to seek the reader between verts (not between elements)
            // we do assume that each extractor reads the full 
            // amount of data for its type (for example a ubyte4 extractor should read 4 bytes even if the 4th is ignored)
            let stride = strideBytes
            let processVert i = 
                ignore (vbDS.Seek(vbStartOffset + (int64 i * int64 stride),SeekOrigin.Begin))
                declElements |> List.iter readVertElement
            [0..(int sd.numVertices-1)] |> List.iter processVert

            // now write the index (primitive) data
            // since we only wrote out the potentially-usable verts, and not the full buffer, we have to offset each index by
            // MinVertexIndex, since that is the lowest possible index that we can use 
            // TODO: need to test this with something that has minVertexIndex != 0; may need to include some code to use that in processTriangle
            let indexElemSize = 2 // 2 = sizeof short (Format.Index16)
            let ibStartOffset = int64 sd.minVertexIndex * (int64 indexElemSize) + int64 (sd.startIndex * uint32 indexElemSize)
            ignore (ibDS.Seek(ibStartOffset, SeekOrigin.Begin))

            let triangles = new ResizeArray<IndexedTri>()
            
            let processTriangle _ = 
                let a = int (ibReader.ReadInt16()) 
                let b = int (ibReader.ReadInt16())
                let c = int (ibReader.ReadInt16())

                // since vert,normal,texture arrays are all the same size, use the same index for each.
                let verts:PTNIndex[] = Array.zeroCreate 3
                verts.[0] <- { Pos = a; Tex = a; Nrm = a }
                verts.[1] <- { Pos = b; Tex = b; Nrm = b }
                verts.[2] <- { Pos = c; Tex = c; Nrm = c }
                triangles.Add({ Verts = verts})

            [1..(int sd.primCount)] |> List.iter processTriangle

            // set up to write files
            let baseDir = State.getExeSnapshotDir()

            if not (Directory.Exists baseDir) then
                Directory.CreateDirectory(baseDir) |> ignore

            let sbasename = sprintf "snap_%d_%dp_%dv" snapshotNum.Value sd.primCount sd.numVertices

            // write textures for enabled stages only
            let maxStage = 7 // because, uh, its a lucky number
            let texturePaths = 
                [0..maxStage] 
                |> List.filter (fun i -> 
                    let state = device.GetTextureStageState(i, TextureStage.ColorOperation)
                    state <> 1) // 1 = D3DTOP_DISABLE
                |> List.map (fun i ->
                    let texName = sprintf "%s_texture%d.dds" sbasename i
                    let texPath = Path.Combine(baseDir, texName)
                    SSInterop.SaveTexture(i, texPath)
                    texName,texPath)

            // get list of applied transforms, if enabled
            let doTransforms = true

            let lookupTransforms map =
                if doTransforms then
                    let profileKey = State.Conf.SnapshotProfile

                    let xforms = map |> Map.tryFind profileKey
                    match xforms with 
                    | None -> 
                        log.Warn "No transforms found for profile: %A" profileKey
                        []
                    | Some xforms -> 
                        log.Info "applying transforms: %A" xforms
                        xforms
                else
                    []

            let appliedPosTransforms = lookupTransforms SnapshotTransforms.Position
            let appliedUVTransforms = lookupTransforms SnapshotTransforms.UV

            // use the first texture (if available) as the mesh material
            let matPath = 
                match texturePaths with
                | [] -> ""
                | (x::xs) -> fst x

            let mesh = { 
                Mesh.Type = Reference
                Triangles = triangles.ToArray()
                Positions = positions.ToArray()
                UVs = uvs.ToArray()
                Normals = normals.ToArray()
                BlendIndices = blendIndices.ToArray()
                BlendWeights = blendWeights.ToArray()
                Declaration = None
                BinaryVertexData = None
                AppliedPositionTransforms = Array.ofList appliedPosTransforms
                AppliedUVTransforms = Array.ofList appliedUVTransforms
                Tex0Path = matPath
                Tex1Path = ""
                Tex2Path = ""
                Tex3Path = ""
                AnnotatedVertexGroups = [||]
            }

            // apply tranforms
            let mesh = MeshTransform.applyMeshTransforms appliedPosTransforms appliedUVTransforms mesh

            // write mesh
            let meshfile = sprintf "%s.mmobj" sbasename 
            let meshfile = Path.Combine(baseDir,meshfile)
            MeshUtil.WriteObj mesh meshfile

            // write vert decl
            let declfile = Path.Combine(baseDir, (sprintf "%s_VBDecl.dat" sbasename))
            File.WriteAllBytes(declfile, declBytes)
            
            // write raw ib and vb; just write the portion that was used by the DIP call
            let getStreamBytes (startoffset) (datastream:SharpDX.DataStream) size =                 
                datastream.Seek(startoffset, SeekOrigin.Begin) |> ignore
                let data:byte[] = Array.zeroCreate size
                let ibBytes = datastream.Read(data,0,data.Length)
                data

            let ibBytesToRead = int sd.primCount * 3 * int indexElemSize
            getStreamBytes ibStartOffset ibDS ibBytesToRead 
                |> 
                (fun bytes -> 
                    // write header
                    let iCount = int sd.primCount * 3
                    let iSize = indexElemSize

                    let fname = Path.Combine(baseDir, (sprintf "%s_IB.dat" sbasename))
                    use bw = new BinaryWriter(new FileStream(fname, FileMode.Create))
                    bw.Write(iCount)
                    bw.Write(iSize)
                    bw.Write(bytes)
                    ())

            let vbBytesToRead = int sd.numVertices * strideBytes           
            getStreamBytes vbStartOffset vbDS vbBytesToRead 
                |> 
                (fun bytes ->
                    // write header
                    let fname = Path.Combine(baseDir, (sprintf "%s_VB.dat" sbasename))
                    use bw = new BinaryWriter(new FileStream(fname, FileMode.Create))
                    bw.Write(sd.numVertices)
                    bw.Write(stride)
                    bw.Write(bytes)
                    ())

            log.Info "Wrote snapshot %d to %s" snapshotNum.Value baseDir

            // TODO: vertex shader & constants
            // TODO: pixel shaders & constants
            
            ib.Unlock()
            vb.Unlock()

            0
        with 
            e -> 
                log.Error "%A" e
                47
