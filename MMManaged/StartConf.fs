namespace ModelMod

open System.IO

open YamlDotNet.RepresentationModel

open CoreTypes

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