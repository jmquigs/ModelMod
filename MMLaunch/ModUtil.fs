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

namespace MMLaunch

open System
open System.IO

open YamlDotNet.RepresentationModel
open YamlDotNet.Serialization

open ViewModelUtil

/// Helper for DDS files, mostly LLM generated
module DdsUtil =
    open System
    open System.IO
    open System.Text

    type HeaderInfo = { FourCC : string; HasDX10 : bool }

    let PathToTexCli = [ "TPlib"; "..\\TPLib"; "..\\..\\TPLib"; "..\\..\\..\\TPLib";  ]
    let PathToSnapshotProfiles = [ "SnapshotProfiles"; "..\\SnapshotProfiles"; "..\\..\\SnapshotProfiles"; "..\\..\\..\\SnapshotProfiles" ]
    let SnapTexFormatFile = "TexFormat.txt"
    let TexConvName = "texconv.exe"

    /// Returns the absolute path of the first existing directory in a list of paths, or None if none exist
    let findFirstExistingPath paths : string option =
        // Map each path to its full path based on the current directory
        let fullPaths = paths |> List.map Path.GetFullPath
        // Find the first path (if any) that exists from the mapped paths
        fullPaths |> List.tryFind Directory.Exists

    let readHeader (path : string) : HeaderInfo =
        use br = new BinaryReader(File.OpenRead path)

        if br.ReadUInt32() <> 0x20534444u then
            invalidOp "Not a DDS file."

        br.BaseStream.Seek(84L, SeekOrigin.Begin) |> ignore
        let fourCCu = br.ReadUInt32()
        let fourCC  = Encoding.ASCII.GetString(BitConverter.GetBytes fourCCu)

        { FourCC = fourCC; HasDX10 = (fourCC = "DX10") }

    /// Read the 4-byte FourCC at offset 84 (0x54) in the DDS header
    /// Returns true if it equals "DX10"
    let needsConversion (filePath : string) : bool =
        (readHeader filePath).HasDX10

    open System.Globalization
    open System.Diagnostics

    let readSnapTexFormatFile () : string * Map<uint32, string> =
        // Find the first existing path for SnapTexFormatFile
        let filePath =
            PathToSnapshotProfiles
            |> List.map (fun dir -> Path.Combine(dir, SnapTexFormatFile))
            |> List.tryFind File.Exists

        // If no path is found, fail with an error
        let filePath =
            match filePath with
            | Some path -> path
            | None -> failwithf "%A not found in any of the specified paths: %A (relative to: %A)" SnapTexFormatFile PathToSnapshotProfiles (Environment.CurrentDirectory)

        // Read the file and process each line
        // syntax: src dxgi input format number (from dxgiformat.h) = list of flags to be supplied to texconv
        // example: 71 = -f BC1_UNORM -dx9
        let lines = File.ReadAllLines(filePath)
        let kvpList =
            lines
            |> Array.choose (fun line ->
                let trimmedLine = line.Trim()

                // Skip blank lines or comments
                if String.IsNullOrWhiteSpace(trimmedLine) || trimmedLine.StartsWith("//") || trimmedLine.StartsWith("#") then
                    None
                else
                    // Find the position of '='
                    let equalIndex = trimmedLine.IndexOf('=')
                    if equalIndex >= 0 then
                        let keyPart = trimmedLine.Substring(0, equalIndex).Trim()
                        let valuePart = trimmedLine.Substring(equalIndex + 1).Trim()

                        // Parse the key
                        let key =
                            if keyPart.ToLowerInvariant().StartsWith("0x") then
                                let hexValue = keyPart.Substring(2)
                                UInt32.Parse(hexValue, NumberStyles.HexNumber)
                            else
                                UInt32.Parse(keyPart)

                        // Use the trimmed value as the map value
                        Some (key, valuePart)
                    else
                        None // If no '=' is found, skip the line
            )

        // Convert the list of key-value pairs to a map
        filePath,Map.ofArray kvpList

    let readFormat(path:string) = 
        let hdr = readHeader path 
        if not hdr.HasDX10 then failwithf "file format read only supported on dx10+ dds texture files"

        use br = new BinaryReader(File.OpenRead path)
        
        br.BaseStream.Seek(0x80L, SeekOrigin.Begin) |> ignore
        br.ReadUInt32()

    /// Converts a DDS file based on its format.
    /// - `filePath`: The full absolute path to the DDS texture file.
    let convertFile (filePath: string) =
        // Read the format of the DDS file.
        let format = readFormat filePath

        // Read the SnapTexFormatFile map.
        let formatFile,formatMap = readSnapTexFormatFile()

        // Lookup the read format in the map.
        let convertArgs = 
            match formatMap.TryFind(format) with
            | Some flags -> flags // Return the found flags.
            | None -> failwithf "DDS DXGI Format %A used by texture was not found in the format map; it needs to be added to %A" format formatFile

        // Determine the path to TexConvName using PathToTexCli, fail with a formatted message if not found.
        let texConvPath =
            match findFirstExistingPath PathToTexCli with
            | Some path -> Path.Combine(path, TexConvName)
            | None -> failwithf "%s not found in any of the specified paths: %A (relative to: %A)" TexConvName PathToTexCli (Environment.CurrentDirectory)

        // Validate that the executable exists at the determined path.
        if not (File.Exists(texConvPath)) then
            failwithf "TexConv executable not found at %s" texConvPath

        // Prepare the command line arguments.
        let arguments = sprintf "-y %s \"%s\"" convertArgs filePath

        // Set up the process start info.
        let startInfo =
            ProcessStartInfo(
                FileName = texConvPath,
                Arguments = arguments,
                UseShellExecute = false,
                WorkingDirectory = Path.GetDirectoryName(filePath),
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true
            )

        // Execute the process.
        use proc = new Process()
        proc.StartInfo <- startInfo

        let mutable errorOut = "";
        let mutable stdOut = "";

        proc.OutputDataReceived.Add(fun args -> if not (String.IsNullOrWhiteSpace(args.Data)) then stdOut <- stdOut + args.Data)
        proc.ErrorDataReceived.Add(fun args -> if not (String.IsNullOrWhiteSpace(args.Data)) then errorOut <- errorOut + args.Data)

        // Start the process and begin reading output and error streams asynchronously.
        proc.Start() |> ignore
        proc.BeginOutputReadLine()
        proc.BeginErrorReadLine()

        // Wait for the process to exit.
        proc.WaitForExit()

        // Check if the process exited with a non-zero exit code.
        if proc.ExitCode <> 0 then
            failwithf "TexConv process failed with exit code %d (error output: %A)" proc.ExitCode errorOut


module ModUtil =

    type YamlRef = {
        Type: string // type of the yaml file (mod, reference, index)
        MeshPath: string
        VertDeclPath: string
        ExpectedPrimCount: int
        ExpectedVertCount: int
    }

    type YamlMod = {
        Type: string // type of the yaml file (mod, reference, index)
        Ref: string
        ModType: string // subtype of the mod (gpureplacement, etc)
        MeshPath: string
        Profile: ModelMod.CoreTypes.SnapProfile
    }

    let getOutputPath modRoot modName = Path.GetFullPath(Path.Combine(modRoot, modName))

    type ModFilePath = string
    type Message = string

    let modIndexTemplate = """type: "Index"
mods:"""
    let addToModIndex (modRoot:string) (modFile:string):Result<unit,Message> =
        try
            // yamldotnot doesn't seem to produce comment/text nodes in the representation model.
            // so rather than lose those, just operate on it as a text file.
            let indexFileName = Path.Combine(modRoot, "ModIndex.yaml")
            let modTextLines = 
                if File.Exists indexFileName then
                    File.ReadAllLines(indexFileName)
                else
                    modIndexTemplate.Split([| "\r\n"; "\n" |], StringSplitOptions.None);
            
            let modName = Path.GetFileNameWithoutExtension modFile
            let modName = sprintf "\"%s\"" modName

            let found = modTextLines |> Array.tryFind (fun l -> l.Contains modName)

            match found with
            | Some l -> Ok(())
            | None ->
                // find the "mods:" line, insert new mod after that so that we don't need to find the terminator
                let modsLineIdx = 
                    match (modTextLines |> Array.tryFindIndex (fun l -> l.ToLowerInvariant().StartsWith("mods:"))) with
                    | None ->
                        // derp
                        failwith "Can't find 'mods:' line"
                    | Some idx -> idx
                    
                let start = modTextLines |> Seq.take (modsLineIdx+1) 
                let rest = modTextLines |> Seq.skip (modsLineIdx+1) 
                let newModLine = seq { yield (sprintf "- {name: %s}" modName) } 
                
                let lines = Seq.concat [ start; newModLine; rest ] 

                File.WriteAllLines(indexFileName, lines)

                Ok(())
        with 
            | e -> Err(e.Message)
    
    let createMod (modRoot:string) (modName:string) (convertTextures:bool) (srcMMObjFile:string) : Result<ModFilePath,Message> = 
        try
            let modName = modName.Trim()
            if modName = "" then 
                failwith "Please enter a mod name"

            if (not (File.Exists srcMMObjFile)) then 
                failwith "Please verify that the source file exists"
        
            let modOutDir = getOutputPath modRoot modName
            if (Directory.Exists modOutDir) then 
                failwithf "Mod directory already exists, try a different mod name? dir: %s" modOutDir

            // make sure filename conforms to expected format
            let (|SnapshotFilename|_|) pattern str = 
                ModelMod.REUtil.checkGroupMatch pattern 4 str 
                |> ModelMod.REUtil.extract 1 int32 
                |> (fun (intParts: int [] option) ->
                        match intParts with
                        | None -> None
                        | Some parts -> // omit snapshot number but return primcount, vertcount
                            Some (parts.[1],parts.[2]))

            let pCount,vCount = 
                match srcMMObjFile.ToLowerInvariant() with 
                | SnapshotFilename @"snap_(\S+)_(\d+)p_(\d+)v.*" parts -> parts
                | _ -> failwithf "Illegal snapshot filename; cannot build a ref from it: %s" srcMMObjFile

            let srcBasename = Path.GetFileNameWithoutExtension(srcMMObjFile)
            let snapSrcDir = Path.GetDirectoryName(srcMMObjFile)
            let refBasename = modName + "Ref"
            let modBasename = modName + "Mod"

            Directory.CreateDirectory modOutDir |> ignore

            // copy vb declaration
            let vbDeclFile =
                let declExt = ".dat"
                let declSuffix = "_VBDecl"
                let declSrc = Path.Combine(snapSrcDir, srcBasename + declSuffix + declExt)
                if File.Exists(declSrc) then
                    let newDeclFile = Path.Combine(modOutDir, refBasename + declSuffix + declExt)
                    File.Copy(declSrc,newDeclFile)
                    newDeclFile
                else 
                    ""

            // copy mtl file, texture file, and rename texture; note, this only supports one texture right now
            let kdMarker = "map_Kd "

            let mtlFile = 
                let mtlExt = ".mtl"
                let mtlSrc = Path.Combine(snapSrcDir, srcBasename + mtlExt)
                if not (File.Exists(mtlSrc)) then None
                else
                    let newMtlFile = Path.Combine(modOutDir, refBasename + mtlExt)
                    let fDat = File.ReadAllLines(mtlSrc)
                    let fDat = fDat |> Array.map (fun line -> 
                        match line with
                        | l when l.StartsWith(kdMarker) -> 
                            // copy the texture file and rename it 
                            let texFile = l.Replace(kdMarker, "").Trim()
                            let texFile = 
                                let texSrc = Path.Combine(snapSrcDir, texFile)
                                if (File.Exists(texSrc)) then 
                                    let texExt = ".dds"
                                    let texBN = refBasename + texExt
                                    let newTexFile = Path.Combine(modOutDir, texBN)
                                    
                                    File.Copy(texSrc,newTexFile)

                                    if convertTextures && DdsUtil.needsConversion texSrc then 
                                        DdsUtil.convertFile newTexFile

                                    texBN
                                else
                                    texFile
                            
                            kdMarker + Path.GetFileName(texFile)
                        | l -> l)
                    File.WriteAllLines(newMtlFile, fDat)
                    Some newMtlFile

            // copy mmobj and rename mtl
            let refMMObjFile = 
                // we already checked that the src exists
                let newMMObjFile = Path.Combine(modOutDir, refBasename + ".mmobj")
                let fDat = File.ReadAllLines(srcMMObjFile)
                let fDat = fDat |> Array.map (fun line ->
                    match line with
                    | l when mtlFile <> None && l.StartsWith("mtllib ") -> "mtllib " +  Path.GetFileName(Option.get mtlFile)
                    | l when l.StartsWith("o ") -> "o " + modName 
                    | l -> l)
                File.WriteAllLines(newMMObjFile, fDat)
                newMMObjFile

            // generate a default mod file that is a copy of the ref
            let modMMObjFile = 
                let modMMObjFile = Path.Combine(modOutDir, modBasename + ".mmobj")
                File.Copy(refMMObjFile,modMMObjFile)
                modMMObjFile

            // generate ref yaml
            let refYamlFile = 
                let refYamlFile = Path.Combine(modOutDir, refBasename + ".yaml")
                let refObj = {
                    YamlRef.Type = "Reference"
                    YamlRef.MeshPath = Path.GetFileName(refMMObjFile)
                    YamlRef.VertDeclPath = if vbDeclFile <> "" then Path.GetFileName(vbDeclFile) else ""
                    YamlRef.ExpectedPrimCount = pCount
                    YamlRef.ExpectedVertCount = vCount
                }
                let sr = new Serializer()
                use sw = new StreamWriter(refYamlFile)
                sr.Serialize(sw, refObj) 
                refYamlFile

            // look for a snapshot meta file which may contain information about what profile was used
            let metaFile = Path.Combine(snapSrcDir, srcBasename + "_Meta.yaml")
            let snapProfile = 
                if File.Exists(metaFile) then 
                    let sd = new Deserializer()
                    use f = File.OpenText(metaFile);
                    let p = sd.Deserialize<ModelMod.Snapshot.SnapMeta>(f)
                    p.Profile
                else 
                    ModelMod.SnapshotProfile.EmptyProfile

            // generate mod yaml 
            let modYamlFile = 
                let modYamlFile = Path.Combine(modOutDir, modBasename + ".yaml")
                let modObj = { 
                    YamlMod.Type = "Mod"
                    ModType = "GPUReplacement"
                    Ref = refBasename
                    MeshPath = Path.GetFileName(modMMObjFile)
                    Profile = snapProfile
                }
                let sr = new Serializer()
                use sw = new StreamWriter(modYamlFile)
                sr.Serialize(sw, modObj) 
                modYamlFile

            Ok(modMMObjFile)
        with 
            | e -> Err(e.Message)
