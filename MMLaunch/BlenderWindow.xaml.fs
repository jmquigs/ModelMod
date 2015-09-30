namespace MMLaunch

open System
open System.Diagnostics
open System.Threading
open System.Windows
open System.Windows.Threading
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

type BlenderView = XAML<"BlenderWindow.xaml", true>

type BlenderViewModel() = 
    inherit ViewModelBase()

    let mutable detectedBlender = None
    let mutable selectedBlender = None

    let detectBlender() = 
        let path = BlenderUtil.detectInstallPath()
        match path with
        | None -> None
        | Some path -> 
            let exe = Path.Combine(path,BlenderUtil.BlenderExe)
            if File.Exists exe then
                Some(exe)
            else
                None

    do
        detectedBlender <- detectBlender()
        let lastBlender = RegConfig.getGlobalValue RegKeys.LastSelectedBlender "" :?> string
        selectedBlender <- 
            match lastBlender with
            | "" -> detectedBlender
            | s when (not (File.Exists s)) -> detectedBlender
            | s -> Some(s)

    member x.SelectedBlender 
        with get() = 
            match selectedBlender with
            | None -> "Not found"
            | Some s -> s
        and set (value:string) = 
            if File.Exists value then
                RegConfig.setGlobalValue RegKeys.LastSelectedBlender value |> ignore

            selectedBlender <- Some(value)
            x.RaisePropertyChanged("SelectedBlender")

    member x.Detect = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action ->
                match detectBlender() with
                | None -> ViewModelUtil.pushDialog "Blender not found; try using Browse to find it manually"
                | Some s -> 
                    match ViewModelUtil.pushOkCancelDialog (sprintf "Found blender:\n%s\nUse this?" s) with
                    | MessageBoxResult.Yes -> x.SelectedBlender <- s
                    | _ -> ()
            ))
    member x.Browse = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> 
                let idir = 
                    match selectedBlender with 
                    | None -> Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles)
                    | Some exe -> Path.GetDirectoryName(exe)
                match ViewModelUtil.pushSelectFileDialog (Some(idir),"Executable files (*.exe)|*.exe") with 
                | None -> ()
                | Some exe ->
                    if (File.Exists (exe)) then
                        x.SelectedBlender <- exe
            ))
    member x.Install = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> 
                match selectedBlender with
                | None -> ViewModelUtil.pushDialog "Please select a version of blender to use first."
                | Some exe ->                    
                    match ViewModelUtil.pushOkCancelDialog ("This will install or update the MMObj blender scripts.  If you have modified the files locally, your changes will be overwritten.  Proceed?") with
                    | MessageBoxResult.Yes ->
                        match BlenderUtil.installMMScripts(exe) with
                        | Ok(dir) -> ViewModelUtil.pushDialog (sprintf "Blender scripts installed and registered in'%s'" dir)
                        | Err(s) -> ViewModelUtil.pushDialog s
                    | _ -> ()
                | _ -> ()
        ))