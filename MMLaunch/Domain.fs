// Slim domain types for the launcher. The full versions live in MMManaged but pull
// in MonoGame/SharpDX (.NET Framework only); the launcher only needs the registry-
// backed RunConfig/GameProfile records and a class shape compatible with
// SnapshotProfile descriptions.

namespace ModelMod

open System

module InputProfiles =
    let PunctRock = "PunctuationKeys"
    let FItUp = "FKeys"

    let ValidProfiles = [ PunctRock; FItUp ]

    let DefaultProfile = FItUp

    let isValid (profile: string) =
        ValidProfiles
        |> List.exists (fun p -> p.ToLowerInvariant() = profile.ToLowerInvariant())

module CoreTypes =
    type GameProfile = {
        ReverseNormals: bool
        UpdateTangentSpace: bool
        CommandLineArguments: string
        DataPathName: string
    }

    let DefaultGameProfile = {
        ReverseNormals = false
        UpdateTangentSpace = true
        CommandLineArguments = ""
        DataPathName = ""
    }

    type RunConfig = {
        ProfileKeyName: string
        ProfileName: string
        ExePath: string
        LoadModsOnStart: bool
        RunModeFull: bool
        InputProfile: string
        SnapshotProfile: string
        GameProfile: GameProfile
        DocRoot: string
        LaunchWindow: int
        MinimumFPS: int
    }

    let DefaultRunConfig = {
        ProfileKeyName = ""
        ProfileName = ""
        ExePath = ""
        RunConfig.RunModeFull = true
        LoadModsOnStart = true
        InputProfile = ""
        SnapshotProfile = ""
        GameProfile = DefaultGameProfile
        DocRoot = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.MyDocuments), "ModelMod")
        LaunchWindow = 15
        MinimumFPS = 28
    }

    let AdjustBlendWeightsDefault = "addx"

    /// Mirrors MMManaged's CoreTypes.SnapProfile shape; used for displaying snapshot
    /// profile descriptions in the launcher.
    type SnapProfile() =
        let mutable name: string = ""
        let mutable posX: ResizeArray<string> = new ResizeArray<string>()
        let mutable uvX: ResizeArray<string> = new ResizeArray<string>()
        let mutable flipTangent: bool = false
        let mutable vecEncoding: string = ""
        let mutable blendIndexInColor1: bool = false
        let mutable blendWeightInColor2: bool = false
        let mutable adjustBlendWeights: string = AdjustBlendWeightsDefault

        static member Create(name, posX, uvX, flipTangent, vecEncoding: string, blendIndexInColor1: bool, blendWeightInColor2: bool) : SnapProfile =
            let p = new SnapProfile()
            p.Name <- name
            p.PosXForm <- posX
            p.UVXForm <- uvX
            p.FlipTang <- flipTangent
            p.VecEncoding <- vecEncoding
            p.BlendIndexInColor1 <- blendIndexInColor1
            p.BlendWeightInColor2 <- blendWeightInColor2
            p

        member x.Name with get() = name and set v = name <- v
        member x.PosXForm with get() = posX and set v = posX <- v
        member x.UVXForm with get() = uvX and set v = uvX <- v
        member x.FlipTang with get() = flipTangent and set v = flipTangent <- v
        member x.VecEncoding with get() = vecEncoding and set v = vecEncoding <- v
        member x.BlendIndexInColor1 with get() = blendIndexInColor1 and set v = blendIndexInColor1 <- v
        member x.BlendWeightInColor2 with get() = blendWeightInColor2 and set v = blendWeightInColor2 <- v
        member x.AdjustBlendWeights with get() = adjustBlendWeights and set v = adjustBlendWeights <- v

        override x.ToString() =
            sprintf
                "[SnapshotProfile: %s; pos: %A; uv: %A, fliptangent: %A, vecencoding: %A, blendindexincolor1: %A, blendweightincolor2: %A, adjustblendweights: %A]"
                name posX uvX flipTangent vecEncoding blendIndexInColor1 blendWeightInColor2 adjustBlendWeights
