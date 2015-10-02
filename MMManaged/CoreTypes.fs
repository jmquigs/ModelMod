namespace ModelMod

open SharpDX.Direct3D9 

type SDXVertexElement = SharpDX.Direct3D9.VertexElement
type SDXVertexDeclUsage = SharpDX.Direct3D9.DeclarationUsage
type SDXVertexDeclType = SharpDX.Direct3D9.DeclarationType

module SnapshotProfiles =
    let Profile1 = "Profile1"
    let Profile2 = "Profile2"

    let ValidProfiles = [ Profile1; Profile2 ]

    let DefaultProfile = Profile1

    let isValid (profile:string) =
        ValidProfiles |> List.exists (fun p -> p.ToLowerInvariant() = profile.ToLowerInvariant())

module InputProfiles = 
    let PunctRock = "PunctuationKeys"
    let FItUp = "FKeys"
    
    let ValidProfiles = [ PunctRock; FItUp ]

    let DefaultProfile = FItUp

    let isValid (profile:string) =
        ValidProfiles |> List.exists (fun p -> p.ToLowerInvariant() = profile.ToLowerInvariant())

module CoreTypes =
    // ------------------------------------------------------------------------
    // Some monogame helper types
    type Vec2F = Microsoft.Xna.Framework.Vector2    
    type Vec3F = Microsoft.Xna.Framework.Vector3
    type Vec4F = Microsoft.Xna.Framework.Vector4

    type Vec4X(x,y,z,w) =
        member v.X = x
        member v.Y = y
        member v.Z = z
        member v.W = w

    // ------------------------------------------------------------------------
    // Configuration types
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
        /// Whether the current/next run is in full (snapshot) mode or playback only
        RunModeFull: bool 
        /// Input profile to use
        InputProfile: string 
        /// Snapshot profile to use (i.e.: model transforms for snapshot) 
        SnapshotProfile: string 
        /// Doc root for this profile.  Currently ignored.
        DocRoot: string 
    } 

    let DefaultRunConfig = {
        ProfileKeyName = ""
        ProfileName = ""
        ExePath = ""
        RunConfig.RunModeFull = true
        LoadModsOnStart = true
        InputProfile = ""
        SnapshotProfile = ""
        DocRoot = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.MyDocuments),"ModelMod")
    }

    // ------------------------------------------------------------------------
    // Mod and ref data

    type ModType = GPUReplacement | CPUReplacement | Deletion | Reference 

    // These types control how weighting is done in MeshRelation, in particular where the 
    // blend indices and blend weights are expected to be found.
    type WeightMode = 
        // Get blend data from mod mesh.  Mod author must ensure that all verts are propertly 
        // weighted in the 3d tool.  This can be tedious, especially with symmetric parts, so this 
        // mode is primarily here for advanced users and control freaks.
        Mod 
        // Get blend data from the ref.  This is the default and easiest mode to use.
        | Ref 
        // Get blend data from the binary ref data.  This is mostly a developer debug mode - it doesn't 
        // support vertex annotation group filtering, amongst other limitations.
        | BinaryRef

    type PTNIndex = { Pos: int; Tex: int; Nrm: int }

    type IndexedTri = {
        Verts: PTNIndex[] // 3 elements long, where each element contains int indexes into position, texture, etc.
    }

    // A vertex declaration may not be present.  If present, both the raw bytes 
    // and an unpacked list of elements are available.
    type VertexDeclarationData = byte[] * SDXVertexElement list

    type BinaryVertexData = {
        NumVerts: uint32
        Stride: uint32
        Data: byte[]
    }

    type MeshReadFlags = {
        ReadMaterialFile: bool
        ReverseTransform: bool
    }
    let DefaultReadFlags = {
        ReadMaterialFile = false 
        ReverseTransform = true
    }

    type Mesh = {
        Type : ModType
        Triangles : IndexedTri[]
        Positions: Vec3F[]
        UVs: Vec2F[]
        Normals: Vec3F[]
        BlendIndices: Vec4X[]
        BlendWeights: Vec4F[]
        Declaration : VertexDeclarationData option
        BinaryVertexData: BinaryVertexData option
        AnnotatedVertexGroups: string list []
        AppliedPositionTransforms: string []
        AppliedUVTransforms: string[]
        // TODO: could use array here for textures
        Tex0Path: string 
        Tex1Path: string 
        Tex2Path: string 
        Tex3Path: string 
    }

    // ------------------------------------------------------------------------
    // These are types loaded by the moddb from yaml files
    type DBReference = {
        Name : string
        Mesh : Mesh
    }

    type GeomDeletion = { PrimCount: int; VertCount: int }

    type ModAttributes = {
        DeletedGeometry: GeomDeletion list
    }

    let EmptyModAttributes = { ModAttributes.DeletedGeometry = [] }

    type DBMod = {
        RefName: string option
        Ref: DBReference option
        Name: string
        Mesh: Mesh option
        WeightMode: WeightMode
        Attributes: ModAttributes
    }

    // union type for the yaml types, for list storage, etc
    type ModElement = 
        Unknown 
        | MReference of DBReference
        | Mod of DBMod

