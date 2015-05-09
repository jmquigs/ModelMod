namespace ModelMod

module Types =
    type Vec2F = Microsoft.Xna.Framework.Vector2    
    type Vec3F = Microsoft.Xna.Framework.Vector3
    type Vec4F = Microsoft.Xna.Framework.Vector4

    type Vec4X(x,y,z,w) =
        member v.X = x
        member v.Y = y
        member v.Z = z
        member v.W = w
    
    
