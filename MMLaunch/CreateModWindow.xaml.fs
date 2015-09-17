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

type CreateModViewModel() = 
    inherit ViewModelBase()

    let mutable snapDir = ""
    let mutable dataDir = ""
    let mutable targetFile = ""
    let mutable modName = ""
    let mutable previewHost: PreviewHost option = None
    
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
        and set value = snapDir <- value
    member x.DataDir
        with get() = dataDir
        and set value = dataDir <- value
    member x.PreviewHost
        with set value = previewHost <- value

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
            targetFile <- file // todo: show preview window
        x.TargetFileChanged())

    member x.TargetFileChanged() = 
        match previewHost with
        | None -> ()
        | Some(host) -> host.SelectedFile <- targetFile

        x.RaisePropertyChanged("CanCreate") 
        x.RaisePropertyChanged("Create") 
        x.RaisePropertyChanged("ModDest")

    member x.CanCreate = 
        let mnvalid = 
            match validateModName(modName) with 
            | Err(_) -> false 
            | Ok(_) -> true
        File.Exists(targetFile) && mnvalid

    member x.Create =  
        new RelayCommand (
            (fun canExecute -> x.CanCreate), 
            (fun action ->
                match validateModName(modName) with
                | Err(e) -> ViewModelUtil.pushDialog(e)
                | Ok(file) ->
                    ViewModelUtil.pushDialog("creating with: " + file)
                
                () ))
