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

    member x.SnapshotDir 
        with get() = snapDir
        and set value = snapDir <- value
    member x.DataDir
        with get() = dataDir
        and set value = dataDir <- value

    member x.BrowseFile = alwaysExecutable (fun action ->
        match ViewModelUtil.pushSelectFileDialog (Some(x.SnapshotDir),"MMObj files (*.mmobj)|*.mmobj") with
        | None -> ()
        | Some (file) -> 
            targetFile <- file // todo: show preview window
        x.TargetFileChanged())

    member x.TargetFileChanged() = 
        x.RaisePropertyChanged("CanCreate") 
        x.RaisePropertyChanged("Create") 

    member x.CanCreate = File.Exists(targetFile)

    member x.Create =  
        new RelayCommand (
            (fun canExecute -> x.CanCreate), 
            (fun action ->
                // dooo it
                () ))
