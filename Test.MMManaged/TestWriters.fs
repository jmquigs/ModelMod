module TestWriters

open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod
open ModelMod.CoreTypes
open ModelMod.VertexTypes

open ModelMod.InteropTypes
open ModelMod.ModDB
open ModelMod.ModDBInterop

open MeshRelation

let emptyMesh = {
   Mesh.Type = ModType.GPUReplacement
   Triangles = [||]
   Positions = [||]
   UVs = [||]
   Normals = [||]
   BlendIndices = [||]
   BlendWeights = [||]
   Declaration = None
   BinaryVertexData = None
   AnnotatedVertexGroups = [||]
   AppliedPositionTransforms = [||]
   AppliedUVTransforms = [||]
   Tex0Path = ""
   Tex1Path = ""
   Tex2Path = ""
   Tex3Path = ""
   Cached = false
}

let newOut(size) =
    let outBytes = Array.zeroCreate size
    let bout = new MemoryStream(outBytes)
    let bw = new BinaryWriter(bout)
    bw,outBytes

let fakeVertRel (refPtIdx:int) =
    {
        VertRel.RefPointIdx = refPtIdx
        VertRel.Distance = 0.0f // unused by this test
        ModVertPos = Vec3F(0.0f, 0.0f, 0.0f) // unused by this test
        RefVertPos = Vec3F(0.0f, 0.0f, 0.0f) // unused by this test
        CpuSkinningData = None // unused by this test
    }

[<Test>]
let ``ModDBInterop: test fillModData``() =
    // TODO: should test this, which will test the entire fill process end to end.
    // importantly it should test intra-vert stride seeking for verts that have
    // gaps in their data (seen in low detail settings for example)
    ()

[<Test>]
let ``ModDBInterop: DataWriters functions``() =
    let testWithWriteFn (targEl:MMVertexElement) desttypes exRead fn =
        desttypes |> List.iter (fun dtype ->
            let targEl = { targEl with Type = dtype  }
            exRead |> List.iteri (fun modVertIndex expected ->
                let (bw: BinaryWriter),outBytes = newOut 256
                fn modVertIndex targEl bw
                bw.Flush()
                let take = expected |> Array.length
                Assert.AreEqual (expected, outBytes |> Array.take take, sprintf "modVertIndex %d, dtype %A" modVertIndex dtype)
            )
        )

    let vertrels = [| fakeVertRel 0; fakeVertRel 1;|]
    let targEl = {
        MMVertexElement.Semantic = MMVertexElemSemantic.Unknown
        SemanticIndex = 0 // unused by this test
        Type = DeclType(SDXVertexDeclType.Unused) // will be set to something real below
        Offset = 0 // unused by this test
    }

    // modmBlendIndex
    // output formats to test
    let mesh = { emptyMesh with BlendIndices = [| Vec4X(1,2,3,4); Vec4X(5,6,7,8); |] }
    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendIndices}
    let desttypes = [
        DeclType(SDXVertexDeclType.Color);
        DeclType(SDXVertexDeclType.Ubyte4);
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let exRead = [ // expected result from each vert write
        [|1uy; 2uy; 3uy; 4uy|]
        [|5uy; 6uy; 7uy; 8uy|]
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmBlendIndex vertrels mesh modVertIndex targEl bw
    )

    // modmBlendWeight ubyte
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.UByte4N)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendWeight}
    let mesh = { emptyMesh with BlendWeights = [| Vec4F(0.25f,0.5f,0.75f,1.0f); Vec4F(0.1f,0.2f,0.3f,0.4f); |] }
    let exRead = [ // expected result from each vert write
        [|64uy; 128uy; 191uy; 255uy|] // conversion is rounded/lossy
        [|26uy; 51uy; 77uy; 102uy|]
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmBlendWeight vertrels mesh modVertIndex targEl bw
    )
    // modmBlendWeight float
    let desttypes = [
        DeclType(SDXVertexDeclType.Float4)
        Format(SDXF.R32G32B32A32_Float)
    ]
    let floatsToByteArray (floats:float32[]) =
        let ms = new MemoryStream()
        let bw = new BinaryWriter(ms)
        floats |> Array.iter (fun f -> bw.Write(f))
        bw.Flush()
        ms.ToArray()

    let exRead = [ // expected result from each vert write
        [|0.25f; 0.5f; 0.75f; 1.0f|] |> floatsToByteArray
        [|0.1f; 0.2f; 0.3f; 0.4f|] |> floatsToByteArray
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmBlendWeight vertrels mesh modVertIndex targEl bw
    )
    // modmNormal ubyte
    let targEl = { targEl with Semantic = MMVertexElemSemantic.Normal }
    let mesh = { emptyMesh with Normals = [| Vec3F(0.25f,0.5f,0.75f); Vec3F(0.1f,0.2f,0.3f); |] }
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.Ubyte4)
        DeclType(SDXVertexDeclType.UByte4N)
        Format(SDXF.R8G8B8A8_UNorm)
    ]

    let fToUint f = f * 128.f + 127.f |> uint8 // what write4ByteVector does
    let exRes1 = [
        [| 0.25f;0.5f;0.75f; |] |> Array.map fToUint
        [| 0.1f;0.2f;0.3f; |] |> Array.map fToUint
    ]
    // write4ByteVector also looks at ReverseNormals so need to test that on/off
    // reversing normals leaves w at end so effectively just x and z are swapped
    let conf = {State.Data.Conf with GameProfile = {State.Data.Conf.GameProfile with ReverseNormals = true}}
    State.testSetConf conf
    let exRead = exRes1 |> List.map (fun a -> [| a.[2]; a.[1]; a.[0]; 0uy |])
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmNormal mesh modVertIndex 0 targEl bw
    )

    let exRes1 = [
        [| 0.25f;0.5f;0.75f; |] |> Array.map fToUint
        [| 0.1f;0.2f;0.3f; |] |> Array.map fToUint
    ]
    let exRead = exRes1 |> List.map (fun a -> [| a.[0]; a.[1]; a.[2]; 0uy |] )
    let conf = {State.Data.Conf with GameProfile = {State.Data.Conf.GameProfile with ReverseNormals = false}}
    State.testSetConf conf
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmNormal mesh modVertIndex 0 targEl bw
    )

    // modmNormal float - doesn't use ReverseNormals so don't need to change conf
    let desttypes = [
        DeclType(SDXVertexDeclType.Float3)
        Format(SDXF.R32G32B32_Float)
    ]
    let exRead = [
        [| 0.25f;0.5f;0.75f; |] |> floatsToByteArray
        [| 0.1f;0.2f;0.3f; |] |> floatsToByteArray
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.modmNormal mesh modVertIndex 0 targEl bw
    )

    // modmBinormalTangent SKIPPED
    // right now it just generates binormals and tangents from a fixed coordinate axis and the
    // results aren't great anyway, so I'm not going to bother testing. at some point I need to
    // update the mod loader to either generate those properly or use vectors from the
    // mesh export which is blender python work (ugh) that I started in `fix-lighting-export`
    // branch

    // refmBlendIndex ubyte
    // note the mesh here now represents the ref mesh, not the mod mesh as in earlier cases.
    // we don't need a mod mesh because the refmFoo functions extract data from the ref only
    let mesh = {
        emptyMesh with
            Type = ModType.Reference
            BlendIndices = [| Vec4X(1,2,3,4); Vec4X(5,6,7,8); |]
    }
    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendIndices}
    let desttypes = [
        DeclType(SDXVertexDeclType.Color);
        DeclType(SDXVertexDeclType.Ubyte4);
        Format(SDXF.R8G8B8A8_UNorm)
        Format(SDXF.R8G8B8A8_UInt)
    ]
    let exRead = [ // expected result from each vert write
        [|1uy; 2uy; 3uy; 4uy|]
        [|5uy; 6uy; 7uy; 8uy|]
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.refmBlendIndex vertrels mesh modVertIndex targEl bw
    )

    // refmBlendWeight ubyte
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.UByte4N)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendWeight}
    let mesh = {
        emptyMesh with
            Type = ModType.Reference
            BlendWeights = [| Vec4F(0.25f,0.5f,0.75f,1.0f); Vec4F(0.1f,0.2f,0.3f,0.4f); |] }
    let exRead = [ // expected result from each vert write
        [|64uy; 128uy; 191uy; 255uy|] // conversion is rounded/lossy
        [|26uy; 51uy; 77uy; 102uy|]
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.refmBlendWeight vertrels mesh modVertIndex targEl bw
    )
    // refmBlendWeight float
    let desttypes = [
        DeclType(SDXVertexDeclType.Float4)
        Format(SDXF.R32G32B32A32_Float)
    ]
    let exRead = [ // expected result from each vert write
        [|0.25f; 0.5f; 0.75f; 1.0f|] |> floatsToByteArray
        [|0.1f; 0.2f; 0.3f; 0.4f|] |> floatsToByteArray
    ]
    testWithWriteFn targEl desttypes exRead (fun modVertIndex targEl bw ->
        DataWriters.refmBlendWeight vertrels mesh modVertIndex targEl bw
    )

[<Test>]
let ``ModDBInterop: RawBinaryWriters functions``() =
    // These are tedious to test because they require a bunch of set up, however, they
    // should not be used very much (check the RawBinaryWriters module for a doc comment
    // explaining them)
    let _ = RawBinaryWriters.rbNormal

    let defm = SharpDX.Direct3D9.DeclarationMethod.Default
    let decl =
        [|
            SDXVertexElement(int16 0, int16 0, SDXVertexDeclType.Ubyte4,
                defm, SDXVertexDeclUsage.Position, byte 0);
            SDXVertexElement(int16 0, int16 8, SDXVertexDeclType.Ubyte4,
                defm, SDXVertexDeclUsage.BlendIndices, byte 0);
            SDXVertexElement(int16 0, int16 16, SDXVertexDeclType.UByte4N,
                defm, SDXVertexDeclUsage.BlendWeight, byte 0);
            SDXVertexElement(int16 0, int16 24, SDXVertexDeclType.UByte4N,
                defm, SDXVertexDeclUsage.Normal, byte 0);
            SDXVertexElement(int16 0, int16 32, SDXVertexDeclType.UByte4N,
                defm, SDXVertexDeclUsage.Binormal, byte 0);
            SDXVertexElement(int16 0, int16 40, SDXVertexDeclType.UByte4N,
                defm, SDXVertexDeclUsage.Tangent, byte 0);
        |]
    let bvd = {
        BinaryVertexData.NumVerts = uint32 2
        Stride = uint32 44
        Data = [1..88] |> List.map byte |> Array.ofList
    }
    let mmdecl = decl |> Array.map VertexTypes.sdxDeclElementToMMDeclElement
    let blh = new ModDB.BinaryLookupHelper(bvd, mmdecl)

    let fakeVertRel (refPtIdx:int) =
        {
            VertRel.RefPointIdx = refPtIdx
            VertRel.Distance = 0.0f // unused by this test
            ModVertPos = Vec3F(0.0f, 0.0f, 0.0f) // unused by this test
            RefVertPos = Vec3F(0.0f, 0.0f, 0.0f) // unused by this test
            CpuSkinningData = None // unused by this test
        }

    let vertrels = [| fakeVertRel 0; fakeVertRel 1;|]

    let targEl = {
        MMVertexElement.Semantic = MMVertexElemSemantic.BlendIndices
        SemanticIndex = 0 // unused by this test
        Type = DeclType(SDXVertexDeclType.Unused) // will be set to something real below
        Offset = 0 // unused by this test
    }

    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendIndices }
    // output formats to test
    let desttypes = [
        DeclType(SDXVertexDeclType.Ubyte4);
        Format(SDXF.R8G8B8A8_UInt)
    ]
    let exRead = [ // expected result from each vert write
        [|9uy; 10uy; 11uy; 12uy|]
        [|53uy; 54uy; 55uy; 56uy|]
    ]
    desttypes |> List.iter (fun dtype ->
        let targEl = { targEl with Type = dtype  }
        exRead |> List.iteri (fun modVertIndex expected ->
            let (bw: BinaryWriter),outBytes = newOut 256
            RawBinaryWriters.rbBlendIndex blh vertrels modVertIndex targEl bw
            bw.Flush()
            Assert.AreEqual (outBytes |> Array.take 4, expected)
        )
    )

    let targEl = { targEl with Semantic = MMVertexElemSemantic.BlendWeight }
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.UByte4N)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let exRead = [ // expected result from each vert write
        [|17uy; 18uy; 19uy; 20uy|]
        [|61uy; 62uy; 63uy; 64uy|]
    ]
    desttypes |> List.iter (fun dtype ->
        let targEl = { targEl with Type = dtype  }
        exRead |> List.iteri (fun modVertIndex expected ->
            let bw,outBytes = newOut 256
            RawBinaryWriters.rbBlendWeight blh vertrels modVertIndex targEl bw
            bw.Flush()
            Assert.AreEqual (outBytes |> Array.take 4, expected)
        )
    )

    let targEl = { targEl with Semantic = MMVertexElemSemantic.Normal }
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.Ubyte4)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let exRead = [ // expected result from each vert write
        [|25uy; 26uy; 27uy; 28uy|]
        [|69uy; 70uy; 71uy; 72uy|]
    ]
    desttypes |> List.iter (fun dtype ->
        let targEl = { targEl with Type = dtype  }
        exRead |> List.iteri (fun modVertIndex expected ->
            let bw,outBytes = newOut 256
            RawBinaryWriters.rbNormal blh vertrels 0 modVertIndex targEl bw
            bw.Flush()
            Assert.AreEqual (outBytes |> Array.take 4, expected)
        )
    )

    let targEl = { targEl with Semantic = MMVertexElemSemantic.Binormal }
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.Ubyte4)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let exRead = [ // expected result from each vert write
        [|33uy; 34uy; 35uy; 36uy|]
        [|77uy; 78uy; 79uy; 80uy|]
    ]
    desttypes |> List.iter (fun dtype ->
        let targEl = { targEl with Type = dtype  }
        exRead |> List.iteri (fun modVertIndex expected ->
            let bw,outBytes = newOut 256
            RawBinaryWriters.rbNormal blh vertrels 0 modVertIndex targEl bw
            bw.Flush()
            Assert.AreEqual (outBytes |> Array.take 4, expected)
        )
    )

    let targEl = { targEl with Semantic = MMVertexElemSemantic.Tangent }
    let desttypes = [
        DeclType(SDXVertexDeclType.Color)
        DeclType(SDXVertexDeclType.Ubyte4)
        Format(SDXF.R8G8B8A8_UNorm)
    ]
    let exRead = [ // expected result from each vert write
        [|41uy; 42uy; 43uy; 44uy|]
        [|85uy; 86uy; 87uy; 88uy|]
    ]
    desttypes |> List.iter (fun dtype ->
        let targEl = { targEl with Type = dtype  }
        exRead |> List.iteri (fun modVertIndex expected ->
            let bw,outBytes = newOut 256
            RawBinaryWriters.rbNormal blh vertrels 0 modVertIndex targEl bw
            bw.Flush()
            Assert.AreEqual (outBytes |> Array.take 4, expected)
        )
    )