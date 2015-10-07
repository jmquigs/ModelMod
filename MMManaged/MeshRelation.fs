// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

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

namespace ModelMod

open System
open System.IO

open Microsoft.Xna.Framework

open CoreTypes

module MeshRelation =

    type Tri = {
        // three elements each
        Position: Vec3F[];
        TexCoord: Vec2F[];
        Normal: Vec3F[];
    }

    let private log = Logging.getLogger("MeshRelation")

    type MVProjections = { x:float ; y:float ; z:float } 

    type CPUSkinningData = {
        UseRef: bool
        VecToModVert: Vec3F
        ModProjections: MVProjections
        RefIndices: int[]   
        RefNormal: Vec3F
        //refTexCoord: Vec2F
    }
    type VertRel = {
        Distance: float32
        RefPointIdx: int
        ModVertPos: Vec3F
        RefVertPos: Vec3F
        CpuSkinningData: CPUSkinningData option
    }

    type MeshRelation(md:DBMod, ref:DBReference) =
        let sw = new Util.StopwatchTracker("MeshRel:" + md.Name + "/" + ref.Name)
        let modMesh = 
            match md.Mesh with 
            | None -> failwith "cannot build vertrel for mod with no mesh"
            | Some (mesh) -> mesh
        let refMesh = ref.Mesh

        let buildTris (mesh:Mesh) =
            let tris = mesh.Triangles |> Array.map (fun iTri -> 
                    let derefed = iTri.Verts |> Array.map (fun vtn ->
                            let pos = refMesh.Positions.[vtn.Pos]
                            let tc = Vector2(0.f,0.f) // refMesh.UVs.[vtn.T]
                            let nrm = Vector3(1.f,0.f,0.f) //refMesh.Normals.[vtn.N]

                            (pos,tc,nrm)
                        )
                    let pos,tc,nrm = derefed |> Array.unzip3
                    { Tri.Position = pos; Tri.TexCoord = tc; Tri.Normal = nrm }
                )
            tris

        let isExcluded modIdx refIdx:bool =
            // use the vgroup annotations to determine whether a particular ref position should be included.
            // if the ref vert has a group annotated with "Exclude" it is excluded.
            // otherwise, if the ref vert has some annotations, and the mod vert also has annotations, they are compared as follows.
            // if the mod vert specifies an "Exclude.GNAME" and the ref vert has "GNAME", the ref vert is excluded.
            // if the mod vert specifies an "Include.GNAME" and the ref vert does not have "GNAME", the ref vert is excluded.
            // otherwise the ref vert is included

            // use active patterns to implement the rules; if a pattern returns Some(x), then the ref is excluded
            let (|UnconditionalExclude|_|) (refAnnts:string list,_:string list) = 
                refAnnts |> List.tryFind (fun (s:string) -> 
                    s.ToUpperInvariant().Equals("EXCLUDE"))

            let (|ModExcludesRef|_|) (refAnnts:string list,modAnnts:string list) =                
                refAnnts |> List.tryFind (fun refA ->
                    let refA = refA.ToUpperInvariant()
                    modAnnts 
                        |> List.tryFind (fun modA -> 
                            let modA = modA.ToUpperInvariant()
                            modA = "EXCLUDE." + refA
                        ) <> None
                )

            let (|ModIncludeNotFoundInRef|_|) (refAnnts:string list, modAnnts:string list) = 
                modAnnts 
                    |> List.tryFind (fun modA -> 
                        let modA = modA.ToUpperInvariant()

                        if not (modA.StartsWith("Include.", StringComparison.InvariantCultureIgnoreCase)) then
                            false
                        else
                            let modA = modA.ToUpperInvariant()
                            // search the refs for the target include
                            refAnnts |> List.tryFind (fun refA ->
                                let refA = refA.ToUpperInvariant()
                                
                                modA = "INCLUDE." + refA
                            ) = None
                    ) 

            if refMesh.AnnotatedVertexGroups.Length = 0 || modMesh.AnnotatedVertexGroups.Length = 0 then
                false
            else
                // should have warned about non-grouped verts on load; now just assume they are not excluded
                if refIdx >= refMesh.AnnotatedVertexGroups.Length then
                    false
                elif modIdx >= modMesh.AnnotatedVertexGroups.Length then
                    false
                else
                    let refAnnotations = refMesh.AnnotatedVertexGroups.[refIdx]
                    let modAnnotations = modMesh.AnnotatedVertexGroups.[modIdx]

                    match refAnnotations,modAnnotations with
                    | [],[] -> false
                    | UnconditionalExclude groupName ->                    
                        true
                    | ModExcludesRef groupName -> 
                        true
                    | ModIncludeNotFoundInRef groupName -> 
                        true
                    | _,_ -> false

        let buildVertRels():VertRel[] = 
            //  CPU-only: build triangles for ref and mod
            let modTris,refTris = 
                match modMesh.Type with 
                | CPUReplacement -> buildTris modMesh, buildTris refMesh
                | GPUReplacement
                | Deletion 
                | Reference -> [||],[||]
                
            let exclusionCheckingEnabled = true

            let exclusionFilter = if exclusionCheckingEnabled then isExcluded else (fun _ _ -> false)

            // for a single mod position index and value, find the relation data
            let getVertRel modIdx (modPos:Vec3F) = 
                let closestDist,closestIdx = 
                    let mutable currIdx = 0
                    let mutable closestDist = System.Single.MaxValue
                    let mutable closestIdx = -1

                    // this is a straight up, bad-ass linear search through the ref positions.
                    // this loop was pretty hot on the instrumentation profiler, but a lot of that was 
                    // because function calls like LengthSquared() only appear to be expensive because of the sheer 
                    // number of invocations.  but they still add up to something, so I reduced the intensity 
                    // by "inlining" the vector subtraction/distance calculations.  This saved about 12% on load times.
                    for refPos in ref.Mesh.Positions do
                        let vX = modPos.X - refPos.X
                        let vY = modPos.Y - refPos.Y
                        let vZ = modPos.Z - refPos.Z
                        let lenSqrd = //v.LengthSquared()
                             (vX) * (vX) +
                             (vY) * (vY) +
                             (vZ) * (vZ)
                        
                        if (lenSqrd >= closestDist) || (exclusionFilter modIdx currIdx) then
                            ()
                        else
                            closestDist <- lenSqrd
                            closestIdx <- currIdx
                        currIdx <- currIdx + 1
                    closestDist,closestIdx

                if closestIdx = -1 then
                    // wat
                    failwith "Unable to find closest index; if your mod is using an 'Include.XX' group, group XX may be missing from the ref"

                let closestDist = float32 (Math.Sqrt (float closestDist))

//                do
//                    printfn "%A: %A (%A)" modIdx closestIdx closestDist

                let cpuSkinningData = 
                    match modMesh.Type with 
                    | Reference 
                    | Deletion
                    | GPUReplacement -> None
                    | CPUReplacement ->
                        failwith "Looks like its time to implement cpu skinning relation code!"
                        //   CPU-only: find closest triangle containing ref vert
                        //   CPU-only: compute coordinate system
                        None

                {   RefPointIdx = closestIdx; 
                    ModVertPos = modPos; 
                    RefVertPos = ref.Mesh.Positions.[closestIdx]
                    Distance = closestDist 
                    CpuSkinningData = cpuSkinningData }
                                    
            // for all mod positions, find the relation data
            let modVertRels = modMesh.Positions |> Array.mapi getVertRel

            // warn if median distance is "large" (could be an indicator of mismatched transforms between ref and mod),
            // in which case the relation is going to be jacked.
            modVertRels 
                |> Array.sortBy (fun vr -> vr.Distance) 
                |> (fun sortedVRs ->
                    let mid = sortedVRs.Length / 2
                    let el = sortedVRs.[mid]
                    if (el.Distance > 1.f) then // threshold here is subjective
                        log.Warn "High median distance detected; ref & mod may not have same scale or other transforms applied: %A" el.Distance
                )

            modVertRels
    
        let vertRels = buildVertRels()

        do 
            log.Info "built mesh relation from mod '%s' to ref '%s'" md.Name ref.Name
            sw.StopAndPrint()
    
        member x.DBMod = md               
        member x.DBRef = ref
        member x.VertRelations = vertRels
        member x.ModMesh = modMesh
        member x.RefMesh = refMesh
        member x.ModVertRels = vertRels

        member x.GetVertDeclaration() =
            // currently, vertex declaration always comes from the ref
            refMesh.Declaration
