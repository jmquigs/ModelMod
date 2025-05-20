// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace ModelMod

open System
open SharpDX.Direct3D9

// Shorthand type defs

type SDXVertexElement = SharpDX.Direct3D9.VertexElement // TODO11
type SDXVertexDeclUsage = SharpDX.Direct3D9.DeclarationUsage // TODO11
type SDXVertexDeclType = SharpDX.Direct3D9.DeclarationType // TODO11

module CoreState = 
    // Run time DLL context, set by Interop.Main
    let mutable Context = ""

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

module CoreTypes =
    // ------------------------------------------------------------------------

    /// Shorthand for Microsoft.Xna.Framework.Vector2
    type Vec2F = Microsoft.Xna.Framework.Vector2

    /// Shorthand for Microsoft.Xna.Framework.Vector3
    type Vec3F = Microsoft.Xna.Framework.Vector3

    /// Shorthand for Microsoft.Xna.Framework.Vector4
    type Vec4F = Microsoft.Xna.Framework.Vector4

    /// A 4-element vector of whatever.  Handy for keeping around data baggage, but doesn't define any
    /// normal vector ops (addition, dot product, etc).
    type Vec4X(x,y,z,w) =
        member v.X = x
        member v.Y = y
        member v.Z = z
        member v.W = w

    // ------------------------------------------------------------------------
    // Configuration types

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

    // ------------------------------------------------------------------------
    // Mod and ref data

    /// List of available mod types.  Really this is "file type", since a reference isn't really a mod (though you can
    /// define things like vertex group exclusions in a reference).
    /// "Replacement" mods replace the original game data; "Addition" mods draw the mod on top of the original game data.
    type ModType =
        /// Animated on the GPU; mod is expected to contain at least blend index data and usually blend weights as well.
        /// Mesh data is also usually not scaled or translated in world space in any way.
        GPUReplacement
        /// Animated on GPU as with GPUReplacement, however the original mesh is also drawn, so the mod is "added" to it.
        | GPUAdditive
        /// Animated on the CPU.  A snapshot of this kind of data usually results in a fully world-transformed and
        /// animated mesh - pretty useless for modding.  ModelMod doesn't not currently support this type of mod,
        /// even though it is technically possible.
        | CPUReplacement
        /// Removal mod.  These don't define meshes, but instead just list a primitive and vertex count.  Whenever
        /// _anything_ is drawn with that exact primitive and vert count, it is not displayed.  This can lead to some
        /// artifacts (especially with things like particle emitters which have very regular and common low-numbered
        /// vert/primitive counts), so should be used with care.
        | Deletion
        /// Reference.  Generally you don't want to touch the reference file.  However, it is possible to change it
        /// so that you can do vertex inclusion/exclusion groups.
        | Reference

    /// These types control how weighting is done in MeshRelation, in particular where the
    /// blend indices and blend weights are expected to be found.
    type WeightMode =
        /// Get blend data from mod mesh.  Mod author must ensure that all verts are propertly
        /// weighted in the 3d tool.  This can be tedious, especially with symmetric parts, so this
        /// mode is primarily here for advanced users and control freaks.
        Mod

        /// Get blend data from the ref.  This is the default and easiest mode to use.
        | Ref
        /// Get blend data from the binary ref data.  This is mostly a developer debug mode - it doesn't
        /// support vertex annotation group filtering, amongst other limitations.
        | BinaryRef

    /// Record which contains indices for position, texture coordinate and normal.  Useful for obj-style meshes.
    type PTNIndex = { Pos: int; Tex: int; Nrm: int }

    /// A triangle composed of PTNIndex recrods.
    type IndexedTri = {
        /// Always 3 elements long
        Verts: PTNIndex[]
    }

    /// The vertex declaration specifies the D3D vertex layout; it is usually captured at snapshot
    /// time, and then used at playback time to create the required format.  Generally you shouldn't modify it
    /// Both the raw bytes and the "unpacked" list are included in this type.
    type VertexDeclarationData = byte[] * SDXVertexElement list

    /// This allows raw vertex data from a snapshot to be reloaded and displayed.  Debug feature.
    type BinaryVertexData = {
        NumVerts: uint32
        Stride: uint32
        Data: byte[]
    }

    /// Various options for reading meshes.
    type MeshReadFlags = {
        /// Whether to read any associated .mtl files.  If true and an .mtl file is available, Tex0Path
        /// will be set to whatever texture is defined in the material file.  This is primarily intended for tools;
        /// Mods only respect texture overrides that are definied in the mod .yaml file.
        ReadMaterialFile: bool
        /// Whether to reverse snapshot transforms on load.  Since meshes in tool-space
        /// are generally useless in game, this usually always
        /// happens.  It can be useful to turn it off in tools, so that the mod is displayed in post-snapshot
        /// space (the same view that the tool will see).  The launcher preview window, for instance, turns this off.
        ReverseTransform: bool
    }

    /// Default read flags; used when no overriding flags are specified.
    let DefaultReadFlags = {
        ReadMaterialFile = false
        ReverseTransform = true
    }

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

        static member Create(name,posX,uvX,flipTangent,vecEncoding:string):SnapProfile = 
            let p = new SnapProfile()
            p.Name <- name
            p.PosXForm <- posX 
            p.UVXForm <- uvX
            p.FlipTang <- flipTangent
            p.VecEncoding <- vecEncoding
            p

        member x.Name with get() = name and set v = name <- v
        member x.PosXForm with get() = posX and set v = posX <- v 
        member x.UVXForm with get() = uvX and set v = uvX <- v
        member x.FlipTang with get() = flipTangent and set v = flipTangent <- v
        member x.VecEncoding with get() = vecEncoding and set v = vecEncoding <- v

        member x.IsPackedVec() = vecEncoding.Trim().ToLowerInvariant() = "packed"
        member x.IsOctaVec() = vecEncoding.Trim().ToLowerInvariant() = "octa"

        override x.ToString() =
            sprintf "[SnapshotProfile: %s; pos: %A; uv: %A, fliptangent: %A, vecencoding: %A]" name posX uvX flipTangent vecEncoding

    /// Basic storage for everything that we consider to be "mesh data".  This is intentionally pretty close to the
    /// renderer level; i.e. we don't have fields like "NormalMap" because the texture stage used for will vary
    /// across games or even within the same game.  Generally if you want to customize a texture its up to you to make
    /// sure its installed on the correct stage.
    type Mesh = {
        /// The type of the Mesh
        Type : ModType
        /// Array of indexed triangles; the indexes are for the Positions, UVs and Normals fields.
        Triangles : IndexedTri[]
        /// Array of positions.
        Positions: Vec3F[]
        /// Array of primary texture coordinate.  At the moment only one set of UVs is supported.
        UVs: Vec2F[]
        /// Array of normals.  Assumed to be normalized (by the 3D tool hopefully).
        Normals: Vec3F[]
        /// Array of blend indices.
        BlendIndices: Vec4X[]
        /// Array of blend weights.
        BlendWeights: Vec4F[]
        /// Vertex declaration; though this is optional, it is required for anything that will be displayed.
        Declaration : VertexDeclarationData option
        /// BinaryVertexData is usually not used.
        BinaryVertexData: BinaryVertexData option
        /// List of custom vertex group names, for use with vertex group inclusions/exclusions.
        AnnotatedVertexGroups: string list []
        /// List of position transforms that have been applied to this mesh.  Derived from snapshot transform profile.
        AppliedPositionTransforms: string []
        /// List of uv transforms that have been applied.  Derived from snapshot transform profile.
        AppliedUVTransforms: string[]
        /// Texture 0 path.  Generally only set if an override texture is being used, though the mesh read flags can affect this.
        Tex0Path: string
        /// Texture 1 path.  Generally only set if an override texture is being used, though the mesh read flags can affect this.
        Tex1Path: string
        /// Texture 2 path.  Generally only set if an override texture is being used, though the mesh read flags can affect this.
        Tex2Path: string
        /// Texture 3 path.  Generally only set if an override texture is being used, though the mesh read flags can affect this.
        Tex3Path: string
        /// Whether this mesh instance was loaded from disk or the cache on the last load
        Cached: bool
    }

    // ------------------------------------------------------------------------
    // These are types loaded by the moddb from yaml files

    /// Storage for a named Reference object.
    /// The Name of a reference is its base file name (no extension).
    type DBReference = {
        Name : string
        Mesh : Mesh
        PrimCount: int
        VertCount: int
    }

    /// Storage for a Deletion mod.
    type GeomDeletion = { PrimCount: int; VertCount: int }

    /// Other than base data, this contains additional data that can be set by a mod in the yaml file.
    type ModAttributes = {
        DeletedGeometry: GeomDeletion list
    }

    /// Default value
    let EmptyModAttributes = { ModAttributes.DeletedGeometry = [] }

    /// Storage for a named mod.
    /// The Name of a mod is its base file name (no extension).
    type DBMod = {
        RefName: string option
        Ref: DBReference option
        Name: string
        Mesh: Mesh option
        WeightMode: WeightMode
        PixelShader: string
        Attributes: ModAttributes
        ParentModName: string option
        /// Whether to update the tangent space (tangent and bitangent vectors) of the mod.  MM currently doesn't load these, and the
        /// python exporter doesn't export them.
        /// A default binormal/tangent is generated on a fixed coordinate axis without using texture data.  This generates results
        /// that are ok in some cases, bad in others.
        /// As of v1.2, MM will, by default, try to generate updated tangents and bitangent vectors the proper way using DirectXMesh.
        /// In general these look better, but in some cases they don't (in particular meshes that use left-right symmetric UV coordinates
        /// can have artifacts in some faces).
        /// Setting this to false disables the update (and thus the fixed coordinate axis will be used).  Setting this to true always
        /// regenerates even if tangent space is globally disabled in the game profile.  When left unspecified the global default is used.
        UpdateTangentSpace: bool option
        /// Snapshot profile; optional since many older mods will not have this.
        Profile: SnapProfile option
    }

    /// Union Parent type for the yaml objects.
    type ModElement =
        Unknown
        | MReference of DBReference
        | Mod of DBMod

/// Contains an abstract vertex element type.  DX9 uses a vertex declaration, while DX11 uses a format.
/// `MMVertexElement` is used to abstract that difference.  Note that the `MMVertexElementType` does
/// not abstract the type, rather just passes it through.
/// There are just too many and its not clear how d3d9 types
/// map into d3d11 types if at all.
module VertexTypes =
    let private log() = Logging.getLogger("VertexTypes")

    /// The list of semantics modelmod cares about even a little (color not important).
    type MMVertexElemSemantic =
        | Position = 0
        | Normal = 1
        | TextureCoordinate = 2
        | BlendWeight = 3
        | BlendIndices = 4
        | Binormal = 5
        | Tangent = 6
        | Color = 8
        | Unknown = 9

    type MMVertexElementType =
        /// Decl types are used in DX9
        DeclType of SDXVertexDeclType
        /// Formats are used in DX11
        | Format of SharpDX.DXGI.Format

    /// Represents an vertex element converted from a d3d9 declaration or d3d11 input element/layout.
    type MMVertexElement = {
        Semantic: MMVertexElemSemantic
        SemanticIndex: int
        Type: MMVertexElementType
        Offset: int
        Slot: int
    }
    let elSemanticNameToDeclSemantic =
        Map.ofList [
            "POSITION", MMVertexElemSemantic.Position
            "BLENDWEIGHT", MMVertexElemSemantic.BlendWeight
            "BLENDINDICES", MMVertexElemSemantic.BlendIndices
            "NORMAL", MMVertexElemSemantic.Normal
            "TEXCOORD", MMVertexElemSemantic.TextureCoordinate
            "TANGENT", MMVertexElemSemantic.Tangent
            "BITANGENT", MMVertexElemSemantic.Binormal
            "BINORMAL", MMVertexElemSemantic.Binormal
            "COLOR", MMVertexElemSemantic.Color
        ]
    let sdxDeclUsageToMMDeclUsage (usage:SDXVertexDeclUsage) =
        match usage with
        | SDXVertexDeclUsage.Position -> MMVertexElemSemantic.Position
        | SDXVertexDeclUsage.Normal -> MMVertexElemSemantic.Normal
        | SDXVertexDeclUsage.TextureCoordinate -> MMVertexElemSemantic.TextureCoordinate
        | SDXVertexDeclUsage.BlendWeight -> MMVertexElemSemantic.BlendWeight
        | SDXVertexDeclUsage.BlendIndices -> MMVertexElemSemantic.BlendIndices
        | SDXVertexDeclUsage.Binormal -> MMVertexElemSemantic.Binormal
        | SDXVertexDeclUsage.Tangent -> MMVertexElemSemantic.Tangent
        | SDXVertexDeclUsage.Color -> MMVertexElemSemantic.Color
        | _ ->
            log().Warn "unrecognized usage %A, using UNKNOWN" usage
            MMVertexElemSemantic.Unknown

    let layoutElToMMEl (el:SharpDX.Direct3D11.InputElement) (elName:string): MMVertexElement =
        let declUsage =
            match Map.tryFind elName elSemanticNameToDeclSemantic with
            | None ->
                log().Warn "unrecognized semantic %A, using UNKNOWN" elName
                MMVertexElemSemantic.Unknown
            | Some(u) -> u

        { MMVertexElement.Semantic = declUsage
          SemanticIndex = el.SemanticIndex
          Type = MMVertexElementType.Format(el.Format)
          Offset = el.AlignedByteOffset
          Slot = el.Slot
        }

    let sdxDeclElementToMMDeclElement (el:SDXVertexElement) =
        { MMVertexElement.Semantic = sdxDeclUsageToMMDeclUsage el.Usage
          SemanticIndex = int el.UsageIndex
          Type = MMVertexElementType.DeclType el.Type
          Offset = int el.Offset
          Slot = int el.UsageIndex
        }