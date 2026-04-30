namespace ModelMod

open System.IO

open CoreTypes

/// Types and helpers shared by the various binary disk caches
/// (MeshDiskCache, MeshRelDiskCache, VBDataDiskCache).  Lifted here so
/// that the same MeshSig record describes a mesh source file across all
/// of them.
module BinCacheTypes =

    /// Identifies a mesh source file plus the parameters that govern how it
    /// gets parsed/transformed.  Used as a freshness check on cache entries.
    type MeshSig = {
        Path: string
        Ticks: int64
        Size: int64
        ModType: ModType
        Flags: MeshReadFlags
    }

    let fileSig (path: string) =
        let fi = FileInfo(path)
        fi.LastWriteTimeUtc.Ticks, fi.Length

    let mkSig (path: string) (modType: ModType) (flags: MeshReadFlags) : MeshSig =
        let t,s = fileSig path
        { Path = path; Ticks = t; Size = s; ModType = modType; Flags = flags }
