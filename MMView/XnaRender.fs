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

namespace MeshView

module XnaRender =
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
        inherit System.IDisposable
        abstract member Update : int -> unit 
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
    
    let wireframeState() = 
        // this needs to be a function that returns a new state each time; rather than a 
        // constant value.  This is because the 
        // state will get bound to the current graphics device when used, which will lead
        // to an exception if the window is closed and reopened because the device has changed.
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

        let wfState = wireframeState()

        if mesh.Tex0Path <> "" then
            // TODO: here we would actually try to load the texture and install it into the effect.  However, 
            // Monogame/SharpDX don't have direct support for loading DDS textures, so we have to do one of these:
            // 1) Use the content pipeline (an additional assembly reference, and there isn't a nuget for it)
            // 2) Use FreeImageNET/FreeImage directly (which is what the content pipeline uses)
            // 3) Add yet another dependency to legacy D3DX and use pinvoke on it.
            // 4) Use one of MS's open source dx toolkits to load it
            // I tried looking at 1, and managed to get it to actually load a TextureContent, but it wasn't
            // clear how to convert that into a Texture2D for the effect without writing a bunch of tedious data
            // filling code - shouldn't there be a utility for this somewhere?
            // Ideally we'd just use FreeImage or something else
            // directly if we can, so that we don't depend on the whole content
            // pipeline just for this one feature.
            // We may also be able to use the DirectXTK for this since it doesn't need to be D3D9 compatible
            // https://github.com/Microsoft/DirectXTK
            // or https://github.com/Microsoft/DirectXTex
            // If we could use one of those open source DX libs, and it was compatible with D3D9, 
            // maybe we could eliminate the whole stupid legacy D3DX dependency entirely.
            // Giving up for now.
            ()

        let renderFn = basicRender { 
            Effect=effect
            RasterizerState=wfState
            VertexBuffer=vb
            IndexBuffer=ib
            VertCount=verts.Length
            PrimCount=mesh.Triangles.Length
        }
    
        { new IXnaRenderable with 
            member x.Update elapsed = ()
            member x.Render wrd = renderFn(wrd)
            member x.Dispose() = 
                effect.Dispose()
                wfState.Dispose()
                vb.Dispose()
                ib.Dispose()
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

        let renderFn = basicRender { Effect=effect; RasterizerState=wireframeState(); VertexBuffer=vb;IndexBuffer=ib;VertCount=verts.Length;PrimCount=12}

        { new IXnaRenderable with 
            member x.Update elapsed = ()
            member x.Render wrd = renderFn(wrd)
            member x.Dispose() = 
                vb.Dispose()
                ib.Dispose()
                effect.Dispose()
        }