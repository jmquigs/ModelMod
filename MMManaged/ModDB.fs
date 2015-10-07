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
open System.Text
open System.IO
open System.Collections.Generic

open Microsoft.FSharp.Core

open YamlDotNet.RepresentationModel
open Microsoft.Xna.Framework

open MeshRelation
open StartConf
open CoreTypes
open InteropTypes

module ModDB =
    let private log = Logging.getLogger("ModDB")

    let private strToLower (s:string option) =
        match s with
        | Some s -> Some (s.ToLower())
        | _ -> None

    let private (|StringValueIgnoreCase|_|) node = Yaml.toOptionalString(Some(node)) |> strToLower

    type ModDB(refObjects,modObjects,meshRels) =
        // explode deletion mods into interop representation now.  
        // this is a little weird because it makes moddb use interop types, but it 
        // is more convenient to do this here than in ModDBInterop.
        let deletionMods = 
            modObjects 
            |> List.filter (fun m -> not (List.isEmpty m.Attributes.DeletedGeometry))
            |> List.map (fun imod ->
                imod.Attributes.DeletedGeometry |> List.map (fun delPair -> 
                    { InteropTypes.EmptyModData with
                        InteropTypes.ModData.modType = 5
                        primType = 4
                        refVertCount = delPair.VertCount
                        refPrimCount = delPair.PrimCount
                    }
                )
            )
            |> List.concat          

        member x.References = refObjects
        member x.Mods = modObjects
        member x.MeshRelations = meshRels
        member x.DeletionMods = deletionMods

    let getMeshTransforms (node:YamlMappingNode) =
        let transforms = node |> Yaml.getOptionalValue "transforms" |> Yaml.toOptionalSequence
        match transforms with
        | None -> []
        | Some xforms -> xforms |> Seq.map Yaml.toString |> List.ofSeq

    let loadUncachedMesh(path, (modType:ModType), flags) = 
        let mesh = MeshUtil.readFrom(path, modType, flags)
        if flags.ReverseTransform && 
            (mesh.AppliedPositionTransforms.Length > 0 || mesh.AppliedUVTransforms.Length > 0) then
            let mesh = MeshTransform.reverseMeshTransforms (List.ofArray mesh.AppliedPositionTransforms) (List.ofArray mesh.AppliedUVTransforms) mesh 
            // clear out applied transforms, since they have been reversed.
            { mesh with AppliedPositionTransforms = [||]; AppliedUVTransforms = [||] }
        else
            mesh

    let loadMesh(path, (modType:ModType), flags) = 
        match MemoryCache.get (path,modType,flags) with 
        | None -> 
            let mesh = loadUncachedMesh(path,modType,flags)
            MemoryCache.save(path,modType,flags,mesh)
            mesh
        | Some mesh ->
            mesh
        
    let getModType = function
        | "cpureplacement" -> ModType.CPUReplacement
        | "gpureplacement" -> ModType.GPUReplacement        
        | "reference" -> ModType.Reference
        | "deletion" -> ModType.Deletion
        | x -> failwithf "unsupported mod type: %A" x

    let getWeightMode = function
        | "mod" -> WeightMode.Mod
        | "ref" -> WeightMode.Ref
        | "binaryref" -> WeightMode.BinaryRef
        | x -> failwithf "unsupported weight mode: %A" x
        
    let buildMod (node:YamlMappingNode) filename =
        let basePath = Path.GetDirectoryName filename
        let modName = Path.GetFileNameWithoutExtension filename

        let refName = node |> Yaml.getOptionalValue "ref" |> Yaml.toOptionalString

        let mesh,modType,weightMode,attrs =
            // TODO: should also support "modtype" here
            let sType = (node |> Yaml.getValue "meshtype" |> Yaml.toString).ToLower().Trim()
            let modType = getModType sType
            match modType with
            | ModType.Reference -> failwithf "Illegal mod mesh: type is set to reference: %A" node
            | ModType.Deletion
            | ModType.CPUReplacement
            | ModType.GPUReplacement -> ()

            // weight mode
            let weightMode = 
                let wstr = (node |> Yaml.getOptionalValue "weightmode" |> Yaml.toOptionalString)
                match wstr with
                | None -> WeightMode.Ref
                | Some s -> getWeightMode (s.ToLowerInvariant().Trim())

            // non-deletion and non-reference types require some refnames
            match (modType,refName) with
            | (ModType.Reference, _) 
            | (ModType.Deletion, _) -> ()
            | (ModType.CPUReplacement, None) 
            | (ModType.GPUReplacement, None) -> failwithf "Illegal mod mesh: type %A requires reference name, but it was not found: %A" modType node
            | (ModType.CPUReplacement, _) 
            | (ModType.GPUReplacement, _) -> ()

            let delGeometry = node |> Yaml.getOptionalValue "delGeometry" |> Yaml.toOptionalSequence
            let delGeometry = 
                match delGeometry with
                | None -> []
                | Some delSeq -> 
                    [ for c in delSeq.Children do
                        let node = Yaml.toMapping "expected an object for delGeometry element" c

                        yield { 
                            GeomDeletion.PrimCount = node |> Yaml.getValue "pc" |> Yaml.toInt
                            GeomDeletion.VertCount = node |> Yaml.getValue "vc" |> Yaml.toInt
                        }
                    ]

            let attrs = { EmptyModAttributes with DeletedGeometry = delGeometry }
            let mesh = 
                match modType with 
                | ModType.Deletion -> None
                | ModType.Reference 
                | ModType.CPUReplacement
                | ModType.GPUReplacement ->     
                    let meshPath = node |> Yaml.getValue "meshPath" |> Yaml.toString
                    if meshPath = "" then failwithf "meshPath is empty"
                    Some (loadMesh (Path.Combine(basePath, meshPath),modType,CoreTypes.DefaultReadFlags))

            // fill in texture paths (if any) from yaml
            let mesh = 
                match mesh with 
                | None -> None
                | Some(m) -> 
                    let useEmptyStringForMissing (x:string option) = 
                        match x with 
                        | None -> ""
                        | Some s when s.Trim() = "" -> ""
                        | Some s -> s
                    let makeAbsolute (path:string) =
                        match path with
                        | "" -> ""
                        | path when Path.IsPathRooted path -> path
                        | _ -> Path.GetFullPath(Path.Combine(basePath,path))

                    let unpack = Yaml.toOptionalString >> useEmptyStringForMissing >> makeAbsolute

                    Some({ m with 
                            Tex0Path = node |> Yaml.getOptionalValue "Tex0Path" |> unpack
                            Tex1Path = node |> Yaml.getOptionalValue "Tex1Path" |> unpack
                            Tex2Path = node |> Yaml.getOptionalValue "Tex2Path" |> unpack
                            Tex3Path = node |> Yaml.getOptionalValue "Tex3Path" |> unpack
                    })

            mesh,modType,weightMode,attrs

        let md = { 
            DBMod.RefName = refName
            Ref = None // defer ref resolution until all files have been loaded - avoids forward ref problems
            Name = modName
            Mesh = mesh
            WeightMode = weightMode
            Attributes = attrs
        }

        let numOverrideTextures = 
            match mesh with
            | None -> 0
            | Some mesh ->
                let oneIfNotEmpty (s:string) = if s.Trim() <> "" then 1 else 0
                oneIfNotEmpty mesh.Tex0Path + oneIfNotEmpty mesh.Tex1Path + oneIfNotEmpty mesh.Tex2Path + oneIfNotEmpty mesh.Tex3Path 

        log.Info "Mod: %A: type: %A, ref: %A, weightmode: %A, override textures: %d" modName modType refName weightMode numOverrideTextures
        Mod(md)

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

        let meshPath = node |> Yaml.getValue "meshpath" |> Yaml.toString
        let mesh = loadMesh (Path.Combine(basePath, meshPath),ModType.Reference,CoreTypes.DefaultReadFlags)

        // load vertex elements (binary)
        let binVertDeclPath = 
            let nval = node |> Yaml.getOptionalValue "VertDeclPath" 
            match nval with 
            // try alternate name if not found
            | None -> node |> Yaml.getOptionalValue "rawMeshVertDeclPath" |> Yaml.toOptionalString
            | _ -> nval |> Yaml.toOptionalString

        let declData = 
            match binVertDeclPath with
            | None -> None
            | Some path -> 
                let bytes,elements = loadBinVertDeclData (Path.Combine(basePath, path))
                log.Info "Found %d vertex elements in %s (%d bytes)" elements.Length path bytes.Length
                Some (bytes,elements)
                
        // load vertex data (binary)
        let binVertDataPath = node |> Yaml.getOptionalValue "rawMeshVBPath" |> Yaml.toOptionalString
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

        log.Info "Ref: %s: binary vertex data: %A" refName (if binVertData = None then false else true)

        MReference(
            { DBReference.Name = refName
              Mesh = mesh})
        
    let loadFile (conf:StartConf.Conf) (filename) =
        use sw = new Util.StopwatchTracker("load file: " + filename)

        let ext = Path.GetExtension(filename).ToLowerInvariant()

        match ext with 
        | ".yaml" ->
            let docs = Yaml.load filename
            let (objects:ModElement list) = [
                for d in docs do
                    let mapNode = Yaml.toOptionalMapping (Some(d.RootNode))
                    match mapNode with 
                    | Some mapNode -> 
                        // locate type field
                        let nType = mapNode |> Yaml.getValue "type"
                        match nType with 
                        | StringValueIgnoreCase "reference" ->
                            yield buildReference mapNode filename 
                        | StringValueIgnoreCase "mod" ->
                            yield buildMod mapNode filename 
                        | _ -> failwithf "Illegal 'type' field in yaml file: %s" filename
            
                    | _ -> failwithf "Don't know how to process yaml node type: %A in file %s" (d.RootNode.GetType()) filename
            ]

            objects
        | ".mmobj" ->
            let mesh = 
                // load it as a reference, but allow conf to control whether it should be transformed (normally it is, but if loading
                // for UI display, we might omit the transform because we want it displayed in tool format, not game-format)
                match conf.AppSettings with 
                | None -> 
                    loadMesh (filename,ModType.Reference, CoreTypes.DefaultReadFlags)
                | Some settings -> 
                    loadMesh (filename,ModType.Reference, settings.MeshReadFlags)

            let refName = Path.GetFileNameWithoutExtension filename
            [ MReference(
                { DBReference.Name = refName
                  Mesh = mesh})]
        | _ -> failwithf "Don't know how to load: %s" filename

    let loadIndexObjects (filename:string) (activeOnly:bool) conf =
        // load the index, find all the mods that we are interested in.
        use input = new StringReader(File.ReadAllText(filename))
        let yamlStream = new YamlStream()
        yamlStream.Load(input)
        let docCount = yamlStream.Documents.Count
        if (docCount <> 1) then failwithf "Too many documents in index file: %A: %d" filename docCount
        let mapNode = Yaml.toOptionalMapping (Some(yamlStream.Documents.[0].RootNode)) |> Option.get
        // type should be "index"
        let nType = mapNode |> Yaml.getValue "type"

        let modsToLoad = 
            match nType with
            | StringValueIgnoreCase "index" -> 
                // get the mod list
                let mods = mapNode |> Yaml.getValue "mods" |> Yaml.toSequence "'mods' sequence not found"
                let mods = 
                    mods 
                    |> Seq.map (fun modnode -> Yaml.toMapping "expected an object for 'mods' element" modnode )
                    |> Seq.filter (fun modMapping ->
                        let active = modMapping |> Yaml.getOptionalValue "active" |> Yaml.toBool true
                        (not activeOnly) || active)
                    |> Seq.map (fun (modMapping) ->
                        modMapping |> Yaml.getValue "name" |> Yaml.toString)
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
                // if some file was miscategorized, it may not actually be Mod - get rid of these
                | Mod(_) -> true
                | _ -> false)
            |> List.map (fun melem ->
                match melem with
                | Mod (imod) -> 
                    match imod.RefName with
                    | None -> ""
                    | Some name -> name.Trim()
                | Unknown 
                | MReference _ -> failwithf "derp, bad filtering: expected Mod but got %A" melem)
            |> List.filter (fun n -> n <> "" )
            |> Set.ofList // cheapo-dedup

        // walk the file list again, loading the references this time
        let refFiles =
            allFiles |> Array.filter (fun diskFile -> 
                    refsToLoad |> Seq.tryFind (fun loadRefFile -> nameMatches diskFile loadRefFile) <> None)   
        let refObjects = refFiles |> Array.map (loadFile conf) |> List.concat
                                             
        // return a list of the refs and objects
        modObjects @ refObjects

    let loadModDB(conf:StartConf.Conf) = 
        use sw = new Util.StopwatchTracker("LoadModDB")

        // read index if available, loading active mods (only) from index
        let indexObjects = 
            match conf.ModIndexFile with
            | None -> []
            | Some path -> loadIndexObjects path true conf

        let extraObjects = [ 
            for file in conf.FilesToLoad do
                yield! loadFile conf file
        ]

        let objects = indexObjects @ extraObjects

        let refs,mods = 
            objects |> 
                List.fold (fun acc x -> 
                    match x with 
                    | MReference ref -> (ref::(fst acc)), snd acc
                    | Mod mmod -> fst acc, mmod::(snd acc)
                    | Unknown -> failwith "unknown object type was loaded: %A x"
                ) ( ([]:DBReference list), ([]:CoreTypes.DBMod list))

        // verify that all required refs are loaded
        let lookupRef (refName:string option) =
            match refName with
            | None -> None
            | Some refName ->
                let found = refs |> List.tryFind (fun ref -> 
                    ref.Name.ToLower() = refName.ToLower() )
                match found with
                | Some(x) -> Some(x)
                | _ -> failwithf "failed to find reference with name: %s" refName

        let mods = mods |> List.map 
                    (fun m ->
                        let ref = lookupRef m.RefName
                        
                        { m with 
                            Ref = ref
                        }
                    )

        let meshRels = 
            mods 
            |> List.filter (fun m -> m.Ref <> None)
            |> List.map (fun m ->
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
