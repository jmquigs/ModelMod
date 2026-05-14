// Slim SnapshotProfile loader for the launcher (the engine version is in
// MMManaged/SnapshotProfile.fs and additionally produces interop structs).

namespace ModelMod

open System
open System.IO

open CoreTypes

module SnapshotProfile =
    let private log = Logging.getLogger("SnapshotProfile")

    let DefaultProfileName = "Profile1"

    let EmptyProfile =
        SnapProfile.Create("", new ResizeArray<string>(), new ResizeArray<string>(), false, "", false, false)

    let private loadSingleProfile (name: string) (values: YamlDotNet.RepresentationModel.YamlMappingNode) =
        let getStrArray def key =
            values
            |> Yaml.getOptionalValue key
            |> Yaml.toOptionalSequence
            |> function
                | None -> def
                | Some s -> s |> Seq.map Yaml.toString |> Array.ofSeq |> fun a -> new ResizeArray<string>(a)

        let mutable posX = getStrArray (new ResizeArray<string>()) "pos"
        if posX.Count = 0 then
            posX <- getStrArray (new ResizeArray<string>()) "PosXForm"

        let mutable uvX = getStrArray (new ResizeArray<string>()) "uv"
        if uvX.Count = 0 then
            uvX <- getStrArray (new ResizeArray<string>()) "UVXForm"

        let mutable flipTang = values |> Yaml.getOptionalBool "flipTangent"
        if flipTang.IsNone then
            flipTang <- values |> Yaml.getOptionalBool "FlipTang"
        let flipTang = Option.defaultValue false flipTang

        let vecEncoding = values |> Yaml.getOptionalString "vecEncoding" |> Option.defaultValue ""
        let blendIndexInColor1 = values |> Yaml.getOptionalBool "BlendIndexInColor1" |> Option.defaultValue false
        let blendWeightInColor2 = values |> Yaml.getOptionalBool "BlendWeightInColor2" |> Option.defaultValue false
        let adjustBlendWeights = values |> Yaml.getOptionalString "AdjustBlendWeights" |> Option.defaultValue AdjustBlendWeightsDefault
        let pname = values |> Yaml.getOptionalString "Name" |> Option.defaultValue name

        let p = SnapProfile.Create(pname, posX, uvX, flipTang, vecEncoding, blendIndexInColor1, blendWeightInColor2)
        p.AdjustBlendWeights <- adjustBlendWeights
        p

    let GetAll (rootDir: string) : Map<string, SnapProfile> =
        let pDir = Path.Combine(rootDir, "SnapshotProfiles")
        if not (Directory.Exists pDir) then
            failwithf "Profile directory does not exist %A" pDir

        let profiles = new ResizeArray<_>()

        Directory.GetFiles pDir
        |> Array.filter (fun fn -> Path.GetExtension(fn).ToLowerInvariant() = ".yaml")
        |> Array.iter (fun fn ->
            let docs = Yaml.load fn

            for d in docs do
                let rootMap = Yaml.toMapping "expected a sequence" d.RootNode

                for p in rootMap.Children do
                    let pname = p.Key |> Yaml.toString
                    let pvals = p.Value |> Yaml.toMapping "expected a mapping"
                    let prof = loadSingleProfile pname pvals
                    profiles.Add(pname, prof))

        profiles.ToArray() |> Map.ofArray

/// Minimal Snapshot.SnapMeta surface used by ModUtil for reading/writing
/// snapshot meta yaml files. The full Snapshot module in MMManaged also
/// contains the actual snapshot-writing pipeline; the launcher only needs
/// the metadata record shape.
module Snapshot =
    type SnapMeta() =
        let mutable profile: CoreTypes.SnapProfile = SnapshotProfile.EmptyProfile
        let mutable context: string = ""
        let mutable vbChecksumAlgo: string = ""
        let mutable vbChecksum: string = ""

        member x.Profile with get () = profile and set v = profile <- v
        member x.Context with get () = context and set v = context <- v
        member x.VBChecksumAlgo with get () = vbChecksumAlgo and set v = vbChecksumAlgo <- v
        member x.VBChecksum with get () = vbChecksum and set v = vbChecksum <- v
