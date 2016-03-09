// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace MeshView

module Camera =
    open Microsoft.Xna.Framework
    open Microsoft.Xna.Framework.Input

    type CameraData(view,proj:Matrix,rot:Matrix,pos:Vector3) = 
        member x.Rotation = rot
        member x.Position = pos
        member x.View = view
        member x.Projection = proj

    let make proj position = 
        let defRot = Matrix.Identity
        let target = 10.0f * defRot.Forward
        let view = Matrix.CreateLookAt(position, target, defRot.Up)

        CameraData(view,proj,defRot,position)

    let update (c:CameraData) (keyState:KeyboardState) gameTime =
        let position = c.Position
        let rot = c.Rotation
    
        let moveSpeed = 0.3f;
        let rotSpeed = 0.02f;
    
        let move (vec:Vector3) = moveSpeed * vec
    
        let position = if (keyState.IsKeyDown(Keys.W)) then (position + move rot.Forward) else position
        let position = if (keyState.IsKeyDown(Keys.S)) then (position + move rot.Backward) else position
        let position = if (keyState.IsKeyDown(Keys.A)) then (position + move rot.Left) else position
        let position = if (keyState.IsKeyDown(Keys.D)) then (position + move rot.Right) else position
        let position = if (keyState.IsKeyDown(Keys.Q)) then (position + move rot.Up) else position
        let position = if (keyState.IsKeyDown(Keys.E)) then (position + move rot.Down) else position

        let yaw = 0.0f
        let pitch = 0.0f
        let pitch = pitch + if (keyState.IsKeyDown(Keys.I)) then rotSpeed else 0.0f
        let pitch = pitch + if (keyState.IsKeyDown(Keys.K)) then -rotSpeed else 0.0f
        let yaw = yaw + if (keyState.IsKeyDown(Keys.J)) then rotSpeed else 0.0f
        let yaw = yaw + if (keyState.IsKeyDown(Keys.L)) then -rotSpeed else 0.0f
    
        let rot = rot * Matrix.CreateFromAxisAngle(rot.Right, pitch);
        let rot = rot * Matrix.CreateFromAxisAngle(rot.Up, yaw);

        let target = position + rot.Forward

        let view = Matrix.CreateLookAt(position, target, rot.Up)

        new CameraData(view,c.Projection,rot,position)