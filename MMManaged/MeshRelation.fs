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

namespace ModelMod

open System
open System.IO

open Microsoft.Xna.Framework

open CoreTypes

/// Utilities for comparing a Mod to a Reference and copying any required data from the Ref to the Mod
/// (such as blend weight information).
module MeshRelation =
    let private log = Logging.getLogger("MeshRelation")

    /// Triangle data (essentially a de-normalized version of IndexedTri).
    type Tri = {
        // three elements each
        Position: Vec3F[];
        TexCoord: Vec2F[];
        Normal: Vec3F[];
    }

    type MVProjections = { X:float ; Y:float ; Z:float }

    /// Data required for CPU animations.  These are currently unimplemented, but this is some of the
    /// data that is needed using a method I prototyped before.  That method is roughly as follows
    /// 1) for each mod vert, compute an offset vector from the ref to the mod, with both in a neutral/root position
    /// The offset vector is computed from nearest vertex of the nearest triangle to the mod vert.  The centroids
    /// of each triangle containing the ref vert are used to find the nearest triangle to the mod vert (normals
    /// should probably be used as well so that double-sided triangles don't cause problems).
    /// 2) At run-time, on each frame, whenever the original ref is drawn, lock the VB and read back the
    /// ref data into system memory.  For each mod vert, re-compute the (current) projection triangle, and compute
    /// the projection vectors again.  Use the projection vectors to offset the ref verts.
    /// 3) Write a new vb using the new ref verts; draw it, and Voila, its the animated mod.
    /// Provided you aren't doing this for a huge amount of data, the performance hit barely registers (though, to be
    /// fair, the original code was in C++, so I don't know how well managed code would handle it).
    type CPUSkinningData = {
        UseRef: bool
        VecToModVert: Vec3F
        ModProjections: MVProjections
        RefIndices: int[]
        RefNormal: Vec3F
        //refTexCoord: Vec2F
    }

    /// Relation data for each ref/mod vert pair.
    type VertRel = {
        Distance: float32
        RefPointIdx: int
        ModVertPos: Vec3F
        RefVertPos: Vec3F
        CpuSkinningData: CPUSkinningData option
    }

    type MeshRelation(md:DBMod, ref:DBReference) =
        let verifyAndGet(name:string) (mo:Lazy<Mesh> option) =
            match mo with
            | None -> failwithf "cannot build vertrel for mod/ref with no mesh: %A" name
            | Some (m) -> m

        let mutable modMesh = None
        let mutable refMesh = None

        let mutable md = md
        let mutable ref = ref

        let updateDBElems (newMd:DBMod) (newRef:DBReference) =
            // can change the mod entries if the meshes didn't change or if the _only_ thing that changed is the cached flag
            // (and it was set to true)
            let newModMesh = (verifyAndGet newMd.Name newMd.Mesh).Force()
            let newRefMesh = (newRef.Mesh.Force())

            let oldModMesh = {modMesh.Value with Cached = true}
            let oldRefMesh = {refMesh.Value with Cached = true}

            // perfnote: these are deep equality compares of the structs
            if newModMesh <> oldModMesh then failwithf "updateDBElems: cannot change mod mesh; make a new mesh relation"
            if newRefMesh <> oldRefMesh then failwithf "updateDBElems: cannot change ref mesh; make a new mesh relation"
            md <- newMd
            ref <- newRef

        // Note: if this calculation is modified in the future to use something
        // other than mesh data, the caching assumptions on reload may change
        // (see `loadModDB`)

        let buildTris (mesh:Mesh) =
            let refMesh = refMesh.Value
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
                            let lonce = Logging.getLogOnceFn(md.Name + ref.Name + modA, 0)
                            lonce("using inclusion group: " + modA)
                            // search the refs for the target include
                            refAnnts |> List.tryFind (fun refA ->
                                let refA = refA.ToUpperInvariant()

                                let amatch = (modA = "INCLUDE." + refA)
                                if amatch then 
                                    let lonce = Logging.getLogOnceFn(md.Name + ref.Name + refA, 0)
                                    lonce("found in ref: " + refA)
                                amatch
                            ) = None
                    )

            let refMesh = refMesh.Value
            let modMesh = modMesh.Value
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
            let refMesh = refMesh.Value
            let modMesh = modMesh.Value

            // for CPU-skinning mods only: build triangles for ref and mod
            let modTris,refTris =
                match modMesh.Type with
                | CPUReplacement -> buildTris modMesh, buildTris refMesh
                | GPUReplacement
                | GPUAdditive
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

                    // This is a straight up, bad-ass linear search through the ref positions.
                    // This loop was pretty hot on the instrumentation profiler, but a lot of that was
                    // because function calls like LengthSquared() only appear to be expensive because of the sheer
                    // number of invocations. They still add up to something, so I reduced the intensity
                    // by "inlining" the vector subtraction/distance calculations.  This saved about 12% on load times.
                    for refPos in refMesh.Positions do
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
                    | GPUAdditive
                    | GPUReplacement -> None
                    | CPUReplacement ->
                        failwith "Looks like its time to implement cpu skinning relation code!"
                        //   CPU-only: find closest triangle containing ref vert
                        //   CPU-only: compute coordinate system
                        None

                {   RefPointIdx = closestIdx;
                    ModVertPos = modPos;
                    RefVertPos = refMesh.Positions.[closestIdx]
                    Distance = closestDist
                    CpuSkinningData = cpuSkinningData }

            // for all mod positions, find the relation data
            let modVertRels = modMesh.Positions |> Array.mapi getVertRel

            // warn if median distance is "large" (could be an indicator of mismatched transforms between ref and mod),
            // in which case the relation is going to be jacked.  This can produce false positives, though.
            modVertRels
                |> Array.sortBy (fun vr -> vr.Distance)
                |> (fun sortedVRs ->
                    let mid = sortedVRs.Length / 2
                    let el = sortedVRs.[mid]
                    if (el.Distance > 1.f) then // threshold here is subjective
                        log.Warn "High median distance detected; ref & mod may not have same scale or other transforms applied: %A" el.Distance
                )

            modVertRels
            
        let buildIt() = 
            //log.Info "Starting build of meshrelation for mod: %A" md.Name
            modMesh <- Some ((verifyAndGet md.Name md.Mesh).Force())
            refMesh <- Some ((ref.Mesh.Force()))

            let sw = new Util.StopwatchTracker("MeshRel:" + md.Name + "/" + ref.Name)
            let vertRels = buildVertRels()
            log.Info "built mesh relation from mod '%s' to ref '%s'" md.Name ref.Name
            sw.StopAndPrint()
            vertRels

        let vertRels = lazy (buildIt())

        member x.IsBuilt = vertRels.IsValueCreated 
        member x.Build() = vertRels.Force()

        member x.DBMod = md
        member x.DBRef = ref

        member x.UpdateDBElems(dbMod,dbRef) = updateDBElems dbMod dbRef

        /// If the MeshRelation has not been built this will be None
        member x.ModMesh = modMesh
        /// If the MeshRelation has not been built this will be None
        member x.RefMesh = refMesh

        member x.VertRelations = vertRels
        member x.ModVertRels = vertRels

        /// If the MeshRelation has not been built this will be None
        member x.GetVertDeclaration() =
            if x.IsBuilt then 
                None
            else 
                // currently, vertex declaration always comes from the ref
                refMesh.Value.Declaration
