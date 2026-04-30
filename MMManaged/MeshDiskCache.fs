namespace ModelMod

open System
open System.Text
open System.IO
open System.Collections.Generic

open Microsoft.FSharp.Core

open YamlDotNet.RepresentationModel
open Microsoft.Xna.Framework

open CoreTypes

/// Utility module to cache full meshes to disk so that startup doesn't have to
/// re-parse mmobj files when nothing has changed.  Modeled on MeshRelDiskCache
/// in MeshRelation.fs.  See comments there about FsPickler / FSharp.Core
/// version sensitivity.
module private MeshDiskCache =
    let private log() = Logging.getLogger("MeshDiskCache")

    open System.IO.Compression
    open MBrace.FsPickler

    open BinCacheTypes

    let private mlog = Logging.getLogger("MeshDiskCache")
    let private ser = FsPickler.CreateBinarySerializer()
    let private cacheVersion = 2

    type Entry = {
        Version: int
        Sig: MeshSig
        Mesh: Mesh
    }

    let private key (path: string) =
        let s = path.ToLowerInvariant()
        use sha = System.Security.Cryptography.SHA256.Create()
        sha.ComputeHash(System.Text.Encoding.UTF8.GetBytes(s))
        |> Seq.map (fun b -> b.ToString("x2"))
        |> String.concat ""

    let relPath (cacheDir: string) (path: string) =
        Path.Combine(cacheDir, "Meshes", key path + ".bin.gz")

    let tryLoad (cacheDir: string) (path: string) (modType: ModType) (flags: MeshReadFlags) : Mesh option =
        let p = relPath cacheDir path
        if not (File.Exists(p)) then None
        else
            use fs = File.OpenRead(p)
            use gz = new GZipStream(fs, CompressionMode.Decompress)
            let e = ser.Deserialize<Entry>(gz)

            if e.Version <> cacheVersion then None
            else
                let cur = mkSig path modType flags
                if e.Sig = cur then Some e.Mesh else None

    let save (cacheDir: string) (path: string) (modType: ModType) (flags: MeshReadFlags) (mesh: Mesh) =
        let dir = Path.Combine(cacheDir, "Meshes")
        Directory.CreateDirectory(dir) |> ignore
        let p = relPath cacheDir path
        let tmp = p + ".tmp"

        mlog.Info "[meshcache]: creating bincache entry: %A for mesh=%A" tmp path

        let e =
            {
                Entry.Version = cacheVersion
                Sig = mkSig path modType flags
                Mesh = mesh
            }

        use fs = File.Create(tmp)
        use gz = new GZipStream(fs, CompressionMode.Compress)
        ser.Serialize(gz, e)

        if File.Exists(p) then File.Delete(p)
        File.Move(tmp, p)

    /// Load specified mesh, bypassing the cache.
    let loadUncachedMesh(path, (modType:ModType), flags) =
        let mesh = MeshUtil.readFrom(path, modType, flags)
        if flags.ReverseTransform &&
            (mesh.AppliedPositionTransforms.Length > 0 || mesh.AppliedUVTransforms.Length > 0) then
            let mesh = MeshTransform.reverseMeshTransforms (mesh.AppliedPositionTransforms) (mesh.AppliedUVTransforms) mesh
            // clear out applied transforms, since they have been reversed.
            { mesh with AppliedPositionTransforms = [||]; AppliedUVTransforms = [||] }
        else
            mesh

    /// Load specified mesh, using cached version if available.  binCacheDir, if non-empty,
    /// enables a binary disk cache that short-circuits mmobj parsing on startup when the source
    /// file is unchanged.
    let loadMesh(path, (modType:ModType), flags, (binCacheDir:string)) =
        match MemoryCache.get (path,modType,flags) with
        | Some mesh ->
            { mesh with Cached = true }
        | None ->
            let useDiskCache = not (System.String.IsNullOrWhiteSpace binCacheDir)
            let diskHit =
                if useDiskCache then
                    try
                        use sw = new Util.StopwatchTracker(sprintf "load mesh disk cache: %s" path)
                        tryLoad binCacheDir path modType flags
                    with e ->
                        log().Error "%A" e
                        None
                else None
            match diskHit with
            | Some mesh ->
                log().Info "[meshcache]: loaded mesh from cache: %s" path
                // populate the in-memory cache so future Ctrl-F1 reloads stay fast without
                // touching disk again
                MemoryCache.save(path, modType, flags, mesh)
                { mesh with Cached = true }
            | None ->
                let mesh = loadUncachedMesh(path, modType, flags)
                MemoryCache.save(path, modType, flags, mesh)
                if useDiskCache then
                    try save binCacheDir path modType flags mesh
                    with e -> log().Error "%A" e
                { mesh with Cached = false }
