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

// Using interop makes the IL unverifiable, disable warning.
#nowarn "9"
#nowarn "51"

namespace ModelMod

open System.IO
open System.Runtime.InteropServices

type VertexBuffer9 = SharpDX.Direct3D9.VertexBuffer
type IndexBuffer9 = SharpDX.Direct3D9.IndexBuffer
type PrimitiveType9 = SharpDX.Direct3D9.PrimitiveType
type Device9 = SharpDX.Direct3D9.Device
type VertexDeclaration9 = SharpDX.Direct3D9.VertexDeclaration
type Format9 = SharpDX.Direct3D9.Format
type LockFlags9 = SharpDX.Direct3D9.LockFlags
type TextureStage9 = SharpDX.Direct3D9.TextureStage
type TransformState9 = SharpDX.Direct3D9.TransformState

type Device11 = SharpDX.Direct3D11.Device

open CoreTypes

open FSharp.Core

/// Utilities for reading types from binary vertex data.
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

/// Snapshot utilities.
module Snapshot =
    type MMVES = VertexTypes.MMVertexElemSemantic
    type MMET = VertexTypes.MMVertexElementType
    type SDXVT = SDXVertexDeclType
    type SDXF = SharpDX.DXGI.Format

    let private log = Logging.getLogger("Snapshot")

    /// Increments on each snapshot.  Note: it will get reset to zero if the assembly is reloaded, which
    /// means that snapshots can overlap (filenames include the vertex and primitive count, so usually
    /// this just results in a slightly messy directory as opposed to snapshot stomping).
    let private snapshotNum = ref 0

    let private lastBaseDir = ref ""
    let private lastBaseName = ref ""

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

    /// Reads a vertex element.  Uses the read output functions to pipe the data to an appropriate handler
    /// function, depending on the type.
    let private readElement (fns:ReadOutputFunctions) (ignoreFns:ReadOutputFunctions) reader (el:VertexTypes.MMVertexElement) =
        let fns =
            if el.SemanticIndex = 0 then
                fns
            else
                ignoreFns

        let handleVector name outputFn =
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Float3 ->
                    outputFn (Extractors.xNrmFromFloat3 reader)
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N
                | SDXVertexDeclType.Ubyte4 ->
                    outputFn (Extractors.xNrmFromUbyte4 reader)
                | _ -> failwithf "Unsupported type for %s: %A" name dt
            | MMET.Format(f) ->
                match f with
                | SDXF.R32G32B32_Float ->
                    outputFn (Extractors.xNrmFromFloat3 reader)
                | SDXF.R8G8B8A8_UInt
                | SDXF.R8G8B8A8_UNorm ->
                    outputFn (Extractors.xNrmFromUbyte4 reader)
                | _ -> failwithf "Unsupported format for %s: %A" name f

        match el.Semantic with
            | MMVES.Position ->
                match el.Type with
                | MMET.DeclType(dt) ->
                    match dt with
                    | SDXVertexDeclType.Unused -> ()
                    | SDXVertexDeclType.Float3 ->
                        fns.Pos (Extractors.xPosFromFloat3 reader)
                    | _ -> failwithf "Unsupported type for position: %A" dt
                | MMET.Format(f) ->
                    match f with
                    | SDXF.R32G32B32_Float ->
                        fns.Pos (Extractors.xPosFromFloat3 reader)
                    | _ -> failwithf "Unsupported format for position: %A" f
            | MMVES.TextureCoordinate ->
                match el.Type with
                | MMET.DeclType(dt) ->
                    match dt with
                    | SDXVertexDeclType.Float2 ->
                        fns.TexCoord (Extractors.xTexFromFloat2 reader)
                    | SDXVertexDeclType.HalfTwo ->
                        fns.TexCoord (Extractors.xTexFromHalfFloat2 reader)
                    | _ -> failwithf "Unsupported type for texture coordinate: %A" dt
                | MMET.Format(f) ->
                    match f with
                    | SDXF.R32G32_Float ->
                        fns.TexCoord (Extractors.xTexFromFloat2 reader)
                    | SDXF.R16G16_Float ->
                        fns.TexCoord (Extractors.xTexFromHalfFloat2 reader)
                    | _ -> failwithf "Unsupported format for texture coordinate: %A" f
            | MMVES.Normal -> handleVector "normal" fns.Normal
            | MMVES.Binormal -> handleVector "binormal" fns.Binormal
            | MMVES.Tangent -> handleVector "tangent" fns.Tangent
            | MMVES.BlendIndices ->
                match el.Type with
                | MMET.DeclType(dt) ->
                    match dt with
                    | SDXVertexDeclType.Color ->
                        // TODO: not sure if its valid to use the ubyte4 extractor; byte size is same but format may be different
                        fns.BlendIndex (Extractors.xBlendIndexFromUbyte4 reader)
                    | SDXVertexDeclType.Ubyte4 ->
                        fns.BlendIndex (Extractors.xBlendIndexFromUbyte4 reader)
                    | _ -> failwithf "Unsupported type for blend index: %A" dt
                | MMET.Format(f) ->
                    match f with
                    | SDXF.R8G8B8A8_UNorm
                    | SDXF.R8G8B8A8_UInt ->
                        fns.BlendIndex (Extractors.xBlendIndexFromUbyte4 reader)
                    | _ -> failwithf "Unsupported format for blend index: %A" f
            | MMVES.BlendWeight ->
                match el.Type with
                | MMET.DeclType(dt) ->
                    match dt with
                    | SDXVertexDeclType.Color
                    | SDXVertexDeclType.UByte4N ->
                        fns.BlendWeight (Extractors.xBlendWeightFromUbyte4 reader)
                    | SDXVertexDeclType.Float4 ->
                        fns.BlendWeight (Extractors.xBlendWeightFromFloat4 reader)
                    | _ -> failwithf "Unsupported type for blend weight: %A" dt
                | MMET.Format(f) ->
                    match f with
                    | SDXF.R8G8B8A8_UNorm ->
                        fns.BlendWeight (Extractors.xBlendWeightFromUbyte4 reader)
                    | SDXF.R32G32B32A32_Float ->
                        fns.BlendWeight (Extractors.xBlendWeightFromFloat4 reader)
                    | _ -> failwithf "Unsupported format for blend weight: %A" f
            | MMVES.Color ->
                // TODO: currently ignored, but should probably keep this as baggage.
                match el.Type with
                | MMET.DeclType(dt) ->
                    match dt with
                    | SDXVertexDeclType.Color ->
                        reader.ReadBytes(4) |> ignore
                    | SDXVertexDeclType.Float4 ->
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                    | _ -> failwithf "Unsupported type for color: %A" dt
                | MMET.Format(f) ->
                    match f with
                    | SDXF.R8G8B8A8_UNorm ->
                        reader.ReadBytes(4) |> ignore
                    | SDXF.R32G32B32A32_Float ->
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                        reader.ReadSingle() |> ignore
                    | _ -> failwithf "Unsupported format for color: %A" f
            | _ -> failwithf "Unsupported semantic: %A" el.Semantic

    let private makeLoggedDisposable (disp:System.IDisposable) (message:string) =
        { new System.IDisposable with
            member x.Dispose() =
                if disp <> null then
                    log.Info "%s" message
                    disp.Dispose()
        }

    let getResult():InteropTypes.SnapshotResult =

        let getLen (s:string) =
            if s.Length < 8192
            then s.Length
            else
                log.Warn "string too long: %A" s
                0
        {
            Directory = lastBaseDir.Value
            SnapFilePrefix = lastBaseName.Value

            DirectoryLen = getLen lastBaseDir.Value
            SnapFilePrefixLen = getLen lastBaseName.Value
        }

    type IDeviceSnapState =
        inherit System.IDisposable

        abstract member StrideBytes: int
        abstract member OffsetBytes: int
        abstract member IBReader: BinaryReader
        abstract member VBReader: BinaryReader
        abstract member VertElements: VertexTypes.MMVertexElement []
        abstract member VBDS: Stream
        abstract member IBDS: Stream
        abstract member IndexSizeBytes: int

        abstract member GetEnabledTextureStages: unit -> int list
        abstract member WriteDecl: string * string -> unit
        abstract member WriteTransforms: string * string -> unit

    type SnapStateD3D9(device:nativeint,sd:InteropTypes.SnapshotData) =
        let mutable vb:VertexBuffer9 = null
        let mutable ib:IndexBuffer9 = null
        let mutable vbLocked = false
        let mutable ibLocked = false

        let unlock() =
            log.Info ("unlocking snapshot buffers")
            if ibLocked then
                ib.Unlock()
            if vbLocked then
                vb.Unlock()

        let mutable offsetBytes = 0
        let mutable strideBytes = 0
        let mutable deviceopt = None
        let mutable vbDisposable = None
        let mutable declBytes = [||]

        let ibReader,vbReader,elements,vbDS,ibDS =
            if sd.BaseVertexIndex <> 0 || sd.MinVertexIndex <> 0u then
                // need a test case for these
                log.Warn "One or more of baseVertexIndex, minVertexIndex is not zero.  Snapshot may not handle this case"

            // check primitive type
            let primType = enum<PrimitiveType9>(sd.PrimType)
            if primType <> PrimitiveType9.TriangleList then failwithf "Cannot snap primitives of type: %A; only triangle lists are supported" primType

            // check for null pointers in sd
            let indexBuffer = sd.RendData.d3d9.IndexBuffer
            let vertDecl = sd.RendData.d3d9.VertDecl

            //log.Info "DX9 sd: vertDecl: %A, indexBuffer: %A" sd.RendData.d3d9.VertDecl sd.RendData.d3d9.IndexBuffer
            if indexBuffer = 0n then failwithf "Index buffer is null (DX9 sd: vertDecl: %A, indexBuffer: %A)" sd.RendData.d3d9.VertDecl sd.RendData.d3d9.IndexBuffer
            if vertDecl = 0n then failwithf "Vertex declaration is null (DX9 sd: vertDecl: %A, indexBuffer: %A)" sd.RendData.d3d9.VertDecl sd.RendData.d3d9.IndexBuffer

            // create the device from the native pointer.
            // note: creating a new sharpdx wrapper object from a native pointer does not increase the com ref count.
            // however, disposing that object will decrease the ref count, which can lead to a crash.  Therefore,
            // we must only dispose objects that are allocated from scratch or via a d3d device call, such as
            // GetStreamSource below.
            let device = new Device9(device)
            deviceopt <- Some(device) // make a reference to prevent GC from doing anything funky with it

            // check the divider
            let mutable divider = 0
            device.GetStreamSourceFrequency(0, &divider)
            if divider <> 1 then failwith "Divider must be 1" // this code doesn't handle other cases right now

            // get active stream information for stream 0.  currently we ignore other streams (will log a warning below if the declaration
            // uses data from non-stream 0).
            device.GetStreamSource(0,&vb,&offsetBytes,&strideBytes)
            match vb with
            | null -> failwith "Stream 0 VB is null, cannot snap"
            | _ -> ()

            // need to dispose the vb
            vbDisposable <- Some(makeLoggedDisposable vb "disposing stream 0 vb")

            log.Info "Stream 0: offset: %d, stride: %d" offsetBytes strideBytes

            // index buffer
            ib <- new IndexBuffer9(indexBuffer) // do not dispose, native code owns it
            let ibDesc = ib.Description
            log.Info"IndexBuffer: Format: %A, Usage: %A, Pool: %A, Size: %d" ibDesc.Format ibDesc.Usage ibDesc.Pool ibDesc.Size

            // check format
            if ibDesc.Format <> Format9.Index16 then failwithf "Cannot snap indices of type: %A; only index16 are supported" ibDesc.Format

            // vertex declaration
            let decl = new VertexDeclaration9(vertDecl) // do not dispose, native code owns it
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
            declBytes <- declMS.ToArray()
            // convert elements to MM elements
            let elements = elements |> Array.map (VertexTypes.sdxDeclElementToMMDeclElement)

            // lock vb and ib
            let vbDS = vb.Lock(0, vb.Description.SizeInBytes, LockFlags9.ReadOnly)
            // sharpdx always appears to return a valid object even if lock fails, so consider it locked
            vbLocked <- true
            if not vbDS.CanRead then failwith "Failed to lock vertex buffer for reading"
            let vbReader = new BinaryReader(vbDS)
            let ibDS = ib.Lock(0, ib.Description.Size, LockFlags9.ReadOnly)
            ibLocked <- true
            if not ibDS.CanRead then failwith "Failed to lock index buffer for reading"
            let ibReader = new BinaryReader(ibDS)
            (ibReader,vbReader,elements,vbDS,ibDS)

        let mutable disposed = false

        interface System.IDisposable with
            member x.Dispose() =
                if not disposed then
                    disposed <- true
                    ibReader.Dispose()
                    vbReader.Dispose()
                    unlock()
                    vbDisposable |> Option.iter (fun vb -> vb.Dispose())

        interface IDeviceSnapState with
            member x.StrideBytes = strideBytes
            member x.OffsetBytes = offsetBytes
            member x.IBReader = ibReader
            member x.VBReader = vbReader
            member x.VertElements = elements
            member x.VBDS = vbDS :> Stream
            member x.IBDS = ibDS :> Stream
            member x.IndexSizeBytes = 2

            member x.GetEnabledTextureStages() =
                // write textures for enabled stages only
                // Note: Sometimes we can't read textures from the device.
                // The flags need to be set properly in CreateTexture to make this
                // possible, and some games don't do that.  I'm fuzzy on the specifics, but I think its
                // D3DUSAGE_DYNAMIC that prevents capture, because the
                // driver might decide to put the texture in video memory and then we can't read it.
                // We could override that universally but it could harm
                // game performance and/or bloat memory.  This is a place where separate snapshot/playback modes could be
                // useful.
                let maxStage = 7 // 8 textures ought to be enough for anybody.

                match deviceopt with
                | Some(device) ->
                    [0..maxStage]
                    |> List.filter (fun i ->
                        let state = device.GetTextureStageState(i, TextureStage9.ColorOperation)
                        if state <> 1 then // 1 = D3DTOP_DISABLE
                            true
                        else
                            // some games disable the stage but put textures on it anyway.
                            let stageTex = device.GetTexture(i)
                            use disp = makeLoggedDisposable stageTex (sprintf "disposing snapshot texture %d" i)
                            stageTex <> null)
                | None -> []

            member x.WriteDecl(basedir,basename) =
                let declfile = Path.Combine(basedir, (sprintf "%s_VBDecl.dat" basename))
                File.WriteAllBytes(declfile, declBytes)

            member x.WriteTransforms(basedir,basename) =
                // Global transforms
                // Usually, only old games that still use fixed function for part of their rendering
                // Will set these, since shaders use constants to get these values instead.
                // So only write out the file if at least one of these is non-identity.
                match deviceopt with
                | None -> ()
                | Some(device) ->
                    let w0 = device.GetTransform(TransformState9.World);
                    let w1 = device.GetTransform(TransformState9.World1);
                    let w2 = device.GetTransform(TransformState9.World2);
                    let w3 = device.GetTransform(TransformState9.World3);
                    let view = device.GetTransform(TransformState9.View);
                    let proj = device.GetTransform(TransformState9.Projection)
                    use s = new StringWriter()
                    let writeMat (mat:SharpDX.Matrix) label =
                        if not (mat.IsIdentity) then
                            s.WriteLine(sprintf "%s: values=%A" label mat)

                    writeMat w0 "w0"
                    writeMat w1 "w1"
                    writeMat w2 "w2"
                    writeMat w3 "w3"
                    writeMat view "view"
                    writeMat proj "proj"

                    let s = s.ToString()
                    if s <> "" then
                        let fname = Path.Combine(basedir, (sprintf "%s_Transforms.txt" basename))
                        File.WriteAllText(fname, s.ToString())

    type SnapStateD3D11(_device:nativeint,sd:InteropTypes.SnapshotData) =
        let vertSize = sd.RendData.d3d11.VertSizeBytes
        let indexSize = sd.RendData.d3d11.IndexSizeBytes

        let mutable offsetBytes = 0
        let mutable strideBytes = vertSize
        let mutable texIdx = []

        let ibReader,vbReader,elements,vbDS,ibDS =
            if sd.BaseVertexIndex <> 0 || sd.MinVertexIndex <> 0u then
                // need a test case for these
                log.Warn "One or more of baseVertexIndex, minVertexIndex is not zero.  Snapshot may not handle this case"

            // check primitive type
            let primType = sd.PrimType
            if primType <> 4 then failwithf "Cannot snap primitives of type: %A; only triangle lists are supported" primType

            // check for null pointers
            let (vbData,vbSize) = sd.RendData.d3d11.VertexData,sd.RendData.d3d11.VertexDataSizeBytes
            let (ibData,ibSize) = sd.RendData.d3d11.IndexData, sd.RendData.d3d11.IndexDataSizeBytes

            if (FSharp.NativeInterop.NativePtr.toNativeInt vbData) = 0n then
                failwithf "VBData is null: size: %A, vert size: %A" vbSize vertSize
            if (FSharp.NativeInterop.NativePtr.toNativeInt ibData) = 0n then
                failwithf "IBData is null: size: %A, index size: %A" ibSize indexSize
            if vertSize = uint32 0 then
                failwithf "Vertex size is zero: %A" vertSize
            if indexSize = uint32 0 then
                failwithf "Index size is zero: %A" indexSize

            // check layout
            let layoutptr = sd.RendData.d3d11.LayoutElems
            if (FSharp.NativeInterop.NativePtr.toNativeInt layoutptr) = 0n then
                failwithf "Layout pointer is null: %A %A" sd.RendData.d3d11.LayoutElems sd.RendData.d3d11.LayoutElemsSizeBytes
            if int sd.RendData.d3d11.LayoutElemsSizeBytes = 0 then
                failwithf "Layout size bytes is zero: %A %A" sd.RendData.d3d11.LayoutElems sd.RendData.d3d11.LayoutElemsSizeBytes

            //log.Info "DX11 sd: layoutptr: %A, sizebytes: %A" sd.RendData.d3d11.LayoutElems sd.RendData.d3d11.LayoutElemsSizeBytes
            let (elements,elStr) = ModDBInterop.d3d11ElementsFromPtr layoutptr (int sd.RendData.d3d11.LayoutElemsSizeBytes)
            log.Info "Elements: vert code %A, semantics:\n  %s" (elStr.GetHashCode()) (elStr.Trim())
            log.Info "VB size: %A, vert size: %A; IB size: %A, index size: %A" vbSize vertSize ibSize indexSize

            let vbDS = new UnmanagedMemoryStream(vbData, int64 vbSize, int64 vbSize, FileAccess.Read)
            let vbReader = new BinaryReader(vbDS)
            let ibDS = new UnmanagedMemoryStream(ibData, int64 ibSize, int64 ibSize, FileAccess.Read)
            let ibReader = new BinaryReader(ibDS)

            let nTexBase = sd.RendData.d3d11.ActiveTexIndices
            let indices = ResizeArray<int>()
            for idx in [0..int sd.RendData.d3d11.NumActiveTexIndices-1] do
                let ival = NativeInterop.NativePtr.get nTexBase idx
                indices.Add(int ival)

            texIdx <- indices |> List.ofSeq

            (ibReader,vbReader,elements,vbDS,ibDS)

        let mutable disposed = false

        interface System.IDisposable with
            member x.Dispose() =
                if not disposed then
                    disposed <- true
                    ibDS.Dispose()
                    vbDS.Dispose()
                    ibReader.Dispose()
                    vbReader.Dispose()

        interface IDeviceSnapState with
            member x.StrideBytes = int strideBytes
            member x.OffsetBytes = offsetBytes
            member x.IBReader = ibReader
            member x.VBReader = vbReader
            member x.VertElements = elements
            member x.VBDS = vbDS :> Stream
            member x.IBDS = ibDS :> Stream
            member x.IndexSizeBytes = int indexSize

            member x.GetEnabledTextureStages() = texIdx
            member x.WriteDecl(basedir,basename) = ()
            member x.WriteTransforms(basedir,basename) = ()

    /// Take a snapshot using the specified snapshot data.  Additional data will be read directly from the device.
    /// Can fail for many reasons; always logs an exception and returns GenericFailureCode on error.
    /// Returns 0 on success.
    let take (device: nativeint) (sd:InteropTypes.SnapshotData) =
        try
            // check context and grab the saveTexture funcptr while we're at it
            let saveTexture =
                match State.Context with
                | "mm_native" -> None
                | "d3d9" -> Some(NativeImportsAsD3D9.SaveTexture)
                | "d3d11" -> None // native code saves these after snapshot
                | s ->
                    failwithf "unrecognized context: %s" s

            incr snapshotNum
            log.Info "Snapshot: number %d" snapshotNum.Value

            let inpSize = sd.SDSize
            let mySize = uint32 (System.Runtime.InteropServices.Marshal.SizeOf(typeof<InteropTypes.SnapshotData>))

            log.Info "  Snapshot data struct size: managed: %d, native: %d"  mySize inpSize

            if mySize <> inpSize then
                // of course is input is larger than my size we just blew the stack, but log anyway
                failwithf "aborting: input snapshot struct size %d does not match code size %d" inpSize mySize

            log.Info "  Capturing %d primitives composed of %d vertices with primitive type %d" sd.PrimCount sd.NumVertices sd.PrimType
            log.Info "  MinVertexIndex: %d, BaseVertexIndex: %d, StartIndex: %d" sd.MinVertexIndex sd.BaseVertexIndex sd.StartIndex

            let dss =
                match State.Context with
                | "d3d9" ->
                    new SnapStateD3D9(device, sd) :> IDeviceSnapState
                | "d3d11" ->
                    new SnapStateD3D11(device, sd) :> IDeviceSnapState
                | s ->
                    failwithf "unrecognized context: %s" s

            // sort elements ascending by offset to avoid seeking the reader
            let declElements = dss.VertElements |> List.ofArray |> List.sortBy (fun el -> el.Offset)

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
            // we don't support usage indices > 0 for any semantic so create some trashcan readers for those
            let readIgnoreFns = {
                ReadOutputFunctions.Pos = fun (x,y,z) -> ()
                Normal = fun (x,y,z) -> ()
                TexCoord = fun (u,v) -> ()
                Binormal = fun (x,y,z) -> ()
                Tangent = fun (x,y,z) -> ()
                BlendIndex = fun (a, b, c, d) -> ()
                BlendWeight = fun (a, b, c, d) -> ()
            }
            // log if any usages over 0 are found
            declElements |> List.iter (fun el ->
                if el.SemanticIndex > 0 then
                    log.Warn "semantic index %A not supported for semantic %A, this data will be ignored" el.SemanticIndex el.Semantic
            )

            // create per-element read function bound to the reader
            let readVertElement = readElement readOutputFns readIgnoreFns dss.VBReader

            // start at minIndex and write out numVerts (we only write the verts used by the DIP call)
            let vbStartOffset = int64 dss.OffsetBytes + ((int64 sd.BaseVertexIndex + int64 sd.MinVertexIndex) * int64 dss.StrideBytes)
            ignore (dss.VBDS.Seek(vbStartOffset, SeekOrigin.Begin) )
            // walk the verts to populate data arrays.
            // elements are sorted in offset order, so we only need to seek the reader between verts (not between elements)
            // we do assume that each extractor reads the full
            // amount of data for its type (for example a ubyte4 extractor should read 4 bytes even if the 4th is ignored)
            let stride = dss.StrideBytes
            let processVert i =
                ignore (dss.VBDS.Seek(vbStartOffset + (int64 i * int64 stride),SeekOrigin.Begin))
                declElements |> List.iter readVertElement
            [0..(int sd.NumVertices-1)] |> List.iter processVert

            // now write the index (primitive) data
            // since we only wrote out the potentially-usable verts, and not the full buffer, we have to offset each index by
            // MinVertexIndex, since that is the lowest possible index that we can use
            // TODO: I think I've seen this work with minVertexIndex <> 0, but I'm not sure since that is an uncommon case;
            // needs definitive test.
            let indexElemSize = dss.IndexSizeBytes
            let readIndex =
                match indexElemSize with
                | 2 -> fun () -> dss.IBReader.ReadInt16() |> int
                | 4 -> fun () -> dss.IBReader.ReadInt32() |> int
                | _ -> failwithf "unsupported index size: %d" indexElemSize
            // TODO: check for dx11
            let ibStartOffset = int64 sd.MinVertexIndex * (int64 indexElemSize) + int64 (sd.StartIndex * uint32 indexElemSize)
            ignore (dss.IBDS.Seek(ibStartOffset, SeekOrigin.Begin))

            let triangles = new ResizeArray<IndexedTri>()

            let processTriangle _ =
                let a = readIndex()
                let b = readIndex()
                let c = readIndex()

                // since vert,normal,texture arrays are all the same size, use the same index for each.
                let verts:PTNIndex[] = Array.zeroCreate 3
                verts.[0] <- { Pos = a; Tex = a; Nrm = a }
                verts.[1] <- { Pos = b; Tex = b; Nrm = b }
                verts.[2] <- { Pos = c; Tex = c; Nrm = c }
                triangles.Add({ Verts = verts})

            [1..(int sd.PrimCount)] |> List.iter processTriangle

            // set up to write files
            let baseDir = State.getExeSnapshotDir()

            if not (Directory.Exists baseDir) then
                Directory.CreateDirectory(baseDir) |> ignore

            let sbasename = sprintf "snap_%d_%dp_%dv" snapshotNum.Value sd.PrimCount sd.NumVertices

            lastBaseDir := baseDir
            lastBaseName := sbasename

            let texturePaths =
                dss.GetEnabledTextureStages()
                |> List.map (fun i ->
                    let texName = sprintf "%s_texture%d.dds" sbasename i
                    let texPath = Path.Combine(baseDir, texName)
                    // log.Info "Saving texture %d to %s" i texPath
                    match saveTexture with
                    | Some(fn) ->
                        if fn(i, texPath) then
                            i,(texName,texPath)
                        else
                            // failed save; native code should have logged it
                            i,("","")
                    | None -> i,(texName,texPath) // assume native is saving it with this name somehow
                )
                |> List.filter (fun (i,(tName,tPath)) -> tName <> "")
                |> Map.ofList

            let snapProfile =
                State.Data.SnapshotProfiles
                |> Map.tryFind State.Data.Conf.SnapshotProfile
                |> function
                    | None ->
                        log.Warn "No transforms found for profile: %A" State.Data.Conf.SnapshotProfile
                        SnapshotProfile.EmptyProfile
                    | Some s ->
                        log.Info "Applying transforms: %A" s
                        s

            let appliedPosTransforms = snapProfile.PosXForm()
            let appliedUVTransforms = snapProfile.UVXForm()

            // use the first texture (if available) as the mesh material
            let texName idx = if texturePaths.ContainsKey idx then (fst <| texturePaths.Item idx) else ""

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
                Tex0Path = texName 0
                Tex1Path = texName 1
                Tex2Path = texName 2
                Tex3Path = texName 3
                AnnotatedVertexGroups = [||]
                Cached = false
            }

            // apply tranforms
            let mesh = MeshTransform.applyMeshTransforms appliedPosTransforms appliedUVTransforms mesh

            // write mesh
            let meshfile = sprintf "%s.mmobj" sbasename
            let meshfile = Path.Combine(baseDir,meshfile)
            MeshUtil.writeObj mesh meshfile

            // write vert decl
            dss.WriteDecl(baseDir, sbasename)

            // write raw ib and vb; just write the portion that was used by the DIP call
            // Note: these are generally for debug only; create mod tool doesn't even use them.
            let getStreamBytes (startoffset) (datastream:Stream) size =
                datastream.Seek(startoffset, SeekOrigin.Begin) |> ignore
                let data:byte[] = Array.zeroCreate size
                let ibBytes = datastream.Read(data,0,data.Length)
                data

            let ibBytesToRead = int sd.PrimCount * 3 * int indexElemSize
            getStreamBytes ibStartOffset dss.IBDS ibBytesToRead
                |>
                (fun bytes ->
                    // write header
                    let iCount = int sd.PrimCount * 3
                    let iSize = indexElemSize

                    let fname = Path.Combine(baseDir, (sprintf "%s_IB.dat" sbasename))
                    use bw = new BinaryWriter(new FileStream(fname, FileMode.Create))
                    bw.Write(iCount)
                    bw.Write(iSize)
                    bw.Write(bytes)
                    ())

            let vbBytesToRead = int sd.NumVertices * dss.StrideBytes
            //log.Warn "read vb: num: %A, stride: %A, startoff: %A, bytes: %A" sd.NumVertices dss.StrideBytes vbStartOffset vbBytesToRead
            getStreamBytes vbStartOffset dss.VBDS vbBytesToRead
                |>
                (fun bytes ->
                    // write header
                    let fname = Path.Combine(baseDir, (sprintf "%s_VB.dat" sbasename))
                    use bw = new BinaryWriter(new FileStream(fname, FileMode.Create))
                    bw.Write(sd.NumVertices)
                    bw.Write(stride)
                    bw.Write(bytes)
                    ())

            dss.WriteTransforms(baseDir, sbasename)

            log.Info "Wrote snapshot %d to %s" snapshotNum.Value baseDir

            dss.Dispose()
            0
        with
            e ->
                log.Error "%A" e
                // Note, don't do this, it crashes. // TODO11: recheck this
                //unlock()
                InteropTypes.GenericFailureCode
