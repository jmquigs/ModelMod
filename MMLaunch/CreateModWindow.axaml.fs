namespace MMLaunch

open System
open System.Collections.ObjectModel
open System.IO

open Avalonia.Controls
open Avalonia.Markup.Xaml
open Avalonia.Threading

open ModelMod

[<AllowNullLiteral>]
type MMObjFileModel(fullPath: string) =
    member x.Name = Path.GetFileName(fullPath)
    member x.FullPath = fullPath

type CreateModViewModel() as self =
    inherit ViewModelBase()

    let mutable snapDir = ""
    let mutable dataDir = ""
    let mutable targetMMObjFile: MMObjFileModel option = None
    let mutable modName = ""
    let mutable previewHost: MeshPreviewControl option = None
    let mutable mmobjFiles = new ObservableCollection<MMObjFileModel>()
    let mutable addToModIndex = true
    let mutable convertTextures = true
    let mutable removeSnapshotsFn = ignore

    let mutable sdWriteTime = DateTime.Now

    let timer = new DispatcherTimer()

    do
        timer.Interval <- TimeSpan(0, 0, 1)
        timer.Tick.Add(fun _ ->
            let sd = self.SnapshotDir
            if Directory.Exists sd then
                let writeTime = Directory.GetLastWriteTime sd
                if writeTime <> sdWriteTime then
                    sdWriteTime <- writeTime
                    self.UpdateFileList())
        timer.Start()

    let validateModName (mn: string) : Result<string, string> =
        let illegalChars = [| '/'; '\\'; ':'; '*'; '?'; '"'; '<'; '>'; '|' |]
        let mn = mn.Trim()

        match mn with
        | "" -> Err "Enter mod name"
        | s when s.IndexOfAny illegalChars >= 0 ->
            Err(sprintf "Mod name cannot contain any of: %A" illegalChars)
        | s when s.Contains ".." -> Err "Mod name cannot contain .."
        | s when Directory.Exists(Path.Combine(dataDir, mn)) ->
            Err "Directory already exists, please choose a different mod name"
        | _ -> Ok(Path.Combine(dataDir, mn))

    member x.UpdateFileList() =
        mmobjFiles.Clear()

        if Directory.Exists snapDir then
            Directory.GetFiles(snapDir, "*.mmobj")
            |> Array.sortBy (fun f -> File.GetLastWriteTime(Path.Combine(snapDir, f)))
            |> Array.rev
            |> Array.map (fun f -> MMObjFileModel(f))
            |> Array.iter (fun m -> mmobjFiles.Add m)

            sdWriteTime <- Directory.GetLastWriteTime snapDir

        x.TargetFileChanged()
        x.RaisePropertyChanged "Files"
        x.RaisePropertyChanged "RemoveSnapshots"

    member x.SnapshotDir
        with get () = snapDir
        and set value =
            snapDir <- value
            x.UpdateFileList()

    member x.DataDir
        with get () = dataDir
        and set value = dataDir <- value

    member x.PreviewHost
        with set value = previewHost <- value

    member x.Files = mmobjFiles

    member x.SelectedFile
        with get () : MMObjFileModel =
            match targetMMObjFile with
            | None -> null
            | Some f -> f
        and set (value: MMObjFileModel) =
            targetMMObjFile <- if isNull value then None else Some value
            x.TargetFileChanged()

    member x.ModName
        with get () = modName
        and set (value: string) =
            modName <- value.Trim()
            x.TargetFileChanged()

    member x.ModDest =
        match validateModName modName with
        | Err s -> s
        | Ok path -> path

    member x.AddToModIndex
        with get () = addToModIndex
        and set value = addToModIndex <- value

    member x.ConvertTextures
        with get () = convertTextures
        and set value = convertTextures <- value

    member x.CanCreate =
        let mnvalid =
            match validateModName modName with
            | Err _ -> false
            | Ok _ -> true

        match targetMMObjFile with
        | None -> false
        | Some f -> File.Exists f.FullPath && mnvalid

    member x.RemoveSnapshotsFn
        with set value = removeSnapshotsFn <- value

    member x.BrowseFile =
        ViewModelUtil.alwaysExecutable (fun _ ->
            match ViewModelUtil.pushSelectFileDialog (Some snapDir, "MMObj files|*.mmobj") with
            | None -> ()
            | Some file ->
                let found = mmobjFiles |> Seq.tryFind (fun m -> m.FullPath = file)

                let model =
                    match found with
                    | Some m -> m
                    | None ->
                        let m = MMObjFileModel(file)
                        mmobjFiles.Add m
                        m

                targetMMObjFile <- Some model
                x.TargetFileChanged())

    member x.TargetFileChanged() =
        match previewHost with
        | None -> ()
        | Some host ->
            match targetMMObjFile with
            | None -> host.SelectedFile <- ""
            | Some file ->
                host.SelectedFile <- file.FullPath

        x.RaisePropertyChanged "CanCreate"
        x.RaisePropertyChanged "Create"
        x.RaisePropertyChanged "ModDest"
        x.RaisePropertyChanged "SelectedFile"

    member x.RemoveSnapshots =
        new RelayCommand((fun _ -> true), (fun _ -> removeSnapshotsFn ()))

    member x.Create =
        new RelayCommand(
            (fun _ -> x.CanCreate),
            (fun _ ->
                match validateModName modName, targetMMObjFile with
                | Err e, _ -> ViewModelUtil.pushDialog e
                | _, None -> ()
                | Ok _, Some file ->
                    match ModUtil.createMod dataDir modName convertTextures file.FullPath with
                    | Ok modFile ->
                        let createdMessage = sprintf "Import %s into blender to edit." modFile

                        let modIndexErr =
                            if addToModIndex then
                                match ModUtil.addToModIndex dataDir modFile with
                                | Err e -> sprintf "\n\nFailed to add mod to mod index, please add it manually: %s\n\n" e
                                | Ok _ -> ""
                            else ""

                        ViewModelUtil.pushDialog (sprintf "Mod created.  %s%s" modIndexErr createdMessage)
                    | Err msg -> ViewModelUtil.pushDialog msg))

type CreateModWindow() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        let vm = CreateModViewModel()
        this.DataContext <- vm

        // Wire the preview control found in XAML to the view-model.
        let preview = this.FindControl<MeshPreviewControl>("ModelPreview")
        if not (isNull preview) then
            vm.PreviewHost <- Some preview

    member x.ViewModel = x.DataContext :?> CreateModViewModel
