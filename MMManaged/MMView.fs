namespace ModelMod

open System.IO

open YamlDotNet.RepresentationModel

open Types

module MMView =
    type WinSettings = {
        PosX: int
        PosY: int
        Width: int
        Height: int
        AllowResize: bool
    }
    type AppSettings = {
        Window: WinSettings option
        Transform: bool
        CamPosition: Vec3F option
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
        let mapNode = Yaml.getRequiredMapping "No root node found" (Some(yamlStream.Documents.[0].RootNode)) 

        let modIndexFile = Yaml.getOptionalValue mapNode "modIndex" |> Yaml.getOptionalString

        let files = Yaml.getOptionalValue mapNode "files" |> Yaml.getSequence // Yaml "'Files' must be a list of files to load" |> Seq.map string |> List.ofSeq
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