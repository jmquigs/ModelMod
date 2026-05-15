namespace MMLaunch

open System
open System.IO

open Avalonia.Controls
open Avalonia.Markup.Xaml

open ModelMod

type BlenderViewModel() as x =
    inherit ViewModelBase()

    let mutable detectedBlender = None
    let mutable selectedBlender = None
    let mutable scriptStatus = ""

    let detectBlender () =
        match BlenderUtil.detectInstallPath () with
        | None -> None
        | Some path ->
            let exe = Path.Combine(path, BlenderUtil.BlenderExe)
            if File.Exists exe then Some exe else None

    let getScriptStatus (scriptdir: string option) =
        let lastScriptDir =
            defaultArg scriptdir (RegConfig.getGlobalValue RegKeys.LastScriptInstallDir "" :?> string)

        match lastScriptDir with
        | "" -> "Unknown: likely not installed, or was installed manually."
        | s when not (Directory.Exists s) -> "Unknown"
        | s ->
            match BlenderUtil.checkScriptStatus s with
            | Err msg -> sprintf "Failed to check status: %s" msg
            | Ok BlenderUtil.NotFound -> "Not installed"
            | Ok BlenderUtil.UpToDate -> "Up to date"
            | Ok BlenderUtil.Diverged -> "Out of date or modified locally"

    do
        detectedBlender <- detectBlender ()
        let lastBlender = RegConfig.getGlobalValue RegKeys.LastSelectedBlender "" :?> string

        selectedBlender <-
            match lastBlender with
            | "" -> detectedBlender
            | s when not (File.Exists s) -> detectedBlender
            | s -> Some s

        scriptStatus <- getScriptStatus None

    member _.SelectedBlender
        with get () =
            match selectedBlender with
            | None -> "Not found"
            | Some s -> s
        and set (value: string) =
            if File.Exists value then
                RegConfig.setGlobalValue RegKeys.LastSelectedBlender value |> ignore
                selectedBlender <- Some value
            x.RaisePropertyChanged "SelectedBlender"

    member _.ScriptStatus
        with get () = scriptStatus
        and set value =
            scriptStatus <- value
            x.RaisePropertyChanged "ScriptStatus"

    member _.Detect =
        new RelayCommand((fun _ -> true), (fun _ ->
            match detectBlender () with
            | None -> ViewModelUtil.pushDialog "Blender not found; try using Browse to find it manually"
            | Some s ->
                match ViewModelUtil.pushOkCancelDialog (sprintf "Found blender:\n%s\nUse this?" s) with
                | DialogResult.Yes -> x.SelectedBlender <- s
                | _ -> ()))

    member _.Browse =
        new RelayCommand((fun _ -> true), (fun _ ->
            let idir =
                match selectedBlender with
                | None -> Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles)
                | Some exe -> Path.GetDirectoryName exe

            match ViewModelUtil.pushSelectFileDialog (Some idir, "Executable files|*.exe") with
            | None -> ()
            | Some exe ->
                if File.Exists exe then x.SelectedBlender <- exe))

    member _.Check =
        new RelayCommand((fun _ -> true), (fun _ ->
            match selectedBlender with
            | None -> ViewModelUtil.pushDialog "Please select a version of blender to use first."
            | Some exe ->
                match BlenderUtil.getAddonsPath exe with
                | Err e -> ViewModelUtil.pushDialog (sprintf "Failed to get addons path: %s" e)
                | Ok path ->
                    let path = Path.Combine(path, BlenderUtil.ModName)
                    if Directory.Exists path then
                        RegConfig.setGlobalValue RegKeys.LastScriptInstallDir path |> ignore
                    x.ScriptStatus <- getScriptStatus (Some path)))

    member _.Install =
        new RelayCommand((fun _ -> true), (fun _ ->
            match selectedBlender with
            | None -> ViewModelUtil.pushDialog "Please select a version of blender to use first."
            | Some exe ->
                match ViewModelUtil.pushOkCancelDialog
                    "This will install or update the MMObj blender scripts.  If you have modified the files locally, your changes will be overwritten.  Proceed?" with
                | DialogResult.Yes ->
                    match BlenderUtil.installMMScripts exe with
                    | Ok dir ->
                        RegConfig.setGlobalValue RegKeys.LastScriptInstallDir dir |> ignore
                        ViewModelUtil.pushDialog (sprintf "Blender scripts installed and registered in '%s'" dir)
                        x.ScriptStatus <- getScriptStatus None
                    | Err s -> ViewModelUtil.pushDialog s
                | _ -> ()))

type BlenderWindow() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        this.DataContext <- BlenderViewModel()
