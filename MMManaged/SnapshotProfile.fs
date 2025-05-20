namespace ModelMod

open FSharp.Core

open System
open System.IO

open CoreTypes

/// SnapshotProfiles are loaded from yaml files in the "SnapshotProfiles" subdirectory of the modelmod installation folder.
module SnapshotProfile =
    let private log = Logging.getLogger("SnapshotProfile")

    let EmptyProfile = SnapProfile.Create("",new ResizeArray<string>(),new ResizeArray<string>(),false,"")

    /// This profile should always exist in SnapshotProfiles.yaml.
    /// If it does not, new game profiles will be created with an empty snapshot profile (not an error, but not desirable either)
    let DefaultProfileName = "Profile1"

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

        let pname = values |> Yaml.getOptionalString "Name" |> Option.defaultValue name 

        SnapProfile.Create(pname,posX,uvX,flipTang,vecEncoding)

    let toInteropStruct (profile: CoreTypes.SnapProfile): InteropTypes.ModSnapProfile =
        // Constants for field size limits
        let maxSnapProfileStrLength = 255

        // Helper function to trim a string and log a warning if exceeds the max length
        let mutable somethingTrimmed = false
        let trimAndLog maxSize fieldName (s: string) =
            if s.Length > maxSize then
                somethingTrimmed <- true
                log.Error "Field '%s' exceeded max length of %d characters." fieldName maxSize
                s.Substring(0, maxSize)
            else
                s

        // Trim the necessary fields
        let trimmedName = trimAndLog maxSnapProfileStrLength "Name" profile.Name
        let trimmedVecEncoding = trimAndLog maxSnapProfileStrLength "VecEncoding" profile.VecEncoding

        // Transformations for PosX and UVX
        let processTransforms transforms maxLength =
            transforms
            |> Seq.map InteropTypes.makeXFormString
            |> Seq.toArray
                    |> fun arr ->
                        if arr.Length < maxLength then
                            // Create an array of empty transform strings to fill up to the max length
                            let fillArray = Array.init (maxLength - arr.Length) (fun _ -> InteropTypes.makeXFormString "")
                            // Concatenate the two arrays
                            Array.append arr fillArray
                        else
                            arr

        let posXTransformed = processTransforms profile.PosXForm InteropTypes.MaxModSnapProfileXFormLen
        let uvXTransformed = processTransforms profile.UVXForm InteropTypes.MaxModSnapProfileXFormLen

        // Check if any transform exceeds the capacity
        let isValid =
            not somethingTrimmed && 
            posXTransformed.Length <= InteropTypes.MaxModSnapProfileXFormLen &&
            uvXTransformed.Length <= InteropTypes.MaxModSnapProfileXFormLen

        // If valid, build the struct; otherwise, return an invalid struct
        if isValid then
            {
                Valid = true
                Name = trimmedName
                PosXLength = posXTransformed.Length
                PosX = posXTransformed
                UVXLength = uvXTransformed.Length
                UVX = uvXTransformed
                FlipTangent = profile.FlipTang
                VecEncoding = trimmedVecEncoding
            }
        else
            log.Error "Profile has too many transforms and will be marked as invalid."
            {
                InteropTypes.EmptyModSnapProfile with
                    Valid = false
                    Name = trimmedName
            }
        
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

