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

open System.IO

open Microsoft.Xna.Framework

open CoreTypes
open InteropTypes

// Abbreviations to make the many match statements below slightly
// less nutty.
type MMET = VertexTypes.MMVertexElementType
type SDXVT = SDXVertexDeclType
type SDXF = SharpDX.DXGI.Format

/// Utility module with various locking functions, used in asynchronous mod load.
/// Originally based on code from Expert F#, with some additions.
module private Locking =
    let private log = Logging.getLogger("Locking")

    open System.Threading

    let private rwlock = new ReaderWriterLockSlim()

    let read f =
        try
            rwlock.EnterReadLock()
            try
                f()
            finally
                rwlock.ExitReadLock()
        with
            | e -> log.Error "Failed to acquire read lock or in lock function %A" e

    let write f =
        try
            rwlock.EnterWriteLock()
            try
                f()
                Thread.MemoryBarrier()
            finally
                rwlock.ExitWriteLock()
        with
            | e -> log.Error "Failed to acquire write lock or in lock function %A" e

    let upgradeRead f =
        try
            rwlock.EnterUpgradeableReadLock()
            try
                f()
                Thread.MemoryBarrier()
            finally
                rwlock.ExitUpgradeableReadLock()
        with
            | e -> log.Error "Failed to acquire upgradeRead lock or in lock function %A" e

/// Provides a ModDB interface to native code.
module ModDBInterop =
    open VertexTypes

    let private log = Logging.getLogger("ModDBInterop")

    /// Initializes the system with a specific modelmod dll path and executable module.  Reads the
    /// registry configuration for the specified executable (if any).
    let setPaths (rootOrDllPath:string) (exeModule:string) =
        try
            // check for valid paths
            if rootOrDllPath.Contains("..") then failwithf "Illegal dll path, contains '..' : %A" rootOrDllPath
            if exeModule.Contains("..") then failwithf "Illegal exe module, contains '..' : %A" exeModule

            log.Info "exe module: %A" exeModule

            // set the root path to the parent of the native ModelMod.dll.
            let rootDir =
                if Directory.Exists(rootOrDllPath) then rootOrDllPath
                else Directory.GetParent(rootOrDllPath).ToString()

            if not (Directory.Exists rootDir) then
                failwithf "root directory does not exist: %A" rootDir

            let conf = RegConfig.load exeModule
            let conf = State.validateAndSetConf rootDir conf

            let ret = {
                RunModeFull = conf.RunModeFull
                LoadModsOnStart = conf.LoadModsOnStart
                InputProfile = conf.InputProfile
                MinimumFPS = conf.MinimumFPS
                ProfileKey = conf.ProfileKeyName
            }
            //log.Info "Returning %A" ret
            ret
        with
        | e ->
            log.Error "%A" e
            {
                RunModeFull = CoreTypes.DefaultRunConfig.RunModeFull
                LoadModsOnStart = CoreTypes.DefaultRunConfig.LoadModsOnStart
                InputProfile = CoreTypes.DefaultRunConfig.InputProfile
                MinimumFPS = CoreTypes.DefaultRunConfig.MinimumFPS
                ProfileKey = ""
            }

    /// Loads the exe-specific data.  Requires a ModIndex.yaml file to exist in the exe's data directory.
    let loadFromDataPath() =
        try
            let exeDataDir = State.getExeDataDir()
            log.Info "Loading data from path: %A" exeDataDir

            if not (Directory.Exists(exeDataDir)) then
                log.Warn "Can't find data directory for this executable, consider setting an override in the GameProfile XX"
                failwithf "Cannot load data, dir does not exist: %A" exeDataDir

            // look for ModIndex file
            let modIndexPath = Path.Combine(exeDataDir,"ModIndex.yaml")
            if not (File.Exists(modIndexPath)) then
                failwithf "Cannot load data, index file does not exist: %A" modIndexPath

            let conf = {
                StartConf.Conf.ModIndexFile = Some modIndexPath
                StartConf.Conf.FilesToLoad = []
                StartConf.Conf.AppSettings = None
            }

            let mdb =  ModDB.loadModDB(conf,Some(State.Data.Moddb))
            Locking.write (fun _ -> State.Data.Moddb <- mdb)

            Util.reportMemoryUsage()
            0
        with
        | e ->
            log.Error "%A" e
            InteropTypes.GenericFailureCode

    let getLoadingState() =
        match State.Data.LoadState with
        | NotStarted -> AsyncLoadNotStarted
        | Complete -> AsyncLoadComplete
        | Pending -> AsyncLoadPending
        | InProgress -> AsyncLoadInProgress

    let loadFromDataPathAsync() =
        Locking.upgradeRead (fun _ ->
            match State.Data.LoadState with
            | Pending | InProgress -> () // no-op
            | NotStarted | Complete ->
                Locking.write (fun _ -> State.Data.LoadState <- Pending)

                async {
                    // now running from thread pool
                    let mutable canLoad = false
                    Locking.write (fun _ ->
                        match State.Data.LoadState with
                        | NotStarted ->
                            log.Info("Async loading state is NotStarted prior to task pool load, WTF?")
                        | InProgress | Complete -> () // no-op
                        | Pending ->
                            State.Data.LoadState <- InProgress
                            canLoad <- true
                    )

                    if canLoad then
                        log.Info("Async load started")
                        loadFromDataPath() |> ignore
                        log.Info("Async load complete")

                        Locking.write (fun _ ->
                            if not (State.Data.LoadState = InProgress) then
                                log.Error "WHOA unexpected loading state: %A" State.Data.LoadState
                            State.Data.LoadState <- Complete)
                } |> Async.Start
        )

        getLoadingState()


    /// Get the loaded mod count.
    let getModCount() = State.Data.Moddb.MeshRelations.Length + State.Data.Moddb.DeletionMods.Length

    /// Converts a mod type to a native-enum compatible interger.
    let modTypeToInt modType =
        match modType with
        | CPUReplacement -> 2
        | GPUAdditive -> 1
        | GPUReplacement -> 3
        | Deletion -> 5
        | Reference -> failwith "A mod has type set to reference"
    let intToModType ival =
        match ival with
        | 1 -> GPUAdditive
        | 2 -> CPUReplacement
        | 3 -> GPUReplacement
        | 5 -> Deletion
        | _ -> failwithf "value cannot be converted into a mod type: %A" ival

    /// Get the MeshRel mod at the specified index.
    let private getMeshRelationMod i =
        let moddb = State.Data.Moddb
        let meshrel = List.item i (moddb.MeshRelations)
        let refm = meshrel.RefMesh
        let modm = meshrel.ModMesh

        let declSize,vertSize =
            // This is used by DX9, but DX11 computes its own vert size based on the current layout.
            match meshrel.GetVertDeclaration() with
            | None -> 
                match CoreState.Context with 
                | "d3d9" -> failwith "A vertex declaration must be set here, native code requires it."
                | "d3d11" -> 
                    (0, 0)
                | x -> failwithf "Unknown context: %A" x
            | Some (data,elements) -> 
                let vertSize = MeshUtil.getVertSizeFromDecl elements
                data.Length,vertSize

        let modType = modTypeToInt modm.Type

        let primType = 4 //D3DPT_TRIANGLELIST // TODO11
        let vertCount = modm.Positions.Length
        let primCount = modm.Triangles.Length
        let indexCount = 0
        let refPrimCount = meshrel.DBRef.PrimCount
        let refVertCount = meshrel.DBRef.VertCount
        let declSizeBytes = declSize
        let vertSizeBytes = vertSize
        let indexElemSizeBytes = 0

        let mname = meshrel.DBMod.Name
        let parentModName =
            match meshrel.DBMod.ParentModName with
            | None -> ""
            | Some(name) -> name

        let updateTS =
            match meshrel.DBMod.UpdateTangentSpace with
            | None -> -1
            | Some(upd) -> if upd then 1 else 0

        {
            InteropTypes.ModData.ModType = modType
            PrimType = primType
            VertCount = vertCount
            PrimCount = primCount
            IndexCount = indexCount
            RefVertCount = refVertCount
            RefPrimCount = refPrimCount
            DeclSizeBytes = declSizeBytes
            VertSizeBytes = vertSizeBytes
            IndexElemSizeBytes = indexElemSizeBytes
            Tex0Path = modm.Tex0Path
            Tex1Path = modm.Tex1Path
            Tex2Path = modm.Tex2Path
            Tex3Path = modm.Tex3Path
            PixelShaderPath = meshrel.DBMod.PixelShader
            ModName = mname
            ParentModName = parentModName
            UpdateTangentSpace = updateTS
        }

    /// Get the mod data at the specified index.  If index is out of range, returns InteropTypes.EmptyModData.
    let getModData(i) =
        // emptyMod is used for error return cases.  Doing this allows us to keep the ModData as an F# record,
        // which does not allow null.  Can't use option type here because native code calls this.
        let emptyMod = InteropTypes.EmptyModData

        try
            let moddb = State.Data.Moddb

            let maxMods = getModCount()

            // the index is "virtualized".  the first n mods are the meshrelation mods.  after that are
            // the deletion mods.

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
                    List.item delIdx moddb.DeletionMods
                | n -> failwithf "invalid mod index: %A" i

            //log.Info "Returning mod %A for index %A" ret i
            ret
        with
            | e ->
                log.Error "%s" e.Message
                log.Error "%s" e.StackTrace
                emptyMod

    /// Return a binary writer for the specified unmanaged pointer.  If size parameter is less than the buffer size,
    /// this will probably explode when you write to it, if you're lucky.
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

        // So far, W in last position seems to be the "common" pattern, even if other components are reversed
        if State.Data.Conf.GameProfile.ReverseNormals then
            bw.Write(z)
            bw.Write(y)
            bw.Write(x)
            bw.Write(w)
        else
            bw.Write(x)
            bw.Write(y)
            bw.Write(z)
            bw.Write(w)

    let writeF3Vector (v:Vector3) (bw:BinaryWriter) =
        // TODO: why doesn't this respect State.Data.Conf.GameProfile.ReverseNormals? related to issue #10 ?
        bw.Write(v.X)
        bw.Write(v.Y)
        bw.Write(v.Z)

    /// Contains helper functions for writing vertex data based on "raw binary" input - which are exact binary dumps of snapshot
    /// data.  These are primarily used for debugging but there might be a few mods out there using them (in particular some early
    /// mods might have just copied the binormals and tangents from the ref mesh, in which case they would have used this.
    /// Vast majority of mods don't use this.
    module RawBinaryWriters =
        /// Write a blend index extracted from raw binary data as a 4-byte array.
        let rbBlendIndex (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBI() =
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Semantic)
                let idx = br.ReadBytes(4)
                bw.Write(idx)

            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Ubyte4 -> writeBI()
                | _ -> failwithf "Unsupported type for raw blend index: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UInt -> writeBI()
                | _ -> failwithf "Unsupported format for raw blend index: %A" el.Type

        /// Write a blend weight extracted from raw binary data as a 4-byte array.
        let rbBlendWeight (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBW() =
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Semantic)
                bw.Write(br.ReadBytes(4))
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N -> writeBW()
                | _ -> failwithf "Unsupported type for raw blend weight: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeBW()
                | _ -> failwithf "Unsupported format for raw blend weight: %A" el.Type

        /// Write a normal extracted from raw binary data.
        let rbNormal (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (_unused: int) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeNormal() =
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Semantic)
                bw.Write(br.ReadBytes(4))
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.Ubyte4 -> writeNormal()
                | _ -> failwithf "Unsupported type for raw normal: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeNormal()
                | _ -> failwithf "Unsupported format for raw normal: %A" el.Type

        /// Write a binormal or tangent vector extracted from raw binary data.
        let rbBinormalTangent (binDataLookup:ModDB.BinaryLookupHelper) (vertRels:MeshRelation.VertRel[]) (_unused: int) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBT() =
                let br = binDataLookup.BinaryReader(vertRels.[modVertIndex].RefPointIdx, el.Semantic)
                bw.Write(br.ReadBytes(4))
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.Ubyte4 -> writeBT()
                | _ -> failwithf "Unsupported type for raw binormal/tangent: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeBT()
                | _ -> failwithf "Unsupported format for raw binormal/tangent: %A" el.Type

    /// Helper functions for writing data.
    module DataWriters =
        let private round (x:float32) = System.Math.Round (float x)

        /// Write a blend index from the specified mesh as a 4-byte array.
        let private writeMeshBI (mesh:Mesh) (idx:int) (bw:BinaryWriter) =
            if (idx > mesh.BlendIndices.Length) then
                failwithf "oops: invalid blend-index index: %A of %A" idx mesh.BlendIndices.Length
            let bi = mesh.BlendIndices.[idx]
            let buf = [| byte (bi.X :?> int32); byte (bi.Y :?> int32) ; byte (bi.Z :?> int32); byte (bi.W :?> int32) |]
            bw.Write(buf)

        /// Write a blend weight from the specified mesh as a 4-byte array.
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

        /// Write a blend weight from the specified mesh as 4 32-bit float values.
        let private writeMeshBWF4 (mesh:Mesh) (idx:int) (bw:BinaryWriter) =
            if (idx > mesh.BlendWeights.Length) then
                failwithf "oops: invalid blend-weight index: %A of %A" idx mesh.BlendWeights.Length
            let wgt = mesh.BlendWeights.[idx]
            bw.Write(wgt.X)
            bw.Write(wgt.Y)
            bw.Write(wgt.Z)
            bw.Write(wgt.W)

        /// Write a blend index from the mod mesh.
        let modmBlendIndex (_vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBI() = writeMeshBI modm modVertIndex bw
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.Ubyte4 -> writeBI()
                | _ -> failwithf "Unsupported type for mod blend index: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeBI()
                | _ -> failwithf "Unsupported format for mod blend index: %A" el.Type

        /// Write a blend index from the ref mesh.
        let refmBlendIndex (vertRels:MeshRelation.VertRel[]) (refm:Mesh) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBI() =
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBI refm refVertIndex bw
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.Ubyte4 -> writeBI()
                | _ -> failwithf "Unsupported type for ref blend index: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm
                | SDXF.R8G8B8A8_UInt -> writeBI()
                | _ -> failwithf "Unsupported format for ref blend index: %A" el.Type

        /// Write a blend weight from the mod mesh.
        let modmBlendWeight (vertRels:MeshRelation.VertRel[]) (modm:Mesh) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBW() = writeMeshBW modm modVertIndex bw
            let writeBWF4() = writeMeshBWF4 modm modVertIndex bw
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N -> writeBW()
                | SDXVertexDeclType.Float4 -> writeBWF4()
                | _ -> failwithf "Unsupported type for mod blend weight: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeBW()
                | SDXF.R32G32B32A32_Float -> writeBWF4()
                | _ -> failwithf "Unsupported format for mod blend weight: %A" el.Type

        /// Write a blend weight from the ref mesh.
        let refmBlendWeight (vertRels:MeshRelation.VertRel[]) (refm:Mesh) (modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            let writeBW() =
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBW refm refVertIndex bw
            let writeBWF4() =
                let refVertIndex = vertRels.[modVertIndex].RefPointIdx
                writeMeshBWF4 refm refVertIndex bw
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N -> writeBW()
                | SDXVertexDeclType.Float4 -> writeBWF4()
                | _ -> failwithf "Unsupported type for ref blend weight: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> writeBW()
                | SDXF.R32G32B32A32_Float -> writeBWF4()
                | _ -> failwithf "Unsupported type for ref blend weight: %A" el.Type
        /// Write a normal from the mod mesh.
        let modmNormal (modm:Mesh) (modNrmIndex: int) (_modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N
                | SDXVertexDeclType.Ubyte4 ->
                    // convert normal to 4 byte rep
                    let srcNrm = modm.Normals.[modNrmIndex]
                    write4ByteVector srcNrm bw
                | SDXVertexDeclType.Float3 ->
                    let srcNrm = modm.Normals.[modNrmIndex]
                    writeF3Vector srcNrm bw
                | _ -> failwithf "Unsupported type for mod normal: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R32G32B32_Float ->
                    let srcNrm = modm.Normals.[modNrmIndex]
                    writeF3Vector srcNrm bw
                | SDXF.R8G8B8A8_UNorm ->
                    let srcNrm = modm.Normals.[modNrmIndex]
                    write4ByteVector srcNrm bw
                | _ -> failwithf "Unsupported type for mod normal: %A" el.Type

        /// Write a binormal or tangent vector, computed using the normal from the mod mesh.
        let modmBinormalTangent (modm:Mesh) (modNrmIndex: int) (_modVertIndex: int) (el:MMVertexElement) (bw:BinaryWriter) =
            // This isn't the most accurate way to compute these, but its easier than the mathematically correct method, which
            // requires inspecting the triangle and uv coordinates.  Its probably worth implementing that at some point,
            // but this produces good enough results in most cases.
            // Update: well actually this produces bad results as shader detail increases (see issue #10)
            // see: http://www.geeks3d.com/20130122/normal-mapping-without-precomputed-tangent-space-vectors/
            let srcNrm = modm.Normals.[modNrmIndex]
            let v1 = Vector3.Cross(srcNrm, Vector3(0.f, 0.f, 1.f))
            let v2 = Vector3.Cross(srcNrm, Vector3(0.f, 1.f, 0.f))
            let t = if (v1.Length() > v2.Length()) then v1 else v2
            t.Normalize()
            let vec =
                if (el.Semantic = MMVertexElemSemantic.Binormal) then
                    let b = Vector3.Cross(srcNrm,t)
                    b.Normalize()
                    b
                else
                    t
            match el.Type with
            | MMET.DeclType(dt) ->
                match dt with
                | SDXVertexDeclType.Color
                | SDXVertexDeclType.UByte4N
                | SDXVertexDeclType.Ubyte4 -> write4ByteVector vec bw
                | SDXVertexDeclType.Float3 -> writeF3Vector vec bw
                | _ -> failwithf "Unsupported type for mod binormal/tangent: %A" el.Type
            | MMET.Format(f) ->
                match f with
                | SDXF.R8G8B8A8_UNorm -> write4ByteVector vec bw
                | SDXF.R32G32B32_Float -> writeF3Vector vec bw
                | _ -> failwithf "Unsupported format for mod binormal/tangent: %A" el.Type

    /// Unmarshal a native array of d3d11 InputElements into an array of MMVertexElement
    let d3d11LayoutToMMVert (br:BinaryReader) (maxSize:int64) =
        // SDX InputElement's marshalling code is internal to the that lib, so I'll just make a binary reader
        // and do it myself
        let els = new ResizeArray<VertexTypes.MMVertexElement>()

        while br.BaseStream.Position < int64 maxSize do
            let semName = br.ReadUInt64()
            let semIndex = br.ReadUInt32()
            let format = br.ReadUInt32()
            let slot = br.ReadUInt32()
            let offset = br.ReadUInt32()
            let slotclass = br.ReadUInt32()
            let stepRate = br.ReadUInt32()

            let name = System.Runtime.InteropServices.Marshal.PtrToStringAnsi(nativeint semName)
            let format = enum<SharpDX.DXGI.Format>(int format)
            let slotclass = enum<SharpDX.Direct3D11.InputClassification>(int slotclass)
            if slotclass = SharpDX.Direct3D11.InputClassification.PerVertexData then
                let sxel = new SharpDX.Direct3D11.InputElement(name, int semIndex, format, int offset, int slot, slotclass, int stepRate)
                let mmel = VertexTypes.layoutElToMMEl sxel name
                els.Add(mmel)
            else
                log.Warn "  Unrecognized slot class in vert: class %A, semantic %A" slotclass name

        els.ToArray()

    let vertElsToString(elements:MMVertexElement[]) =
        use sw = new StringWriter()
        for sxel in elements do
            sw.WriteLine(sprintf "  %A %A %A %A" sxel.Semantic sxel.SemanticIndex sxel.Offset sxel.Type)
        sw.Flush()
        sw.ToString()

    let observedVertTypes = new System.Collections.Generic.HashSet<string>()
    let d3d11ElementsFromPtr (ptr:nativeptr<byte>) (sizebytes:int) = 
        // "use" the stream, but the docs say disposal isn't necessary
        use stream = new UnmanagedMemoryStream(ptr, int64 sizebytes, int64 sizebytes, FileAccess.Read)
        use br = new BinaryReader(stream)
        let elements = d3d11LayoutToMMVert br (int64 sizebytes)
        // log the vert details if we haven't seen it before
        let elStr = vertElsToString(elements)
        if not (observedVertTypes.Contains(elStr)) then
            log.Info "Vert type %A contains %d elements" (elStr.GetHashCode()) elements.Length
            for sxel in elements do
                log.Info "  %A %A %A %A" sxel.Semantic sxel.SemanticIndex sxel.Offset sxel.Type
                if sxel.Slot > 0 then
                    log.Warn "    %A uses unsupported slot %A, data from (if any) slot 0 will be used to fill this" sxel.Semantic sxel.Slot
            observedVertTypes.Add(elStr) |> ignore
        (elements,elStr)

    type VertexDecl =
        WriteD3D9Decl of (BinaryWriter * int)
        | ReadD3D11Layout of (MMVertexElement [])

    /// Fill the render buffers associated with the specified mod.
    // Note: there is a lot of symmetry between this and the snapshot module (essentially they are the same
    // process in two different directions), but they have totally separate implementations right now.  Might be worth
    // unifying them in some way.
    let private fillModDataInternalHelper
        (modIndex:int)
        (destDeclBx:VertexDecl) (vertSize:int option)
        (destVbBw:BinaryWriter) (destVbSize:int)
        (destIbBw:BinaryWriter) (destIbSize:int) =
        try
            let moddb = State.Data.Moddb

            let md = getModData modIndex
            if (intToModType md.ModType) <> GPUReplacement
                && (intToModType md.ModType) <> GPUAdditive
                then failwithf "unsupported mod type: %d" md.ModType

            // grab more stuff that we'll need
            let meshrel = List.item modIndex (moddb.MeshRelations)
            let refm = meshrel.RefMesh
            let modm = meshrel.ModMesh
            let vertRels = meshrel.VertRelations

            let declElements =
                match destDeclBx with
                | WriteD3D9Decl (destDeclBw,destDeclSize) ->
                    let srcDeclData,declElements =
                        match meshrel.GetVertDeclaration() with
                        | None -> failwith "A vertex declaration must be set here, native code requires it."
                        | Some (data,elements) -> data,elements
                    // copy declaration data to destination
                    if (destDeclSize > 0) then
                        if destDeclSize <> srcDeclData.Length then
                            failwithf "Decl src/dest mismatch: src: %d, dest: %d" srcDeclData.Length destDeclSize

                        destDeclBw.Write(srcDeclData)
                    declElements |> List.map (VertexTypes.sdxDeclElementToMMDeclElement) |> Array.ofList
                | ReadD3D11Layout elements -> elements

            // in DX9 we use the vertex size from the mod, because we also create the declaration.
            // in DX11 we use the vertex size computed from the elements which are currently being
            // used to render.  TBH I'm not sure the DX9
            // approach is right, because, the declaration needs to match the shader, and we
            // don't control that.  So in the general case we can't just use whatever
            // decl we want, though it will accidentally work a lot.  For instance the
            // declaration/shaders used to render could change
            // based on detail level.  That definitely happens in DX11.
            let vertSizeBytes =
                match vertSize with
                | None -> md.VertSizeBytes
                | Some(size) -> size

            // copy index data...someday
            if (destIbSize > 0) then
                failwith "Filling index data is not yet supported"

            // copy vertex data.  this is where most of the work happens.
            if (destVbSize > 0) then
                // we aren't using an index list, so we'll fill the buffer with vertices representing all the primitives.

                // walk the mod triangle list: for each point on each triangle, write a unique entry into the vb.
                // the following bools control where data comes from (normally we don't want to change these except
                // when debugging something)

                // true: use normals from the mod
                // false: copy normals from nearest ref vert in raw binary data.  possibly useful for debugging but
                // will otherwise produces screwy results if the mod mesh is different enough.
                let useModNormals = true

                // true: compute the binormal and tangent from the mod normal
                // false: copy bin/tan from the nearest ref in raw binary data.  mostly for debug.
                let computeBinormalTangent = true

                let srcVbSize = md.PrimCount * 3 * vertSizeBytes
                if (destVbSize <> srcVbSize) then
                    failwithf "VB size src/dest mismatch: src: %d, dest: %d (prims: %A, vert size: %A)" srcVbSize destVbSize md.PrimCount vertSizeBytes

                let bw = destVbBw
                // sort vertex elements in offset order ascending, so that we don't have to reposition the memory stream
                // as we go
                let declElements = declElements |> Array.sortBy (fun el -> el.Offset)

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

                // determine how we will write the data, depending on weight mode and available input data sources

                let useRefBlendData,useRefBinaryData =
                    // if blending is required, fail unless specified weight source has the data.
                    // otherwise return the bool configuration tuple
                    let needsBlend = MeshUtil.hasBlendElements declElements
                    let wm = meshrel.DBMod.WeightMode

                    // user friendly error message
                    let failMsg = sprintf "mod named %A specifies %A weight mode, but no blend index/weight data found; add the data or use a different weight mode" meshrel.DBMod.Name wm

                    match needsBlend, wm with
                    | true,BinaryRef ->
                        match refm.BinaryVertexData with
                        | None -> failwith failMsg
                        | _ -> true,true
                    | true,WeightMode.Mod ->
                        match modm.BlendIndices,modm.BlendWeights with
                        | _,[||]
                        | [||],_ -> failwith failMsg
                        | _ -> false,false
                    | true,WeightMode.Ref ->
                        match refm.BlendIndices,refm.BlendWeights with
                        | _,[||]
                        | [||],_ -> failwith failMsg
                        | _ -> true,false
                    | false,_ -> false,false

                let refBinDataLookup =
                    match refm.BinaryVertexData with
                    | None -> None
                    | Some bvd -> Some (new ModDB.BinaryLookupHelper(bvd,declElements))

                let blendIndexWriter,blendWeightWriter =
                    if useRefBlendData then
                        if useRefBinaryData then
                            let binDataLookup =
                                match refBinDataLookup with
                                | None -> failwith "Binary vertex data is required to write blend index,blend weight"
                                | Some bvd -> bvd

                            let biw = RawBinaryWriters.rbBlendIndex binDataLookup vertRels
                            let bww = RawBinaryWriters.rbBlendWeight binDataLookup vertRels

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
                        let nrmw = RawBinaryWriters.rbNormal binDataLookup vertRels
                        nrmw

                let binormalTangentWriter =
                    if computeBinormalTangent then
                        DataWriters.modmBinormalTangent modm
                    else
                        let binDataLookup =
                            match refBinDataLookup with
                            | None -> failwith "Binary vertex data is required to write binormal"
                            | Some bvd -> bvd
                        RawBinaryWriters.rbBinormalTangent binDataLookup vertRels

                // Write part of a vertex.  The input element controls which
                // part is written.
                let writeElement (v:PTNIndex) (el:VertexTypes.MMVertexElement) =
                    let modVertIndex = v.Pos
                    let modNrmIndex = v.Nrm

                    match el.Semantic with
                        | MMVertexElemSemantic.Position ->
                            match el.Type with
                            | MMET.Format(f) when f = SDXF.R32G32B32_Float ->
                                let srcPos = srcPositions.[modVertIndex]
                                bw.Write(srcPos.X)
                                bw.Write(srcPos.Y)
                                bw.Write(srcPos.Z)
                            | MMET.DeclType(dt) when dt = SDXVT.Float3 ->
                                let srcPos = srcPositions.[modVertIndex]
                                bw.Write(srcPos.X)
                                bw.Write(srcPos.Y)
                                bw.Write(srcPos.Z)
                            | MMET.DeclType(dt) when dt = SDXVT.Unused -> ()
                            | _ -> failwithf "Unsupported type for position: %A" el.Type
                        | MMVertexElemSemantic.TextureCoordinate ->
                            match el.Type with
                            | MMET.DeclType(dt) when dt = SDXVT.Float2 ->
                                let srcTC = srcTex.[v.Tex]
                                bw.Write(srcTC.X)
                                bw.Write(srcTC.Y)
                            | MMET.DeclType(dt) when dt = SDXVT.HalfTwo ->
                                let srcTC = srcTex.[v.Tex]
                                bw.Write(MonoGameHelpers.floatToHalfUint16 srcTC.X)
                                bw.Write(MonoGameHelpers.floatToHalfUint16 srcTC.Y)
                            | MMET.Format(f) when f = SDXF.R32G32_Float ->
                                let srcTC = srcTex.[v.Tex]
                                bw.Write(srcTC.X)
                                bw.Write(srcTC.Y)
                            | _ -> failwithf "Unsupported type for texture coordinate: %A" el.Type
                        | MMVertexElemSemantic.Normal -> normalWriter modNrmIndex modVertIndex el bw
                        | MMVertexElemSemantic.Binormal
                        | MMVertexElemSemantic.Tangent -> binormalTangentWriter modNrmIndex modVertIndex el bw
                        | MMVertexElemSemantic.BlendIndices -> blendIndexWriter modVertIndex el bw
                        | MMVertexElemSemantic.BlendWeight -> blendWeightWriter modVertIndex el bw
                        | MMVertexElemSemantic.Color ->
                            // TODO: if/when snapshot & import/export write this out, will need to populate it here
                            match el.Type with
                            | MMET.DeclType(dt) when dt = SDXVT.Color ->
                                let bytes:byte[] = [|255uy;255uy;255uy;255uy|];
                                bw.Write(bytes);
                            | MMET.DeclType(dt) when dt = SDXVT.Float4 ->
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                            | MMET.Format(f) when f = SDXF.R32G32B32A32_Float ->
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                                bw.Write(1.f)
                            | _ -> failwithf "Unsupported type for Color: %A" el.Type
                        | _ -> failwithf "Unsupported semantic: %A" el.Semantic

                // Write a full vertex to the buffer.
                let writeVertex (v:PTNIndex) =
                    let startPos = bw.BaseStream.Position
                    let writeToVert = writeElement v
                    declElements |> Array.iter (fun el ->
                        // may need to seek to skip unused space in the vert.  only need to seek
                        // ahead because we sorted the elements by offset ascending above.
                        let currPos = bw.BaseStream.Position
                        let currOffset = currPos - startPos
                        let elOffset = int64 el.Offset
                        if elOffset > currOffset then
                            bw.BaseStream.Seek(elOffset - currOffset, SeekOrigin.Current) |> ignore
                        writeToVert el
                    )
                    if (bw.BaseStream.Position % (int64 vertSizeBytes) <> 0L) then
                        let bytesWrote = bw.BaseStream.Position - startPos
                        failwithf "Wrote an insufficient number of bytes for the vertex (wrote %A, want %A, possible offset/stride/vertsize issue)" bytesWrote vertSizeBytes

                // Write the three triangle verts to the buffer.
                let writeTriangle (tri:IndexedTri) = tri.Verts |> Array.iter writeVertex

                // Write all the triangles to the buffer.
                modm.Triangles |> Array.iter writeTriangle

                if int64 destVbSize <> bw.BaseStream.Position then
                    // uh oh
                    log.Warn "vb fill did not produce the expected number of bytes (want %A, got %A)" destVbSize bw.BaseStream.Position
            0
        with
            | e ->
                log.Error "%s" e.Message
                log.Error "%s" e.StackTrace
                InteropTypes.GenericFailureCode

    /// Fill the render buffers associated with the specified mod.
    let fillModData
        (modIndex:int)
        (destDeclData:nativeptr<byte>) (destDeclSize:int)
        (destVbData:nativeptr<byte>) (destVbSize:int)
        (destIbData:nativeptr<byte>) (destIbSize:int) =
            match CoreState.Context with
            | "d3d9" ->
                let declArg = WriteD3D9Decl((getBinaryWriter destDeclData destDeclSize), destDeclSize)
                let vertSize = None // detect from mod
                fillModDataInternalHelper
                    modIndex
                    declArg vertSize
                    (getBinaryWriter destVbData destVbSize) destVbSize
                    (getBinaryWriter destIbData destIbSize) destIbSize
            | "d3d11" ->
                try
                    // create vertex element description from declData buffer
                    let (elements,elStr) = d3d11ElementsFromPtr destDeclData destDeclSize
                    let declArg = ReadD3D11Layout(elements)
                    let vertSize = MeshUtil.getVertSizeFromEls elements
                    log.Info "filling d3d11 vertex buffer stream size: %A; vert size: %A, vert type id: %A" destVbSize vertSize (elStr.GetHashCode())
                    fillModDataInternalHelper modIndex declArg (Some(vertSize))
                        (getBinaryWriter destVbData destVbSize) destVbSize
                        (getBinaryWriter destIbData destIbSize) destIbSize
                with
                | e ->
                    log.Error "Exception while filling data: %A" e
                    InteropTypes.GenericFailureCode
            | _ ->
                log.Error "Fill not implemented for context: %A" CoreState.Context
                InteropTypes.GenericFailureCode

    // For FSI testing...
    let testFill (modIndex:int,destDecl:byte[],destVB:byte[],destIB:byte[]) =
        let declArg = WriteD3D9Decl((new BinaryWriter(new MemoryStream(destDecl))), destDecl.Length)
        let vertSize = None // detect from mod
        fillModDataInternalHelper
            modIndex
            declArg vertSize
            (new BinaryWriter(new MemoryStream(destVB))) destVB.Length
            (new BinaryWriter(new MemoryStream(destIB))) destIB.Length




