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

open System.IO

open YamlDotNet.RepresentationModel

open CoreTypes

/// Contains startup configuration utilities.  
/// Allows override of the standard registry and modindex load 
/// scheme.  Normally only used for non-game invocations
/// (preview window and MMView tool).  Unlike RunConfig, StartConf data is not stored in the registry.
module StartConf =
    // only use for MMView tool
    type WinSettings = {
        PosX: int
        PosY: int
        Width: int
        Height: int
        AllowResize: bool
    }
    type AppSettings = {
        Window: WinSettings option
        CamPosition: Vec3F option
        MeshReadFlags: MeshReadFlags
    }
    type Conf = { 
        ModIndexFile: string option
        FilesToLoad: string list
        AppSettings: AppSettings option // only present for the UI tools; MMView, etc
    }

    let loadConf confPath (appSettings:AppSettings option) = 
        let text = File.ReadAllText(confPath)
        use input = new StringReader(text)
        let yamlStream = new YamlStream()
        yamlStream.Load(input)
        let docCount = yamlStream.Documents.Count
        if (docCount <> 1) then
            failwithf "Expected 1 document, got %d" docCount
        let mapNode = Yaml.toMapping "No root node found" (yamlStream.Documents.[0].RootNode) 

        let modIndexFile = mapNode |> Yaml.getOptionalValue "modIndex" |> Yaml.toOptionalString

        let files = mapNode |> Yaml.getOptionalValue "files" |> Yaml.toOptionalSequence // Yaml "'Files' must be a list of files to load" |> Seq.map string |> List.ofSeq
        let files = 
            match files with
            | None -> []
            | Some files -> files |> Seq.map string |> List.ofSeq

        let conf = {
            ModIndexFile= modIndexFile
            FilesToLoad = files
            AppSettings = appSettings
        }

        Some (conf)