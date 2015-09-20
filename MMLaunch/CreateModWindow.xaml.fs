namespace MMLaunch

open System
open System.Diagnostics
open System.Threading
open System.Windows
open System.IO
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input
open System.ComponentModel
open System.Collections.ObjectModel
open Microsoft.Win32

open FsXaml

open ViewModelUtil
open ModelMod

type CreateModView = XAML<"CreateModWindow.xaml", true>

type MMObjFileModel(fullPath) =     
    member x.Name 
        with get() = 
            Path.GetFileName(fullPath)
    member x.FullPath 
        with get() = fullPath

type CreateModViewModel() = 
    inherit ViewModelBase()

    let mutable snapDir = ""
    let mutable dataDir = ""
    let mutable targetMMObjFile:MMObjFileModel = MMObjFileModel("")
    let mutable modName = ""
    let mutable previewHost: PreviewHost option = None
    let mutable mmobjFiles = new ObservableCollection<MMObjFileModel>([])
    let mutable addToModIndex = true
    
    let validateModName (mn:string):Result<string,string> =
        let illegalChars = [|'/'; '\\'; ':'; '*'; '?'; '"'; '<'; '>'; '|'|]

        let mn = mn.Trim()

        match mn with
        | "" -> Err("Enter mod name")
        | s when s.IndexOfAny(illegalChars) >= 0 -> Err(sprintf "Mod name cannot contain any of: %A" illegalChars)
        | s when s.Contains("..") -> Err("Mod name cannot contain ..")
        | s when Directory.Exists(Path.Combine(dataDir,mn)) -> Err("Directory already exists, please choose a different mod name")
        | s ->
            Ok(Path.Combine(dataDir,mn))

    // SnapshotDir,DataDir,PreviewHost are usually just set once on view creation 

    member x.SnapshotDir 
        with get() = snapDir
        and set value = 
            snapDir <- value
            mmobjFiles.Clear()
            if Directory.Exists snapDir then
                Directory.GetFiles (snapDir, "*.mmobj") 
                    |> Array.map (fun f -> MMObjFileModel(f))
                    |> Array.iter (fun nt -> mmobjFiles.Add(nt))
            x.TargetFileChanged()
            x.RaisePropertyChanged("Files") 
    member x.DataDir
        with get() = dataDir
        and set value = dataDir <- value
    member x.PreviewHost
        with set value = previewHost <- value

    member x.Files 
        with get() = mmobjFiles

    member x.SelectedFile
        with get() = 
            targetMMObjFile
        and set value = 
            targetMMObjFile <- value
            x.TargetFileChanged()

    member x.ModName
        with get() = modName
        and set (value:string) = 
            modName <- value.Trim()
            x.TargetFileChanged()

    member x.ModDest 
        with get() = 
            // TODO: would like to update this on each keystroke, but that seems to require 
            // hooking the selectionChanged event, which is a PITA from F# 
            match validateModName modName with
            | Err(s) -> s
            | Ok(path) -> path

    member x.BrowseFile = alwaysExecutable (fun action ->
        match ViewModelUtil.pushSelectFileDialog (Some(x.SnapshotDir),"MMObj files (*.mmobj)|*.mmobj") with
        | None -> ()
        | Some (file) -> 
            // try to find existing model in collection
            let found = mmobjFiles |> Seq.tryFind (fun m -> m.FullPath = file)
            let model = 
                match found with
                | None -> 
                    let model = new MMObjFileModel(file)
                    mmobjFiles.Add(model)
                    model
                | Some (model) -> model
            targetMMObjFile <- model
        x.TargetFileChanged())
            
    member x.TargetFileChanged() = 
        match previewHost with
        | None -> ()
        | Some(host) -> 
            host.SelectedFile <- targetMMObjFile.FullPath

        x.RaisePropertyChanged("CanCreate") 
        x.RaisePropertyChanged("Create") 
        x.RaisePropertyChanged("ModDest")
        x.RaisePropertyChanged("SelectedFile")

    member x.AddToModIndex 
        with get() = addToModIndex
        and set value = addToModIndex <- value

    member x.CanCreate = 
        let mnvalid = 
            match validateModName(modName) with 
            | Err(_) -> false 
            | Ok(_) -> true
        File.Exists(targetMMObjFile.FullPath) && mnvalid

    member x.Create =  
        new RelayCommand (
            (fun canExecute -> x.CanCreate), 
            (fun action ->
                match validateModName(modName) with
                | Err(e) -> ViewModelUtil.pushDialog(e)
                | Ok(file) ->
                    match (ModUtil.createMod dataDir modName targetMMObjFile.FullPath) with
                    | Ok(modFile) -> 
                        let createdMessage = sprintf "Import %s into blender to edit." modFile
                        let modIndexErr = 
                            if addToModIndex then
                                match (ModUtil.addToModIndex dataDir modFile) with
                                | Err(e) -> (sprintf "\n\nFailed to add mod to mod index, please add it manually: %s\n\n" e)
                                | Ok(_) -> ""
                            else
                                ""
                                
                        ViewModelUtil.pushDialog(sprintf "Mod created.  %s%s" modIndexErr createdMessage )
                    | Err(msg) -> ViewModelUtil.pushDialog(msg)                
                () ))
