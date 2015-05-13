#if COMPILED
namespace ModelMod
#endif

module SnapshotProfiles =
    let Profile1 = "profile1"
    let Profile2 = "profile2"

    let ValidProfiles = [ Profile1; Profile2 ]

module Types =
    type Vec2F = Microsoft.Xna.Framework.Vector2    
    type Vec3F = Microsoft.Xna.Framework.Vector3
    type Vec4F = Microsoft.Xna.Framework.Vector4

    type Vec4X(x,y,z,w) =
        member v.X = x
        member v.Y = y
        member v.Z = z
        member v.W = w
    
    type RunConfig = {
        RunModeFull: bool
        InputProfile: string
        SnapshotProfile: string
        DocRoot: string
    }

    let DefaultRunConfig = {
        RunConfig.RunModeFull = true
        InputProfile = ""
        SnapshotProfile = ""
        DocRoot = ""
    }

