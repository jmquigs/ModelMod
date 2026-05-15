namespace ModelMod

open System 

module ConfigTypes =
    /// Contains the name of all available input profiles.  An input profile is just a set of keybindings for
    /// controlling ModelMod in games.  Different games and systems require different input layouts, so that
    /// ModelMod doesn't interfere too much with the game.  Some games make heavy use of the F keys, for instance,
    /// so the punctuation layout is a better choice.  Its expected that some games won't work well with either of
    /// these, and some new layouts will need to be defined.
    /// The managed code generally doesn't care about the precise definition of each layout, with the exception of
    /// The launcher app, which describes the layout in the UI.  Native code (specificaly RenderState.cpp) is
    /// responsible for actually setting up the bindings.
    module InputProfiles =
        let PunctRock = "PunctuationKeys"
        let FItUp = "FKeys"

        let ValidProfiles = [ PunctRock; FItUp ]

        let DefaultProfile = FItUp

        let isValid (profile:string) =
            ValidProfiles |> List.exists (fun p -> p.ToLowerInvariant() = profile.ToLowerInvariant())  
            
    /// Contains settings specific to a particular game.  Usually these settings relate to how a game lays out geometry data
    /// in D3D memory.
    type GameProfile = {
        /// Controls the order in which normal vector components are written to D3D buffers.
        /// False: XYZW; True: ZYXW
        ReverseNormals: bool
        /// Controls whether tangent space updates are globally enabled or disabled.  Whatever this setting is, mods
        /// can opt in or out by setting "UpdateTangentSpace" to true or false in their yaml files.
        UpdateTangentSpace: bool
        /// Command line arguments that should be passed to the game when launched.
        CommandLineArguments: string
        /// An alternate name for the game data directory data directory in case the exe base name does not map to any extant directory.
        /// Can also be a full absolute path.
        DataPathName: string
    }

    let DefaultGameProfile = {
        ReverseNormals = false
        UpdateTangentSpace = true
        CommandLineArguments = ""
        DataPathName = ""
    }

    /// A run config for modelmod.  These are stored in the registry.
    type RunConfig = {
        /// Reg key that this profile is stored under, e.g "Profile0000"
        ProfileKeyName: string
        /// Friendly name for profile (if missing, defaults to exe base name)
        ProfileName: string
        /// Path to exe
        ExePath: string
        /// If true, mods will be load and displayed on startup; otherwise they must be loaded
        /// and displayed manually with keyboard commands
        LoadModsOnStart: bool
        /// Whether the current/next run is in full (snapshot) mode or playback only.  This is really for
        /// future use, since right now we don't have a separate "playback" mode (the idea is that we could be
        /// slightly more efficient by disabling certain things, like shader constant tracking,
        /// when we know we are only playing back mods).
        RunModeFull: bool
        /// Input profile (i.e.: input key layout)
        InputProfile: string
        /// Snapshot profile (i.e.: model transforms for snapshot)
        SnapshotProfile: string
        /// Game profile
        GameProfile: GameProfile
        /// Doc root for this profile.  Currently ignored.
        DocRoot: string
        /// Period of time that the Loader will wait for the game to start before exiting.
        LaunchWindow: int
        /// MinimumFPS desired.  Below this number, modelmod will temporarily shut off mod rendering in an effort to improve FPS.
        MinimumFPS: int
    }

    /// When no run configuration is available in the registry, this is what is used.  The Input and Snapshot
    /// modules define their own defaults.  The default DocRoot is <MyDocuments>\ModelMod.
    let DefaultRunConfig = {
        ProfileKeyName = ""
        ProfileName = ""
        ExePath = ""
        RunConfig.RunModeFull = true
        LoadModsOnStart = true
        InputProfile = ""
        SnapshotProfile = ""
        GameProfile = DefaultGameProfile
        DocRoot = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.MyDocuments),"ModelMod")
        LaunchWindow = 15
        MinimumFPS = 28
    }

    /// Value for AdjustBlendWeights that applies the blend weight sum-to-1.0 fix (adds deficit to X component).
    let AdjustBlendWeightsDefault = "addx"

    /// A snapshot profile controls what types of data transformations
    /// (typically vertex position and uv coordinates) that are applied by the snapshotter.  These are typically used to
    /// position the snapshotted mesh in a location that is convenient for use in a 3D tool (therefore, different tools may
    /// need different profiles for the same game).  The transforms are automatically reversed on load so that the data is
    /// in the correct space for the game.
    /// More recently the profile has been extended to specify how certain parts of the mesh data (e.g tangent space vectors)
    /// should be interpreted, both during snapshot and mod load.
    type SnapProfile() =
        let mutable name:string = "";
        let mutable posX:ResizeArray<String> = new ResizeArray<string>();
        let mutable uvX:ResizeArray<String> = new ResizeArray<string>();
        let mutable flipTangent:bool = false;
        let mutable vecEncoding:string = "";
        let mutable blendIndexInColor1:bool = false;
        let mutable blendWeightInColor2:bool = false;
        let mutable adjustBlendWeights:string = AdjustBlendWeightsDefault;

        static member Create(name,posX,uvX,flipTangent,vecEncoding:string,blendIndexInColor1:bool,blendWeightInColor2:bool):SnapProfile =
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

        member x.IsPackedVec() = vecEncoding.Trim().ToLowerInvariant() = "packed"
        member x.IsOctaVec() = vecEncoding.Trim().ToLowerInvariant() = "octa"
        override x.ToString() =
            sprintf "[SnapshotProfile: %s; pos: %A; uv: %A, fliptangent: %A, vecencoding: %A, blendindexincolor1: %A, blendweightincolor2: %A, adjustblendweights: %A]" name posX uvX flipTangent vecEncoding blendIndexInColor1 blendWeightInColor2 adjustBlendWeights

    let EmptySnapProfile = SnapProfile.Create("",new ResizeArray<string>(),new ResizeArray<string>(),false,"",false,false)

    /// This profile should always exist in SnapshotProfiles.yaml.
    /// If it does not, new game profiles will be created with an empty snapshot profile (not an error, but not desirable either)
    let DefaultSnapProfileName = "Profile1"