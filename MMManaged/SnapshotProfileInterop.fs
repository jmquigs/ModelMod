namespace ModelMod

open FSharp.Core

open System
open System.IO

open ConfigTypes

/// SnapshotProfiles are loaded from yaml files in the "SnapshotProfiles" subdirectory of the modelmod installation folder.
/// This module serializes it to a native interop structure.
/// It is separate from the loader (`SnapshotProfileLoad`) so that MMLaunch can include that directly; it doesn't need this 
/// and can't include it since this depends indirectly on CoreTypes and therefore SharpDX/Monogame which don't build under 
/// MMLaunch
module SnapshotProfileInterop =
    let private log = Logging.getLogger("SnapshotProfileInterop")

    let toInteropStruct (profile: SnapProfile): InteropTypes.ModSnapProfile =
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
                BlendIndexInColor1 = profile.BlendIndexInColor1
                BlendWeightInColor2 = profile.BlendWeightInColor2
            }
        else
            log.Error "Profile has too many transforms and will be marked as invalid."
            {
                InteropTypes.EmptyModSnapProfile with
                    Valid = false
                    Name = trimmedName
            }
        


