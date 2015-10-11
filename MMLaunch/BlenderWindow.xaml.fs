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
    let mutable scriptStatus = ""

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

    let getScriptStatus (scriptdir:string option) = 
        let lastScriptDir = defaultArg scriptdir (RegConfig.getGlobalValue RegKeys.LastScriptInstallDir "" :?> string)

        match lastScriptDir with
        | "" -> "Unknown"
        | s when not (Directory.Exists(s)) -> "Unknown"
        | s ->
            match BlenderUtil.checkScriptStatus s with
            | Err(s) -> sprintf "Failed to check status: %s" s
            | Ok(BlenderUtil.NotFound) -> "Not installed"
            | Ok(BlenderUtil.UpToDate) -> "Up to date"
            | Ok(BlenderUtil.Diverged) -> "Out of date or modified locally"

    do
        detectedBlender <- detectBlender()
        let lastBlender = RegConfig.getGlobalValue RegKeys.LastSelectedBlender "" :?> string
        selectedBlender <- 
            match lastBlender with
            | "" -> detectedBlender
            | s when (not (File.Exists s)) -> detectedBlender
            | s -> Some(s)
        scriptStatus <- getScriptStatus None

    member x.SelectedBlender 
        with get() = 
            match selectedBlender with
            | None -> "Not found"
            | Some s -> s
        and set (value:string) = 
            if File.Exists value then
                RegConfig.setGlobalValue RegKeys.LastSelectedBlender value |> ignore

            x.RaisePropertyChanged("SelectedBlender")

    member x.ScriptStatus 
        with get() = 
            scriptStatus
        and set value = 
            scriptStatus <- value
            x.RaisePropertyChanged("ScriptStatus")

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
    member x.Check = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> 
                match selectedBlender with
                | None -> ViewModelUtil.pushDialog "Please select a version of blender to use first."
                | Some exe ->
                    match (BlenderUtil.getAddonsPath exe) with
                    | Err(e) -> ViewModelUtil.pushDialog (sprintf "Failed to get addons path: %s" e)
                    | Ok(path) ->
                        let path = Path.Combine(path,BlenderUtil.ModName)
                        if Directory.Exists path then
                            RegConfig.setGlobalValue RegKeys.LastScriptInstallDir path |> ignore
                        x.ScriptStatus <- getScriptStatus (Some(path))
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
                        | Ok(dir) -> 
                            RegConfig.setGlobalValue RegKeys.LastScriptInstallDir dir |> ignore
                            ViewModelUtil.pushDialog (sprintf "Blender scripts installed and registered in'%s'" dir)
                            x.ScriptStatus <- getScriptStatus None

                        | Err(s) -> ViewModelUtil.pushDialog s
                    | _ -> ()
        ))