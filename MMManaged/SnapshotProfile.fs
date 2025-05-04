namespace ModelMod

open FSharp.Core

open System
open System.IO

/// A snapshot profile controls what types of data transformations
/// (typically vertex position and uv coordinates) that are applied by the snapshotter.  These are typically used to
/// position the snapshotted mesh in a location that is convenient for use in a 3D tool (therefore, different tools may
/// need different profiles for the same game).  The transforms are automatically reversed on load so that the data is
/// in the correct space for the game.
/// SnapshotProfiles are loaded from yaml files in the "SnapshotProfiles" subdirectory of the modelmod installation folder.
module SnapshotProfile =
    type Profile(name,posX,uvX,flipTangent,vecEncoding) =
        member x.Name() = name
        member x.PosXForm() = posX
        member x.UVXForm() = uvX
        member x.FlipTang() = flipTangent
        member x.VecEncoding() = vecEncoding

        override x.ToString() =
            sprintf "[SnapshotProfile: %s; pos: %A; uv: %A, fliptangent: %A, vecencoding: %A]" name posX uvX flipTangent vecEncoding

    let EmptyProfile = Profile("",[],[],false,"")

    /// This profile should always exist in SnapshotProfiles.yaml.
    /// If it does not, new game profiles will be created with an empty snapshot profile (not an error, but not desirable either)
    let DefaultProfileName = "Profile1"

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
                    for p in rootMap.Children do
                        let pname = p.Key |> Yaml.toString
                        let pvals = p.Value |> Yaml.toMapping "expected a mapping"

                        let getStrList def key =
                            pvals
                            |> Yaml.getOptionalValue key
                            |> Yaml.toOptionalSequence
                            |> function
                                | None -> def
                                | Some s -> s |> Seq.map Yaml.toString |> List.ofSeq

                        let getBool def key = 
                            pvals 
                            |> Yaml.getOptionalValue key 
                            |> Yaml.toOptionalBool 
                            |> function 
                                | None -> def
                                | Some b -> b

                        let getString def key = 
                            pvals 
                            |> Yaml.getOptionalValue key
                            |> Yaml.toOptionalString
                            |> function 
                                | None -> def
                                | Some s -> s
                            

                        let posX = getStrList [] "pos"
                        let uvX = getStrList [] "uv"
                        let flipTang = getBool false "flipTangent"
                        let vecEncoding = getString "" "vecEncoding"

                        profiles.Add(pname,Profile(pname,posX,uvX,flipTang,vecEncoding))
            )
        profiles.ToArray() |> Map.ofArray

