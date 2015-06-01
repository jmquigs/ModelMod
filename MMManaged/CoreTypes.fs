namespace ModelMod

open SharpDX.Direct3D9 

type SDXVertexElement = SharpDX.Direct3D9.VertexElement
type SDXVertexDeclUsage = SharpDX.Direct3D9.DeclarationUsage
type SDXVertexDeclType = SharpDX.Direct3D9.DeclarationType

module SnapshotProfiles =
    let Profile1 = "profile1"
    let Profile2 = "profile2"

    let ValidProfiles = [ Profile1; Profile2 ]

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
        ExePath: string
        RunModeFull: bool
        InputProfile: string
        SnapshotProfile: string
        DocRoot: string
    }

    let DefaultRunConfig = {
        ExePath = ""
        RunConfig.RunModeFull = true
        InputProfile = ""
        SnapshotProfile = ""
        DocRoot = ""
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

