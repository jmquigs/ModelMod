// Slim SnapshotProfile loader for the launcher (the engine version is in
// MMManaged/SnapshotProfile.fs and additionally produces interop structs).

namespace ModelMod

open System
open System.IO

open ConfigTypes

/// Minimal Snapshot.SnapMeta surface used by ModUtil for reading/writing
/// snapshot meta yaml files. The full Snapshot module in MMManaged also
/// contains the actual snapshot-writing pipeline; the launcher only needs
/// the metadata record shape.
module Snapshot =
    type SnapMeta() =
        let mutable profile: ConfigTypes.SnapProfile = ConfigTypes.EmptySnapProfile
        let mutable context: string = ""
        let mutable vbChecksumAlgo: string = ""
        let mutable vbChecksum: string = ""

        member x.Profile with get () = profile and set v = profile <- v
        member x.Context with get () = context and set v = context <- v
        member x.VBChecksumAlgo with get () = vbChecksumAlgo and set v = vbChecksumAlgo <- v
        member x.VBChecksum with get () = vbChecksum and set v = vbChecksum <- v
