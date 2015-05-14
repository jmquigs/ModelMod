namespace ModelMod

open System
open System.Text
open System.IO
open System.Collections.Generic

open Microsoft.FSharp.Core

open YamlDotNet.RepresentationModel
open Microsoft.Xna.Framework

open MeshRelation
open MMView
open ModTypes
open InteropTypes

module ModDB =
    let private log = Logging.GetLogger("ModDB")

    let strToLower (s:string option) =
        match s with
        | Some s -> Some (s.ToLower())
        | _ -> None

    let (|StringValue|_|) node = Yaml.getScalar node
    let (|StringValueLower|_|) node = Yaml.getScalar node |> strToLower

    let loadMesh(path,(meshType:MeshType)) = MeshUtil.ReadFrom(path, meshType)       

    type MThing() = class end

    type MReference(name,mesh) =
        inherit MThing()
        interface IReference with
            member x.Name = name
            member x.Mesh = mesh

    type MMod(name,mesh,refName,ref,attrs) =
        inherit MThing()
        interface IMod with
            member x.RefName = refName
            member x.Ref = ref
            member x.Name = name
            member x.Mesh = mesh
            member x.Attributes = attrs

    type ModDB(refObjects,modObjects,meshRels) =
        // explode deletion mods into interop representation now.  TODO: this violates abstraction since it makes moddb use interop types.
        // perhaps this should be moved into the interop state
        let deletionMods = 
            modObjects 
            |> List.map (fun m -> m :> IMod)
            |> List.filter (fun m -> not (List.isEmpty m.Attributes.DeletedGeometry))
            |> List.map (fun imod ->
                imod.Attributes.DeletedGeometry |> List.map (fun delPair -> 
                    { 
                        InteropTypes.ModData.modType = 5
                        primType = 4
                        vertCount = 0 
                        primCount = 0
                        indexCount = 0
                        refVertCount = delPair.VertCount
                        refPrimCount = delPair.PrimCount
                        declSizeBytes = 0
                        vertSizeBytes = 0
                        indexElemSizeBytes = 0
                    }
                )
            )
            |> List.concat          

        member x.References = refObjects
        member x.Mods = modObjects
        member x.MeshRelations = meshRels
        member x.DeletionMods = deletionMods

    let getMeshTransforms (node:YamlMappingNode) =
        let transforms = Yaml.getOptionalValue node "transforms" |> Yaml.getSequence
        match transforms with
        | None -> []
        | Some xforms -> xforms |> Seq.map Yaml.getRequiredString |> List.ofSeq

    // Apply the list of named transforms to the specified mesh.  The invokeBuilder function allows you 
    // to change the order in which they are applied.  
    // NOTE: this implementation has some good points, but I'm sure it could be done more simply.
    let private applyMeshTransformsInternal (xforms:string list) (invokeBuilder) (mesh:Mesh) =
        if List.length xforms = 0 then
            mesh
        else
            // helper function to allow the mesh to be passed to parseXformFunc
            let getMeshXformFunc xname = MeshTransform.parseVec3XformFunc false mesh xname

            // convert all the names (and arguments) into functions
            let funcs = xforms |> List.map getMeshXformFunc
            // reduce to apply the functions in order with a single function
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (fun f1 f2 -> f1 >> f2)

            let mesh = MeshUtil.ApplyPositionTransformation compositeTransform mesh

            // also need to do normals
            let getMeshXformFunc xname = MeshTransform.parseVec3XformFunc true mesh xname
            let funcs = xforms |> List.map getMeshXformFunc
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (fun f1 f2 -> f1 >> f2)
            let mesh = MeshUtil.ApplyNormalTransformation compositeTransform mesh
            mesh

    let private applyUVTransformsInternal (uv_xforms:string list) (invokeBuilder) (mesh:Mesh) =
        if List.length uv_xforms = 0 then
            mesh
        else
            let getMeshXformFunc xname = MeshTransform.parseVec2XformFunc false mesh xname
            let funcs = uv_xforms |> List.map getMeshXformFunc
            let compositeTransform = funcs |> Seq.map invokeBuilder |> Seq.reduce (fun f1 f2 -> f1 >> f2)

            let mesh = MeshUtil.ApplyUVTransformation compositeTransform mesh
            mesh

    let applyMeshTransforms (xforms:string list) (uv_xforms:string list) (mesh:Mesh) = 
        let mesh = applyMeshTransformsInternal xforms MeshTransform.buildInvocation mesh 
        let mesh = applyUVTransformsInternal uv_xforms MeshTransform.buildInvocation mesh 
        mesh

    let reverseMeshTransforms (xforms:string list) (uv_xforms:string list) (mesh:Mesh) = 
        let mesh = applyMeshTransformsInternal (List.rev xforms) MeshTransform.buildReverseInvocation mesh 
        let mesh = applyUVTransformsInternal (List.rev uv_xforms) MeshTransform.buildReverseInvocation mesh 
        mesh

    let loadUntransformedMesh(path,(meshType:MeshType)) = MeshUtil.ReadFrom(path, meshType)

    let loadAndTransformMesh(path,(meshType:MeshType)) = 
        let mesh = loadUntransformedMesh(path, meshType)
        if mesh.AppliedPositionTransforms.Length > 0 || mesh.AppliedUVTransforms.Length > 0 then
            let mesh = reverseMeshTransforms (List.ofArray mesh.AppliedPositionTransforms) (List.ofArray mesh.AppliedUVTransforms) mesh 
            // clear out applied transforms, since they have been reversed.
            { mesh with AppliedPositionTransforms = [||]; AppliedUVTransforms = [||] }
        else
            mesh
        
    let getMeshType = function
        | "cpureplacement" -> MeshType.CPUReplacement
        | "gpureplacement" -> MeshType.GPUReplacement        
        | "reference" -> MeshType.Reference
        | "deletion" -> MeshType.Deletion
        | _ -> failwith "unsupported mod type"
        
    let buildMod (node:YamlMappingNode) filename =
        let basePath = Path.GetDirectoryName filename
        let modName = Path.GetFileNameWithoutExtension filename

        let refName = Yaml.getOptionalValue node "ref" |> Yaml.getOptionalString

        let mesh,attrs =
            let sType = (Yaml.getRequiredValue node "meshtype" |> Yaml.getString).ToLower().Trim()
            let meshType = getMeshType sType
            match meshType with
            | MeshType.Reference -> failwithf "Illegal mod mesh: type is set to reference: %A" node
            | MeshType.Deletion
            | MeshType.CPUReplacement
            | MeshType.GPUReplacement -> ()

            // non-deletion and non-reference types require some refnames
            match (meshType,refName) with
            | (MeshType.Reference, _) 
            | (MeshType.Deletion, _) -> ()
            | (MeshType.CPUReplacement, None) 
            | (MeshType.GPUReplacement, None) -> failwithf "Illegal mod mesh: type %A requires reference name, but it was not found: %A" meshType node
            | (MeshType.CPUReplacement, _) 
            | (MeshType.GPUReplacement, _) -> ()

            let delGeometry = Yaml.getOptionalValue node "delGeometry" |> Yaml.getSequence
            let delGeometry = 
                match delGeometry with
                | None -> []
                | Some delSeq -> 
                    [ for c in delSeq.Children do
                        yield { 
                            // TODO: omg the yaml interface is so bad
                            GeomDeletion.PrimCount = Yaml.getRequiredValue (Yaml.getRequiredMapping "expected an object for field type 'pc'" (Some c)) "pc" |> Option.get |> Yaml.getRequiredInt
                            GeomDeletion.VertCount = Yaml.getRequiredValue (Yaml.getRequiredMapping "expected an object for field type 'vc'" (Some c)) "vc" |> Option.get |> Yaml.getRequiredInt
                        }
                    ]

            let attrs = { EmptyModAttributes with DeletedGeometry = delGeometry }
            let mesh = 
                match meshType with 
                | MeshType.Deletion -> None
                | MeshType.Reference 
                | MeshType.CPUReplacement
                | MeshType.GPUReplacement ->     
                    let meshPath = Yaml.getRequiredValue node "meshPath" |> Yaml.getString
                    if meshPath = "" then failwithf "meshPath is empty"
                    Some (loadAndTransformMesh (Path.Combine(basePath, meshPath),meshType))
            mesh,attrs

        new MMod(modName,mesh,refName,None,attrs) // defer ref resolution

    let readVertexElement (reader:BinaryReader) =
        let stream = reader.ReadInt16() // hmm, these are actually uint16s, but sharpdx defines them as int16s
        let offset = reader.ReadInt16()
        let dtype = reader.ReadByte()
        let dmethod = reader.ReadByte()
        let usage = reader.ReadByte()
        let usageindex = reader.ReadByte()

        new SharpDX.Direct3D9.VertexElement(
            stream,
            offset,
            LanguagePrimitives.EnumOfValue<byte,SharpDX.Direct3D9.DeclarationType>(dtype),
            LanguagePrimitives.EnumOfValue<byte,SharpDX.Direct3D9.DeclarationMethod>(dmethod),
            LanguagePrimitives.EnumOfValue<byte,SharpDX.Direct3D9.DeclarationUsage>(usage),
            usageindex)

    let writeVertexElement (ve:SharpDX.Direct3D9.VertexElement) (writer:BinaryWriter) =
        writer.Write(ve.Stream)
        writer.Write(ve.Offset)
        writer.Write(byte ve.Type)
        writer.Write(byte ve.Method)
        writer.Write(byte ve.Usage)
        writer.Write(byte ve.UsageIndex)

    let loadBinVertDeclData (path:string) =
        let dat = File.ReadAllBytes(path)

        // its an array of D3DVERTEXELEMENT9 elements.  Use SharpDX9's container to hold the data.
        let structSize = 8 // bytes
        if (dat.Length % structSize <> 0) then
            failwithf "Binary vertex declaration array has unexpected size, should be a multiple of %A: size is: %A" structSize dat.Length

        let numElements = dat.Length / structSize
        let reader = new BinaryReader(new MemoryStream(dat))
        let elements = 
            [ for i in [1..numElements] do
                yield readVertexElement reader
            ] 
        dat, elements

    let loadBinVertData (path:string) =
        let dat = File.ReadAllBytes(path)

        // read header
        let memStream = new MemoryStream(dat)
        let reader = new BinaryReader(memStream)

        // num verts: uint32
        let numVerts = reader.ReadUInt32()
        // stride: uint32
        let stride = reader.ReadUInt32()
        // the rest is all binary vb data
        let vData = reader.ReadBytes(dat.Length - (int memStream.Position))

        {   
            BinaryVertexData.NumVerts = numVerts
            Stride = stride
            Data = vData
        }


    let buildReference (node:YamlMappingNode) filename =
        //log.Info "Building reference from %A" node

        let basePath = Path.GetDirectoryName filename
        let refName = Path.GetFileNameWithoutExtension filename

        let meshPath = Yaml.getRequiredValue node "meshpath" |> Yaml.getString
        let mesh = loadAndTransformMesh (Path.Combine(basePath, meshPath),MeshType.Reference)

        // load vertex elements (binary)
        let binVertDeclPath = 
            let nval = Yaml.getOptionalValue node "VertDeclPath" 
            match nval with 
            // try alternate name if not found
            | None -> Yaml.getOptionalValue node "rawMeshVertDeclPath" |> Yaml.getOptionalString
            | _ -> nval |> Yaml.getOptionalString

        let declData = 
            match binVertDeclPath with
            | None -> None
            | Some path -> 
                let bytes,elements = loadBinVertDeclData (Path.Combine(basePath, path))
                log.Info "Found %d vertex elements in %s (%d bytes)" elements.Length path bytes.Length
                Some (bytes,elements)
                
        // load vertex data (binary)
        let binVertDataPath = Yaml.getOptionalValue node "rawMeshVBPath" |> Yaml.getOptionalString
        let binVertData = 
            match binVertDataPath with
            | None -> None
            | Some path ->
                let vdata = loadBinVertData (Path.Combine(basePath, path))
                log.Info "Found %d verts in %s (%d bytes)" vdata.NumVerts path vdata.Data.Length 
                Some vdata

//        let sw = new Util.StopwatchTracker("apply transforms: " + filename)
//        let mesh = applyMeshTransforms (getMeshTransforms node) mesh
//        sw.StopAndPrint()

        let mesh = { mesh with BinaryVertexData = binVertData; Declaration = declData }
        new MReference(refName,mesh)
        
    let loadFile (conf:MMView.Conf) (filename) =
        use sw = new Util.StopwatchTracker("load file: " + filename)

        let ext = Path.GetExtension(filename).ToLowerInvariant()

        match ext with 
        | ".yaml" ->
            use input = new StringReader(File.ReadAllText(filename))
            let yamlStream = new YamlStream()
            yamlStream.Load(input)
            let mapping = yamlStream.Documents.Count
            let (objects:MThing list) = [
                for d in yamlStream.Documents do
                    let mapNode = Yaml.getMapping (Some(d.RootNode))
                    match mapNode with 
                    | Some mapNode -> 
                        // locate type field
                        let nType = Yaml.getRequiredValue mapNode "type"
                        match nType.Value with 
                        | StringValueLower "reference" ->
                            yield buildReference mapNode filename :> MThing
                        | StringValueLower "mod" ->
                            yield buildMod mapNode filename :> MThing
                        | _ -> failwithf "Illegal 'type' field in yaml file: %s" filename
            
                    | _ -> failwithf "Don't know how to process yaml node type: %A in file %s" (d.RootNode.GetType()) filename
            ]

            objects
        | ".mmobj" ->
            let mesh = 
                // load it as a reference, but allow conf to control whether it should be transformed (normally it is, but if loading
                // for UI display, we might omit the transform because we want it displayed in tool format, not game-format)
                match conf.AppSettings with 
                | Some settings when settings.Transform = false -> loadUntransformedMesh (filename,MeshType.Reference)
                | _ -> loadAndTransformMesh (filename,MeshType.Reference)
            let refName = Path.GetFileNameWithoutExtension filename
            [ new MReference(refName,mesh) ]
        | _ -> failwithf "Don't know how to load: %s" filename

    let LoadIndexObjects (filename:string) (activeOnly:bool) conf =
        // load the index, find all the mods that we are interested in.
        use input = new StringReader(File.ReadAllText(filename))
        let yamlStream = new YamlStream()
        yamlStream.Load(input)
        let docCount = yamlStream.Documents.Count
        if (docCount <> 1) then failwithf "Too many documents in index file: %A: %d" filename docCount
        let mapNode = Yaml.getMapping (Some(yamlStream.Documents.[0].RootNode)) |> Option.get
        // type should be "index"
        let nType = Yaml.getRequiredValue mapNode "type"

        let modsToLoad = 
            match nType.Value with
            | StringValueLower "index" -> 
                // get the mod list
                let mods = Yaml.getRequiredValue mapNode "mods" |> Yaml.getRequiredSequence "'mods' sequence not found"
                let mods = 
                    mods 
                    |> Seq.map (fun modnode -> Yaml.getMapping(Some modnode) |> Option.get )
                    |> Seq.filter (fun modMapping ->
                        let active = Yaml.getOptionalValue modMapping "active" |> Yaml.getOptionalBool true
                        (not activeOnly) || active)
                    |> Seq.map (fun (modMapping) ->
                        Yaml.getRequiredValue modMapping "name" |> Yaml.getString)
                    |> List.ofSeq
                mods
            | _ -> failwith "Expected data with 'type: \"Index\"' in %s"  filename

        // get a list of all the yaml files in all subdirectories beneath the index file.
        let searchRoot = Directory.GetParent(filename).FullName
        let allFiles = Directory.GetFiles(searchRoot, "*.yaml", SearchOption.AllDirectories)

        // walk the file list, loading the mods that are on the load list
        let nameMatches f1 f2 =
            f1 = f2 ||
            Path.GetFileNameWithoutExtension(f1).ToLowerInvariant() = Path.GetFileNameWithoutExtension(f2).ToLowerInvariant()

        let modFiles = 
            modsToLoad |> List.fold (fun acc modName -> 
                let foundFile = allFiles |> Array.tryFind (fun diskFile -> nameMatches diskFile modName)
                match foundFile with 
                | None -> 
                    log.Warn "No mod file found for mod named '%s'" modName
                    acc
                | Some file -> 
                    file::acc
            ) []

        let modObjects = modFiles |> List.map (loadFile conf) |> List.concat

        // examine all the mods and get list of ref files to load
        let refsToLoad = 
            modObjects 
            |> List.filter (function
                // if some file was miscategorized, it may not actually be Mod
                | :? MMod -> true
                | _ -> false)
            |> List.map (fun mthing ->
                let imod = (mthing :?> MMod) :> IMod
                match imod.RefName with
                | None -> ""
                | Some name -> name.Trim())
            |> List.filter (fun n -> n <> "" )
            |> Set.ofList // cheapo-dedup

        // walk the file list again, loading the references this time
        let refFiles =
            allFiles |> Array.filter (fun diskFile -> 
                    refsToLoad |> Seq.tryFind (fun loadRefFile -> nameMatches diskFile loadRefFile) <> None)   
        let refObjects = refFiles |> Array.map (loadFile conf) |> List.concat
                                             
        // return a list of the refs and objects
        modObjects @ refObjects

    let LoadModDB(conf:MMView.Conf) = 
        use sw = new Util.StopwatchTracker("LoadModDB")

        // read index if available, loading active mods (only) from index
        let indexObjects = 
            match conf.ModIndexFile with
            | None -> []
            | Some path -> LoadIndexObjects path true conf

        let extraObjects = [ 
            for file in conf.FilesToLoad do
                yield! loadFile conf file
        ]

        let objects = indexObjects @ extraObjects

        let refs,mods = 
            objects |> 
                List.fold (fun acc x -> 
                    match x with 
                    | :? MReference as ref -> (ref::(fst acc)), snd acc
                    | :? MMod as mmod -> fst acc, mmod::(snd acc)
                    | _ -> failwith "unknown object type was loaded"
                ) ( ([]:MReference list), ([]:MMod list))

        // verify that all required refs are loaded
        let lookupRef (refName:string option) =
            match refName with
            | None -> None
            | Some refName ->
                let found = refs |> List.tryFind (fun x -> 
                    let ref = x :> IReference
                    ref.Name.ToLower() = refName.ToLower() )
                match found with
                | Some(x) -> Some(x :> IReference)
                | _ -> failwithf "failed to find reference with name: %s" refName

        let mods = mods |> List.map 
                    (fun m ->
                        let m = m :> IMod
                        let ref = lookupRef m.RefName
                        
                        new MMod(m.Name,m.Mesh,m.RefName,ref,m.Attributes)
                    )

        let meshRels = 
            mods 
            |> List.filter (fun m -> (m :> IMod).Ref <> None)
            |> List.map (fun m ->
                let m = m :> IMod
                new MeshRelation(m, Option.get m.Ref)
            )

        new ModDB(refs,mods,meshRels)        

    let createUsageOffsetLookupMap(elements:SDXVertexElement list) =
        // create a map of the elements by usage so that we can quickly look up the offset of a given usage.
        // actually this is an array, because the usage values are really small, and using an array is a bit faster
        // that a mutable dictionary - roughly 33% as measured.  Its almost 10x faster than an immutable dictionary.
        let elements = 
            elements 
            // filter out unused elements and usageindexes > 0 (they are just repeats)
            |> List.filter (fun el -> el.Type <> SDXVertexDeclType.Unused && el.UsageIndex = (byte 0))
        let min = elements |> List.minBy (fun el -> el.Usage)
        let max = elements |> List.maxBy (fun el -> el.Usage)

        let minIdx = int min.Usage
        let maxIdx = int max.Usage

        do if (minIdx < 0) then failwith "Invalid minimum index"

        let lookupArray:int[] = Array.zeroCreate (maxIdx + 1)
        let offsetLookup = elements |> List.fold (fun (arr:int[]) el -> arr.[int el.Usage] <- int el.Offset; arr ) lookupArray
        offsetLookup 

    type BinaryLookupHelper(bvd:BinaryVertexData,elements:SDXVertexElement list) =
        let ms = new MemoryStream(bvd.Data)
        let br = new BinaryReader(ms)

        let stride = int bvd.Stride

        let offsetLookup = createUsageOffsetLookupMap elements

        // return a reader into the binary data for the given vertex index and usage.  
        // caller should use the element type to determine how much data to read in what format.
        // caller must not dispose or close the reader.
        member x.BinaryReader(vertIdx:int,usage:SDXVertexDeclUsage) =
            let offset = (vertIdx * stride) + offsetLookup.[int usage]
            ms.Seek(int64 offset, SeekOrigin.Begin) |> ignore
            br

// This is needed for interop runs, where we need to keep the loaded ModDB state somewhere but we don't want
// to pass it over the interop barrier directly.
module State =
    let private log = Logging.GetLogger("State")

    // The data directory contains all data for all games, as well as the selection texture.
    let private DefaultDataDir = "Data"
    // This is another name for the data directory.  If a directory exists with this name, it is used instead of the default.  If a file
    // exists and it contains a single line that is an absolute path to another directory that exists, that directory is used instead 
    // (i.e., it acts like a symlink.)
    let private SymlinkName = "MMData" 

    let mutable Moddb = new ModDB.ModDB([],[],[])
    let mutable RootDir = "."
    let mutable ExeModule = ""
    let mutable Conf = Types.DefaultRunConfig
    let mutable realDataDir = ""

    let ValidateAndSetConf (conf:Types.RunConfig): Types.RunConfig =
        let snapProfile = 
            match conf.SnapshotProfile with
            | profile when (SnapshotProfiles.ValidProfiles |> List.exists (fun p -> p.ToLowerInvariant() = profile.ToLowerInvariant() )) -> profile.ToLowerInvariant()
            | _ ->
                let def = SnapshotProfiles.Profile1
                log.Info "Unrecognized snapshot profile: %A; using %A" conf.SnapshotProfile def
                def.ToLowerInvariant()
            
        let conf = 
            { conf with
                SnapshotProfile = snapProfile
            }
        log.Info "Conf: %A" conf
        Conf <- conf
        conf

    let private initDataDir() =
        let dPath = Path.Combine(RootDir,SymlinkName)
        let dPath = 
            if Directory.Exists(dPath) then dPath 
            else
                if File.Exists(dPath) then
                    let symLink = File.ReadAllText(dPath).Trim()
                    if Directory.Exists(symLink) then
                        symLink
                    else
                        failwithf "Sym link found in '%s' but the target directory '%s' does not exist" dPath symLink
                else
                    // symlink not found, use the default dir
                    let dPath = Path.Combine(RootDir,DefaultDataDir)
                    if not (Directory.Exists(dPath)) then
                        failwithf "Cannot initialize data directory: %s" dPath
                    else 
                        dPath
                    
        realDataDir <- dPath
                  
    let getBaseDataDir() = 
        if realDataDir = "" then
            initDataDir()
        if realDataDir = "" then
            failwith "Unable to init data dir"

        let exeDataDir = Path.Combine(RootDir,realDataDir)
        exeDataDir

    let getExeBaseName() = 
        let exeBase = Path.GetFileNameWithoutExtension(ExeModule.ToLowerInvariant())
        exeBase

    let getExeDataDir() = 
        let exeDataDir = Path.Combine(getBaseDataDir(),getExeBaseName())
        exeDataDir

    let getExeSnapshotDir() =
        let dataDir = getExeDataDir();
        Path.Combine(dataDir,"snapshots")
              
