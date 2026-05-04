module TestMeshRelation

open NUnit.Framework

open ModelMod
open ModelMod.CoreTypes

// Build a minimal Mesh with the given positions and vertex-group annotations.
// The relation builder only consults Positions, AnnotatedVertexGroups, and Type
// (the last only matters for CPU skinning, which we don't exercise here).
let private mkMesh (modType: ModType) (positions: Vec3F[]) (groups: string list[]) =
    {
        Mesh.Type = modType
        Triangles = [||]
        Positions = positions
        UVs = [||]
        Normals = [||]
        BlendIndices = [||]
        BlendWeights = [||]
        Declaration = None
        BinaryVertexData = None
        AnnotatedVertexGroups = groups
        AppliedPositionTransforms = [||]
        AppliedUVTransforms = [||]
        Tex0Path = ""
        Tex1Path = ""
        Tex2Path = ""
        Tex3Path = ""
        Cached = false
    }

let private mkRef (name: string) (mesh: Mesh) : DBReference =
    {
        Name = name
        Mesh = lazy mesh
        MeshPath = ""
        MeshReadFlags = DefaultReadFlags
        PrimCount = 0
        VertCount = mesh.Positions.Length
    }

let private mkMod (name: string) (refr: DBReference) (mesh: Mesh) : DBMod =
    {
        RefName = Some refr.Name
        Type = mesh.Type
        Ref = Some refr
        Name = name
        Mesh = Some (lazy mesh)
        MeshPath = ""
        MeshReadFlags = DefaultReadFlags
        WeightMode = WeightMode.Ref
        PixelShader = ""
        Attributes = EmptyModAttributes
        ParentModName = None
        UpdateTangentSpace = None
        Profile = None
        VBChecksum = None
    }

[<Test>]
let ``MeshRelation: Include and Exclude vertex group tracking``() =
    // Reference mesh: 8 verts, indices 0..3 in "Hat.Top", indices 4..7 in "Hat.Bottom".
    // Top and bottom positions are deliberately interleaved very close together so that
    // a naive nearest-neighbor match without group filtering would pick the wrong group;
    // this proves the include/exclude logic is doing the work, not coincidence.
    let refPositions = [|
        // Hat.Top (indices 0..3) at x = 0.00, 1.00, 2.00, 3.00
        Vec3F(0.0f,  0.f, 0.f)
        Vec3F(1.0f,  0.f, 0.f)
        Vec3F(2.0f,  0.f, 0.f)
        Vec3F(3.0f,  0.f, 0.f)
        // Hat.Bottom (indices 4..7) at x = 0.05, 1.05, 2.05, 3.05
        Vec3F(0.05f, 0.f, 0.f)
        Vec3F(1.05f, 0.f, 0.f)
        Vec3F(2.05f, 0.f, 0.f)
        Vec3F(3.05f, 0.f, 0.f)
    |]
    let refGroups : string list [] = [|
        ["Hat.Top"];    ["Hat.Top"];    ["Hat.Top"];    ["Hat.Top"]
        ["Hat.Bottom"]; ["Hat.Bottom"]; ["Hat.Bottom"]; ["Hat.Bottom"]
    |]
    let refMesh = mkMesh Reference refPositions refGroups

    // Mod mesh: 8 verts.
    //   verts 0..3 use "Include.Hat.Top": positioned at x = 0.04 etc., physically
    //     closer to Hat.Bottom (0.05) than Hat.Top (0.00). The include filter
    //     must force them to map to Hat.Top (indices 0..3).
    //   verts 4..7 use "Exclude.Hat.Top": positioned at x = 0.01 etc., physically
    //     closer to Hat.Top (0.00) than Hat.Bottom (0.05). The exclude filter
    //     must force them to map to Hat.Bottom (indices 4..7).
    let modPositions = [|
        Vec3F(0.04f, 0.f, 0.f)
        Vec3F(1.04f, 0.f, 0.f)
        Vec3F(2.04f, 0.f, 0.f)
        Vec3F(3.04f, 0.f, 0.f)
        Vec3F(0.01f, 0.f, 0.f)
        Vec3F(1.01f, 0.f, 0.f)
        Vec3F(2.01f, 0.f, 0.f)
        Vec3F(3.01f, 0.f, 0.f)
    |]
    let modGroups : string list [] = [|
        ["Include.Hat.Top"]; ["Include.Hat.Top"]; ["Include.Hat.Top"]; ["Include.Hat.Top"]
        ["Exclude.Hat.Top"]; ["Exclude.Hat.Top"]; ["Exclude.Hat.Top"]; ["Exclude.Hat.Top"]
    |]
    let modMesh = mkMesh GPUReplacement modPositions modGroups

    let refr = mkRef "TestRef" refMesh
    let dbmod = mkMod "TestMod" refr modMesh

    // empty bin cache dir disables disk caching for this test
    let mr = MeshRelation.MeshRelation(dbmod, refr, "")
    let vrs = mr.Build()

    Assert.AreEqual(8, vrs.Length, sprintf "wrong vert rel count: %d" vrs.Length)

    // First 4 mod verts (Include.Hat.Top) must map into the Hat.Top ref range 0..3.
    for i in 0..3 do
        let idx = vrs.[i].RefPointIdx
        Assert.IsTrue(
            idx >= 0 && idx <= 3,
            sprintf "mod vert %d (Include.Hat.Top) matched ref idx %d; expected one of 0..3" i idx)

    // Last 4 mod verts (Exclude.Hat.Top) must map into the Hat.Bottom ref range 4..7.
    for i in 4..7 do
        let idx = vrs.[i].RefPointIdx
        Assert.IsTrue(
            idx >= 4 && idx <= 7,
            sprintf "mod vert %d (Exclude.Hat.Top) matched ref idx %d; expected one of 4..7" i idx)

    // Check that all 4 Hat.Top ref verts get used by exactly one mod vert
    // (4 Include.Hat.Top mod verts mapped to 4 distinct Hat.Top ref verts), and
    // similarly for Hat.Bottom. This catches a regression where the filter
    // works but every mod vert collapses onto a single ref vert.
    let topMatches = [for i in 0..3 -> vrs.[i].RefPointIdx] |> Set.ofList
    let botMatches = [for i in 4..7 -> vrs.[i].RefPointIdx] |> Set.ofList
    Assert.AreEqual(4, topMatches.Count, sprintf "Include.Hat.Top mod verts collapsed: %A" topMatches)
    Assert.AreEqual(4, botMatches.Count, sprintf "Exclude.Hat.Top mod verts collapsed: %A" botMatches)
