namespace ModelMod

open FSharp.Core

open System
open System.IO

open ConfigTypes
open Yaml

/// SnapshotProfiles are loaded from yaml files in the "SnapshotProfiles" subdirectory of the modelmod installation folder.
module SnapshotProfileLoad =
    let private log = Logging.getLogger("SnapshotProfileLoad")

    let loadSingleProfile (name:string) (values:YamlDotNet.RepresentationModel.YamlMappingNode) = 
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

        let mutable vecEncoding = values |> Yaml.getOptionalString "vecEncoding"
        let vecEncoding = Option.defaultValue "" vecEncoding

        let blendIndexInColor1 = values |> Yaml.getOptionalBool "BlendIndexInColor1" |> Option.defaultValue false
        let blendWeightInColor2 = values |> Yaml.getOptionalBool "BlendWeightInColor2" |> Option.defaultValue false

        let adjustBlendWeights = values |> Yaml.getOptionalString "AdjustBlendWeights" |> Option.defaultValue AdjustBlendWeightsDefault

        let pname = values |> Yaml.getOptionalString "Name" |> Option.defaultValue name

        let p = SnapProfile.Create(pname,posX,uvX,flipTang,vecEncoding,blendIndexInColor1,blendWeightInColor2)
        p.AdjustBlendWeights <- adjustBlendWeights
        p

    /// Returns a map of all available profiles.  Throws exception if the profiles cannot be loaded.
    let GetAll rootDir =
        let pDir = Path.Combine(rootDir, "SnapshotProfiles")
        if not (Directory.Exists(pDir)) then
            failwithf "Profile directory does not exist %A" pDir
        let profiles = new ResizeArray<_>()

        Directory.GetFiles(pDir)
            |> Array.filter (fun fn -> Path.GetExtension(fn).ToLowerInvariant() = ".yaml")
            |> Array.iter (fun fn ->
                let docs = Yaml.load fn

                for d in docs do
                    let rootMap = Yaml.toMapping "expected a sequence" d.RootNode
                    // each file can contain multiple profiles, walk each
                    for p in rootMap.Children do
                        let pname = p.Key |> Yaml.toString
                        let pvals = p.Value |> Yaml.toMapping "expected a mapping"
                        let prof = loadSingleProfile pname pvals
                        profiles.Add(pname,prof)
            )
        profiles.ToArray() |> Map.ofArray