module XnaRender

open Microsoft.Xna.Framework
open Microsoft.Xna.Framework.Graphics

open ModelMod
open ModelMod.CoreTypes

type ObjectRenderData = {
    Effect: BasicEffect;
    RasterizerState :RasterizerState;
    VertexBuffer: VertexBuffer;
    IndexBuffer: IndexBuffer;
    VertCount: int;
    PrimCount: int;
}

type WorldRenderData = {
    Device:GraphicsDevice;
    View:Matrix;
    Projection:Matrix;
}

type IXnaRenderable =
    abstract member Update : int -> unit // TODO: should probably be a in different interface (ITickable?)
    abstract member Render : WorldRenderData -> unit;

let basicRender (objRenderData:ObjectRenderData) (worldRenderData:WorldRenderData) =
    let device = worldRenderData.Device
    let saveState = device.RasterizerState

    let effect = objRenderData.Effect

    // set view/projection in effect
    effect.View <- worldRenderData.View
    effect.Projection <- worldRenderData.Projection
    device.RasterizerState <- objRenderData.RasterizerState
    device.Indices <- objRenderData.IndexBuffer
    device.SetVertexBuffer objRenderData.VertexBuffer

    for pass in effect.CurrentTechnique.Passes do
        pass.Apply()
        device.DrawIndexedPrimitives(PrimitiveType.TriangleList, 0, 0, objRenderData.VertCount, 0, objRenderData.PrimCount)

    device.RasterizerState <- saveState
    
let wireframeState = 
    let state = new RasterizerState()
    state.CullMode <- CullMode.CullClockwiseFace
    state.FillMode <- FillMode.WireFrame
    state
       
let toXNAVert position =
    VertexPositionColor(position, Color.RoyalBlue)

let makeVBIB device (mesh:Mesh) =
    let vt = typeof<VertexPositionColor>
    let numVerts = mesh.Positions.Length
    let primCount = mesh.Triangles.Length

    let vb = new VertexBuffer(device, vt, numVerts, BufferUsage.None)

    let indices = [
        for tri in mesh.Triangles do
            yield! [| int16 tri.Verts.[0].Pos; int16 tri.Verts.[1].Pos; int16 tri.Verts.[2].Pos;  |]
    ]
    let ib = new IndexBuffer(device, IndexElementSize.SixteenBits, 16 * indices.Length, BufferUsage.None)
    ib.SetData (List.toArray indices)
    vb,ib    

// helper function to change the vert data on the VB
let setVBData (vb:VertexBuffer) verts =
    vb.GraphicsDevice.SetVertexBuffer null // Xna doesn't like it if we try to change the vb data while the vb is set on the device
    vb.SetData verts
   
let MakeMesh(device, mesh:Mesh) =
    let vt = typeof<VertexPositionColor>

    let verts = mesh.Positions |> Array.map (fun p -> VertexPositionColor(p, Color.RoyalBlue))
    let vb = new VertexBuffer(device, vt, verts.Length, BufferUsage.None)
    vb.SetData verts

    let indices = [
        for tri in mesh.Triangles do
            yield! [| int16 tri.Verts.[0].Pos; int16 tri.Verts.[1].Pos; int16 tri.Verts.[2].Pos;  |]
    ]
    let ib = new IndexBuffer(device, IndexElementSize.SixteenBits, 16 * indices.Length, BufferUsage.None)
    ib.SetData (List.toArray indices)

    let effect = new BasicEffect(device)
    effect.LightingEnabled <- false
    effect.World <- Matrix.Identity

    let renderFn = basicRender { 
        Effect=effect
        RasterizerState=wireframeState
        VertexBuffer=vb
        IndexBuffer=ib
        VertCount=verts.Length
        PrimCount=mesh.Triangles.Length
    }
    
    { new IXnaRenderable with 
        member x.Update elapsed = ()
        member x.Render wrd = renderFn(wrd)
    }

let MakeBox(device) =
    let vt = typeof<VertexPositionColor>
    
    let verts = [| 
        VertexPositionColor(Vector3(-1.f, -1.f,  1.f), Color.RoyalBlue); //0 FBL
        VertexPositionColor(Vector3(-1.f,  1.f,  1.f), Color.RoyalBlue); //1 FTL
        VertexPositionColor(Vector3( 1.f, -1.f,  1.f), Color.RoyalBlue); //2 FBR
        VertexPositionColor(Vector3( 1.f,  1.f,  1.f), Color.RoyalBlue); //3 FTR

        VertexPositionColor(Vector3(-1.f, -1.f, -1.f), Color.RoyalBlue); //4 BBL
        VertexPositionColor(Vector3(-1.f,  1.f, -1.f), Color.RoyalBlue); //5 BTL
        VertexPositionColor(Vector3( 1.f, -1.f, -1.f), Color.RoyalBlue); //6 BBR
        VertexPositionColor(Vector3( 1.f,  1.f, -1.f), Color.RoyalBlue); //7 BTR
            
    |]
    let vb = new VertexBuffer(device, vt, 8, BufferUsage.None)
    vb.SetData verts

    // cube = 6 faces 2 tris each, 3 verts per tri = 6 * 2 * 3 indices
    let indices = [|
        1s; 0s; 2s; // front 
        2s; 3s; 1s;
        5s; 1s; 3s; // top
        3s; 7s; 5s;
        3s; 2s; 6s; // right
        6s; 7s; 3s;
        0s; 1s; 5s; // left
        5s; 4s; 0s;
        0s; 4s; 2s; // bottom
        4s; 6s; 2s;
        4s; 7s; 6s; // back            
        5s; 7s; 4s;
    |]
    let ib = new IndexBuffer(device, IndexElementSize.SixteenBits, 16 * indices.Length, BufferUsage.None)
    ib.SetData indices
        
    let effect = new BasicEffect(device)
    effect.LightingEnabled <- false

    effect.World <- Matrix.Identity
    // View and Projection set during render

    let renderFn = basicRender { Effect=effect; RasterizerState=wireframeState; VertexBuffer=vb;IndexBuffer=ib;VertCount=verts.Length;PrimCount=12}

    { new IXnaRenderable with 
        member x.Update elapsed = ()
        member x.Render wrd = renderFn(wrd)
    }