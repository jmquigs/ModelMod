// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

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
open System.Windows.Controls
open System.ComponentModel
open System.Collections.ObjectModel
open Microsoft.Win32

open FsXaml

open ViewModelUtil
open ModelMod

type CreateModView = XAML<"CreateModWindow.xaml", true>

[<AllowNullLiteral>] // For listbox selecteditem compatibility
type MMObjFileModel(fullPath) =
    member x.Name 
        with get() = 
            Path.GetFileName(fullPath)
    member x.FullPath 
        with get() = fullPath

type CreateModViewModel() as self = 
    inherit ViewModelBase()

    let mutable snapDir = ""
    let mutable dataDir = ""
    let mutable targetMMObjFile:MMObjFileModel option = None
    let mutable modName = ""
    let mutable previewHost: PreviewHost option = None
    let mutable modNameTB: TextBox option = None
    let mutable mmobjFiles = new ObservableCollection<MMObjFileModel>([])
    let mutable addToModIndex = true
    let mutable removeSnapshotsFn = ignore

    let mutable sdWriteTime = DateTime.Now

    let timer = new DispatcherTimer()
    do 
        timer.Interval <- new TimeSpan(0,0,1)
        timer.Tick.Add(fun (args) -> 
            let sd = self.SnapshotDir
            if Directory.Exists(sd) then
                let writeTime = Directory.GetLastWriteTime(sd)
                if writeTime <> sdWriteTime then
                    ()
                    sdWriteTime <- writeTime
                    self.UpdateFileList()
        )
        timer.Start()
            
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

    member x.UpdateFileList() = 
        mmobjFiles.Clear()
        if Directory.Exists snapDir then
            Directory.GetFiles (snapDir, "*.mmobj") 
                |> Array.sortBy (fun f -> File.GetLastWriteTime(Path.Combine(snapDir,f) ))
                |> Array.rev
                |> Array.map (fun f -> MMObjFileModel(f))
                |> Array.iter (fun nt -> mmobjFiles.Add(nt))
            sdWriteTime <- Directory.GetLastWriteTime(snapDir)

        x.TargetFileChanged()
        x.RaisePropertyChanged("Files")
        x.RaisePropertyChanged("RemoveSnapshots")

    member x.SnapshotDir 
        with get() = snapDir
        and set value = 
            snapDir <- value
            x.UpdateFileList()
    member x.DataDir
        with get() = dataDir
        and set value = dataDir <- value
    member x.PreviewHost
        with set value = previewHost <- value
    member x.ModNameTB 
        with set value = 
            modNameTB <- value
            modNameTB |> Option.iter (fun tb -> tb.SelectionChanged.Add(x.ModNameChanged) )

    member x.Files 
        with get() = mmobjFiles

    member x.SelectedFile
        with get() = 
            match targetMMObjFile with
            | None -> null
            | Some(file) -> file
        and set value = 
            if value = null then
                targetMMObjFile <- None
            else
                targetMMObjFile <- Some(value)
            x.TargetFileChanged()

    member x.ModNameChanged args = 
        modNameTB |> Option.iter (fun tb ->
            modName <- tb.Text.Trim()
            x.TargetFileChanged()
        )

    member x.ModName
        with get() = modName
        and set (value:string) = 
            modName <- value.Trim()
            x.TargetFileChanged()

    member x.ModDest 
        with get() = 
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
            targetMMObjFile <- Some(model)
        x.TargetFileChanged())
            
    member x.TargetFileChanged() = 
        match previewHost with
        | None -> ()
        | Some(host) -> 
            match targetMMObjFile with
            | None -> host.SelectedFile <- ""
            | Some file -> 
                host.SelectedFile <- file.FullPath

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
        match targetMMObjFile with
            | None -> false
            | Some (file) -> File.Exists(file.FullPath) && mnvalid

    member x.RemoveSnapshotsFn
        with set value = removeSnapshotsFn <- value

    member x.RemoveSnapshots =
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> removeSnapshotsFn() ))

    member x.Create =
        new RelayCommand (
            (fun canExecute -> x.CanCreate), 
            (fun action ->
                match validateModName(modName),targetMMObjFile with
                | Err(e),_ -> ViewModelUtil.pushDialog(e)
                | _,None -> ()
                | Ok(_),Some(file) ->
                    match (ModUtil.createMod dataDir modName file.FullPath) with
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
            ))
