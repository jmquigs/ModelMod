// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley
//
// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

namespace MMLaunch

open System
open System.Collections.ObjectModel
open System.Diagnostics
open System.IO

open Avalonia
open Avalonia.Controls
open Avalonia.Markup.Xaml
open Avalonia.Media.Imaging
open Avalonia.Threading

open ModelMod
open ModelMod.ConfigTypes

// Helper module for using parameterized loc strings with failwith, etc
module Formatters =
    type String = Printf.StringFormat<string -> unit>
    type StringRetString = Printf.StringFormat<string -> string>
    type StringAnyRetString<'a> = Printf.StringFormat<string -> 'a -> string>
    type AnyRetString<'a> = Printf.StringFormat<'a -> string>

module LocStrings =
    module Input =
        let Header = "Input:"
        let Desc1 = "Press CONTROL followed by the following keys."
        let Desc2 = "There is no in-game UI that displays these, so try alt-tab if you forget them."
        let Reload = "Load (or reload) modelmod managed code, configuration, and mods"
        let ReloadMods = "Load (or reload) mods only"
        let Toggle = "Toggle mod display"
        let ClearTex = "Clear the active texture list (will be rebuilt from scene textures)"
        let SelectNextTex = "Select Previous Texture"
        let SelectPrevTex = "Select Next Texture"
        let DoSnapshot = "Take snapshot of current selection"

    module Snapshot =
        let Header = "Snapshot Profile:"

    module Errors =
        let ProfileNameRequired = "Profile name may not be empty"
        let NoFilesInDir = Formatters.AnyRetString("No files in %s")
        let SnapshotDirNotFound = Formatters.StringRetString("Snapshot directory does not exist: %s")
        let LoaderStartFailed = Formatters.StringAnyRetString("Start Failed: %s (target: %s)")
        let LoaderUnknownExit = Formatters.AnyRetString("Unknown (code: %d)")
        let NoInputDescription = "No Input description available"
        let NoSnapshotDescription = "No Snapshot description available"
        let BadExePath = "Cannot set exe path; it is already used by another profile"

    module Misc =
        let NewProfileName = "New Profile"
        let PermanentRemove:Formatters.StringAnyRetString<string> = Formatters.StringAnyRetString("Permanently remove files (no recycle bin) in %s?\n\nFiles:\n%s")
        //let ConfirmRecycle = Formatters.StringAnyRetString("Remove files in %s?\n%A")
        let ConfirmProfileRemove = Formatters.AnyRetString("Remove profile '%s'?\n\nNote: mod & snapshot files that are associated with this profile will not be removed.")
        let RecycleMoar = Formatters.StringAnyRetString("%s\n...and %d more")
        let LoaderNotStarted = "Not Started"
        let LoaderStartPending = "Start Pending..."
        let LoaderStopped = Formatters.StringAnyRetString("Exited with status: %s (target: %s)")
        let LoaderStarted = Formatters.AnyRetString("Started; waiting for exit (target: %s)")
        let ExeFilesFilter = "Executable files (*.exe)"
        let StartCopy =
            Formatters.StringAnyRetString(
                "This version of ModelMod does not know how to start this game.  " +
                "Therefore ModelMod probably doesn't work with it.\n\nIf you want to try manually, " +
                "first figure out whether the game is 32 or 64 bit and whether it uses d3d9 or 11." +
                "\nThen you copy d3d9.dll or d3d11.dll into the game's executable directory, and then start the game manually." +
                "\nThe destination may be the game's directory or a subdirectory (like 'bin64')." +
                "\nIf the game is 32 bit, copy %s\\modelmod_32\\d3d9.dll. (or d3d11.dll)" +
                "\nIf the game is 64 bit, copy %s\\modelmod_64\\d3d9.dll. (or d3d11.dll)" +
                "\nIf you don't know where to copy the file, it is the same location Reshade would use." +
                "\nRemove d3d9.dll (or d3d11.dll) from the game's directory to stop using ModelMod.")

module ProfileText =
    module Input =
        let CommandOrder = [
            LocStrings.Input.ReloadMods; LocStrings.Input.Toggle
            LocStrings.Input.ClearTex
            LocStrings.Input.SelectNextTex; LocStrings.Input.SelectPrevTex; LocStrings.Input.DoSnapshot
            LocStrings.Input.Reload ]

        let PunctKeys = [@"\"; "]"; ";"; ","; "."; "/"; "-"]
        let FKeys = ["F1"; "F2"; "F6"; "F3"; "F4"; "F7"; "F10"]

        let Descriptions =
            let makeInputDesc keys =
                List.fold2 (fun acc key text -> acc + (sprintf "%s\t%s\n" key text)) "" keys CommandOrder

            Map.ofList [ (InputProfiles.PunctRock, makeInputDesc PunctKeys)
                         (InputProfiles.FItUp, makeInputDesc FKeys) ]

/// Loads an exe file's icon as an Avalonia Bitmap.  Falls back to null on
/// failure.  Uses System.Drawing's icon extraction; works on Windows only,
/// which is fine since the launcher only runs there.
module IconLoader =
    let extractFromExe (exePath: string) : Bitmap option =
        if String.IsNullOrEmpty exePath || not (File.Exists exePath) then None
        else
            try
                use icon = System.Drawing.Icon.ExtractAssociatedIcon(exePath)
                if isNull icon then None
                else
                    use bmp = icon.ToBitmap()
                    use ms = new MemoryStream()
                    bmp.Save(ms, System.Drawing.Imaging.ImageFormat.Png)
                    ms.Position <- 0L
                    Some(new Bitmap(ms))
            with _ -> None

/// Mutable wrapper around an immutable RunConfig.
[<AllowNullLiteral>] // ListBox bindings need a reference type that supports null
type ProfileModel(config: ConfigTypes.RunConfig) =
    inherit ViewModelBase()
    let mutable config = config

    // set defaults for empty profile values
    let mutable config =
        if config.SnapshotProfile.Trim() = "" then
            { config with SnapshotProfile = ConfigTypes.DefaultSnapProfileName }
        else config

    let mutable config =
        if config.InputProfile.Trim() = "" || not (InputProfiles.isValid config.InputProfile) then
            { config with InputProfile = InputProfiles.DefaultProfile }
        else config

    let save () =
        try RegConfig.saveProfile config
        with e -> ViewModelUtil.pushDialog (sprintf "%s" e.Message)

    let mutable iconSource: Bitmap option =
        IconLoader.extractFromExe config.ExePath

    member x.Config = config

    member x.Icon
        with get () =
            match iconSource with
            | Some b -> b :> Avalonia.Media.IImage
            | None -> null

    member x.ProfileKeyName = config.ProfileKeyName

    member x.Name
        with get () = config.ProfileName
        and set (value: string) =
            let value = value.Trim()
            if value = "" then ViewModelUtil.pushDialog LocStrings.Errors.ProfileNameRequired
            else
                config <- { config with ProfileName = value }
                save ()
                x.RaisePropertyChanged "Name"

    member x.ExePath
        with get () = config.ExePath
        and set value =
            config <- { config with ExePath = value }
            iconSource <- IconLoader.extractFromExe value
            save ()

    member x.InputProfile
        with get () = config.InputProfile
        and set value =
            config <- { config with InputProfile = value }
            save ()

    member x.SnapshotProfile
        with get () = config.SnapshotProfile
        and set value =
            config <- { config with SnapshotProfile = value }
            save ()

    member x.LaunchWindow
        with get () = config.LaunchWindow
        and set value =
            config <- { config with LaunchWindow = value }
            save ()

    member x.LoadModsOnStart
        with get () = config.LoadModsOnStart
        and set value =
            config <- { config with LoadModsOnStart = value }
            save ()

    member x.GameProfile
        with get () = config.GameProfile
        and set value =
            config <- { config with GameProfile = value }
            save ()

module MainViewUtil =
    let pushSelectExecutableDialog (currentExe: string option) =
        let initialDir =
            match currentExe with
            | None -> None
            | Some exe when File.Exists exe -> Some(Directory.GetParent(exe).ToString())
            | _ -> None

        ViewModelUtil.pushSelectFileDialog (initialDir, LocStrings.Misc.ExeFilesFilter + "|*.exe")

    let getDirLocator (profile: ProfileModel) =
        let root = ProcessUtil.getMMRoot ()
        State.DirLocator(root, profile.Config)

    let getSnapshotDir (profile: ProfileModel) =
        Path.GetFullPath(getDirLocator(profile).ExeSnapshotDir)

    let getDataDir (profile: ProfileModel) =
        Path.GetFullPath(getDirLocator(profile).ExeDataDir)

    let pushDeleteProfileDialog (profile: ProfileModel) =
        let msg = sprintf LocStrings.Misc.ConfirmProfileRemove profile.Name
        match ViewModelUtil.pushOkCancelDialog msg with
        | DialogResult.Yes -> true
        | _ -> false

    let private setOwner (child: Window) (parent: Window) =
        child.WindowStartupLocation <- WindowStartupLocation.CenterOwner

    let makeBlenderWindow (parentWin: Window) =
        let view = BlenderWindow()
        setOwner view parentWin
        view, (view.DataContext :?> BlenderViewModel)

    let makePreferencesWindow (parentWin: Window) =
        let view = PreferencesWindow()
        setOwner view parentWin
        view, (view.DataContext :?> PreferencesViewModel)

    let makeGameProfileWindow (parentWin: Window) =
        let view = GameProfileWindow()
        setOwner view parentWin
        view, (view.DataContext :?> GameProfileViewModel)

    let makeConfirmDialog (parentWin: Window) =
        let view = ConfirmDialog()
        setOwner view parentWin
        view, view.ViewModel

    let private showDialogSync (win: Window) (parent: Window) =
        let task = win.ShowDialog(parent)
        let frame = DispatcherFrame()
        task.ContinueWith(fun _ -> frame.Continue <- false) |> ignore
        Dispatcher.UIThread.PushFrame(frame)

    let pushRemoveSnapshotsDialog (mainWin: Window) (profile: ProfileModel) =
        let snapdir = getSnapshotDir profile

        if not (Directory.Exists snapdir) then
            ViewModelUtil.pushDialog (sprintf LocStrings.Errors.SnapshotDirNotFound snapdir)
        else
            let files = Directory.GetFiles snapdir

            if files.Length = 0 then
                ViewModelUtil.pushDialog (sprintf LocStrings.Errors.NoFilesInDir snapdir)
            else
                let display = 10
                let take = Math.Min(files.Length, display)
                let moar = files.Length - take
                let preview = files |> Seq.take take |> Array.ofSeq
                let preview = String.Join("\n", preview)

                let baseMsg = sprintf LocStrings.Misc.PermanentRemove snapdir preview
                let msg = if moar = 0 then baseMsg else sprintf LocStrings.Misc.RecycleMoar baseMsg moar

                let view, vm = makeConfirmDialog mainWin
                vm.Text <- msg
                vm.CheckBoxText <- "" // recycle bin not available on .NET 8 cross-platform; always permanent delete
                showDialogSync view mainWin

                if vm.Confirmed then
                    for f in files do
                        try File.Delete f with _ -> ()

    let makeCreateModDialog (parentWin: Window) (profile: ProfileModel) =
        let view = CreateModWindow()
        setOwner view parentWin

        let vm = view.ViewModel
        vm.SnapshotDir <- getSnapshotDir profile
        vm.DataDir <- getDataDir profile

        view, vm

    let failValidation (msg: string) = ViewModelUtil.pushDialog msg

    let profileDirHasFiles (p: ProfileModel) (dirSelector: ProfileModel -> string) =
        try
            if p.ExePath = "" then false
            else
                let sd = dirSelector p
                if not (Directory.Exists sd) then false
                else Directory.EnumerateFileSystemEntries(sd).GetEnumerator().MoveNext()
        with _ -> false

    let openModsDir (p: ProfileModel) =
        if profileDirHasFiles p getDataDir then
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- true
            proc.StartInfo.FileName <- getDataDir p
            proc.Start() |> ignore

    let showDialog (win: Window) (parent: Window) = showDialogSync win parent

/// Used for Snapshot and Input profiles, since they both basically just have a name
/// and description as far as the UI is concerned.
type SubProfileModel(name: string) =
    member x.Name = name

type LaunchWindowModel(name: string, time: int) =
    member x.Name = name
    member x.Time = time

type GameExePath = string

type LoaderState =
    | NotStarted
    | StartPending of GameExePath
    | StartFailed of Exception * GameExePath
    | Started of Process * GameExePath
    | Stopped of Process * GameExePath

type MainViewModel() as self =
    inherit ViewModelBase()

    let mutable selectedProfile: ProfileModel option = None
    let mutable loaderState = NotStarted

    do RegConfig.init ()

    let snapshotProfileDefs, snapshotProfileNames =
        try
            let defs = SnapshotProfileLoad.GetAll(ProcessUtil.getMMRoot ())
            let names = defs |> Map.toList |> List.map fst
            defs, names
        with _ -> Map.ofList [], []

    let observableProfiles =
        new ObservableCollection<ProfileModel>(
            RegConfig.loadAll ()
            |> Array.sortBy (fun gp -> gp.ProfileName.ToLowerInvariant().Trim())
            |> Array.map (fun rc -> ProfileModel(rc)))

    let timer = new DispatcherTimer()

    do
        timer.Interval <- TimeSpan(0, 0, 1)
        timer.Tick.Add(fun _ -> self.PeriodicUpdate())
        timer.Start()

    let launchWindows =
        [ LaunchWindowModel("5 Seconds", 5)
          LaunchWindowModel("15 Seconds", 15)
          LaunchWindowModel("30 Seconds", 30)
          LaunchWindowModel("45 Seconds", 45) ]

    let getSelectedProfileField (getter: ProfileModel -> 'a) (devVal: 'a) =
        match selectedProfile with
        | None -> devVal
        | Some p -> getter p

    let setSelectedProfileField (setter: ProfileModel -> unit) =
        match selectedProfile with
        | None -> ()
        | Some p -> setter p

    member x.PeriodicUpdate() =
        try
            let currentRoot = ProcessUtil.getMMRoot ()
            let regRoot = RegConfig.getMMRoot ()
            if currentRoot <> regRoot then RegConfig.setMMRoot currentRoot |> ignore
        with _ -> ()

        x.UpdateLoaderState
        <| match loaderState with
           | NotStarted
           | StartPending _
           | StartFailed _ -> loaderState
           | Stopped (_, _) -> loaderState
           | Started (proc, exe) ->
               if proc.HasExited then
                   try
                       File.Delete(Path.Combine(Path.GetDirectoryName exe, "ModelModCLRAppDomain.dll"))
                   with _ -> ()

                   Stopped(proc, exe)
               else loaderState

        x.UpdateProfileButtons()

    member x.LoaderStateText =
        match loaderState with
        | NotStarted -> LocStrings.Misc.LoaderNotStarted
        | StartPending _ -> LocStrings.Misc.LoaderStartPending
        | StartFailed (e, exe) -> sprintf LocStrings.Errors.LoaderStartFailed e.Message exe
        | Stopped (proc, exe) ->
            let exitReason =
                ProcessUtil.getLoaderExitReason proc (sprintf LocStrings.Errors.LoaderUnknownExit proc.ExitCode)
            sprintf LocStrings.Misc.LoaderStopped exitReason exe
        | Started (_, exe) -> sprintf LocStrings.Misc.LoaderStarted exe

    member x.Profiles = observableProfiles

    member x.SnapshotProfiles =
        new ObservableCollection<SubProfileModel>(snapshotProfileNames |> List.map SubProfileModel)

    member x.InputProfiles =
        new ObservableCollection<SubProfileModel>(InputProfiles.ValidProfiles |> List.map SubProfileModel)

    member x.LaunchWindows = new ObservableCollection<LaunchWindowModel>(launchWindows)

    /// Bound to the listbox.  ListBox can't bind to an option-typed property,
    /// so we expose a plain ProfileModel-or-null adapter and translate.
    member x.SelectedProfileRaw
        with get () : ProfileModel =
            match selectedProfile with
            | None -> null
            | Some p -> p
        and set (value: ProfileModel) =
            x.SelectedProfile <-
                if isNull value then None else Some value

    member x.SelectedProfile
        with get () = selectedProfile
        and set value =
            selectedProfile <- value

            x.RaisePropertyChanged "SelectedProfile"
            x.RaisePropertyChanged "SelectedProfileRaw"
            x.RaisePropertyChanged "SelectedProfileName"
            x.RaisePropertyChanged "SelectedProfileExePath"
            x.RaisePropertyChanged "SelectedProfileLoadModsOnStart"
            x.RaisePropertyChanged "SelectedProfileLaunchWindow"
            x.RaisePropertyChanged "SelectedInputProfile"
            x.RaisePropertyChanged "SelectedSnapshotProfile"
            x.RaisePropertyChanged "ProfileAreaVisible"
            x.RaisePropertyChanged "ProfileDescription"
            x.UpdateLaunchUI()
            x.UpdateProfileButtons()

    member x.SelectedProfileName
        with get () = getSelectedProfileField (fun p -> p.Name) ""
        and set (value: string) = setSelectedProfileField (fun p -> p.Name <- value)

    member x.SelectedProfileExePath
        with get () = getSelectedProfileField (fun p -> p.ExePath) ""
        and set (value: string) = setSelectedProfileField (fun p -> p.ExePath <- value)

    member x.SelectedProfileLoadModsOnStart
        with get () = getSelectedProfileField (fun p -> p.LoadModsOnStart) ConfigTypes.DefaultRunConfig.LoadModsOnStart
        and set (value: bool) = setSelectedProfileField (fun p -> p.LoadModsOnStart <- value)

    member x.SelectedProfileLaunchWindow
        with get () =
            let time = getSelectedProfileField (fun p -> p.LaunchWindow) ConfigTypes.DefaultRunConfig.LaunchWindow

            match launchWindows |> List.tryFind (fun lt -> lt.Time = time) with
            | None -> launchWindows.Head.Time
            | Some lw -> lw.Time
        and set (value: int) = setSelectedProfileField (fun p -> p.LaunchWindow <- value)

    member x.SelectedInputProfile
        with get () = getSelectedProfileField (fun p -> p.InputProfile) ConfigTypes.DefaultRunConfig.InputProfile
        and set (value: string) =
            setSelectedProfileField (fun p -> p.InputProfile <- value)
            x.RaisePropertyChanged "ProfileDescription"

    member x.SelectedSnapshotProfile
        with get () = getSelectedProfileField (fun p -> p.SnapshotProfile) ConfigTypes.DefaultRunConfig.SnapshotProfile
        and set (value: string) =
            setSelectedProfileField (fun p -> p.SnapshotProfile <- value)
            x.RaisePropertyChanged "ProfileDescription"

    member x.ProfileDescription =
        match x.SelectedProfile with
        | None -> ""
        | Some profile ->
            let inputText =
                match ProfileText.Input.Descriptions |> Map.tryFind profile.InputProfile with
                | None -> LocStrings.Errors.NoInputDescription
                | Some text ->
                    LocStrings.Input.Header + "\n" + LocStrings.Input.Desc1 + "\n" + LocStrings.Input.Desc2 + "\n" + text

            let snapshotText =
                let stext =
                    snapshotProfileDefs
                    |> Map.tryFind profile.SnapshotProfile
                    |> function
                        | None -> LocStrings.Errors.NoSnapshotDescription
                        | Some p -> p.ToString()

                LocStrings.Snapshot.Header + "\n" + stext

            inputText + "\n" + snapshotText

    member x.LauncherProfileIcon =
        match loaderState with
        | Started (_, exe)
        | StartPending exe ->
            match observableProfiles |> Seq.tryFind (fun p -> p.ExePath = exe) with
            | None -> null
            | Some p -> p.Icon
        | NotStarted
        | StartFailed _
        | Stopped _ ->
            match x.SelectedProfile with
            | None -> null
            | Some p -> p.Icon

    member x.ProfileAreaVisible = selectedProfile.IsSome

    member x.BrowseExe =
        ViewModelUtil.alwaysExecutable (fun mainWin ->
            selectedProfile
            |> Option.iter (fun selectedProfile ->
                match MainViewUtil.pushSelectExecutableDialog (Some selectedProfile.ExePath) with
                | None -> ()
                | Some exePath ->
                    let existingProfileKey = RegConfig.findProfilePath exePath

                    let ok =
                        match existingProfileKey with
                        | None -> true
                        | Some key -> key.EndsWith(selectedProfile.ProfileKeyName)

                    if ok then
                        selectedProfile.ExePath <- exePath

                        if selectedProfile.Name = ""
                           || selectedProfile.Name.StartsWith(LocStrings.Misc.NewProfileName) then
                            selectedProfile.Name <- RegConfig.getDefaultProfileName exePath

                        x.SelectedProfile <- x.SelectedProfile
                    else
                        MainViewUtil.failValidation LocStrings.Errors.BadExePath))

    member x.UpdateLoaderState(newState) =
        if newState <> loaderState then
            loaderState <- newState
            x.UpdateLaunchUI()

    member x.UpdateLaunchUI() =
        x.RaisePropertyChanged "LoaderStateText"
        x.RaisePropertyChanged "LoaderIsStartable"
        x.RaisePropertyChanged "StartInSnapshotMode"
        x.RaisePropertyChanged "StartInDebugMode"
        x.RaisePropertyChanged "LauncherProfileIcon"
        x.RaisePropertyChanged "ViewInjectionLog"
        x.RaisePropertyChanged "ViewModelModLog"

    member x.UpdateProfileButtons() =
        x.RaisePropertyChanged "DeleteProfile"
        x.RaisePropertyChanged "RemoveSnapshots"
        x.RaisePropertyChanged "CreateMod"
        x.RaisePropertyChanged "OpenMods"

    member x.HasModFiles =
        match x.SelectedProfile with
        | None -> false
        | Some profile -> MainViewUtil.profileDirHasFiles profile MainViewUtil.getDataDir

    member x.HasSnapshots =
        match x.SelectedProfile with
        | None -> false
        | Some profile -> MainViewUtil.profileDirHasFiles profile MainViewUtil.getSnapshotDir

    member x.LoaderIsStartable =
        match loaderState with
        | StartPending _
        | Started _ -> false
        | StartFailed _
        | Stopped _
        | NotStarted -> true

    member x.NewProfile =
        new RelayCommand(
            (fun _ -> true),
            (fun _ ->
                let seqNextName =
                    seq {
                        let rec next count =
                            let nextName =
                                if count = 0 then LocStrings.Misc.NewProfileName
                                else sprintf "%s (%d)" LocStrings.Misc.NewProfileName count

                            match observableProfiles |> Seq.tryFind (fun pm -> pm.Name = nextName) with
                            | None -> nextName
                            | Some _ -> next (count + 1)
                        yield next 0
                    }

                let profile =
                    ProfileModel({ RegConfig.loadDefaultProfile () with ProfileName = Seq.head seqNextName })

                observableProfiles.Add profile

                x.RaisePropertyChanged "Profiles"
                x.SelectedProfile <- Some profile))

    member x.DeleteProfile =
        new RelayCommand(
            (fun _ -> selectedProfile.IsSome),
            (fun _ ->
                x.SelectedProfile
                |> Option.iter (fun profile ->
                    if MainViewUtil.pushDeleteProfileDialog profile then
                        try
                            if profile.Config.ProfileKeyName <> "" then
                                RegConfig.removeProfile profile.Config

                            if observableProfiles.Count <= 1 then
                                x.SelectedProfile <- None
                                observableProfiles.Clear()
                            else
                                x.SelectedProfile <- Some(observableProfiles.Item(observableProfiles.Count - 1))
                                x.UpdateLaunchUI()
                                x.UpdateProfileButtons()

                                observableProfiles.Remove profile |> ignore
                        with e -> ViewModelUtil.pushDialog e.Message)))

    member x.DoRemoveSnapshots(mainWin: Window) =
        x.SelectedProfile
        |> Option.iter (fun profile -> MainViewUtil.pushRemoveSnapshotsDialog mainWin profile)

    member x.RemoveSnapshots =
        new RelayCommand(
            (fun _ -> x.HasSnapshots),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | Some w -> x.DoRemoveSnapshots w
                | None -> ()))

    member x.CreateMod =
        new RelayCommand(
            (fun _ -> x.HasSnapshots),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    x.SelectedProfile
                    |> Option.iter (fun profile ->
                        let view, vm = MainViewUtil.makeCreateModDialog w profile
                        vm.RemoveSnapshotsFn <- (fun _ -> x.DoRemoveSnapshots w)
                        MainViewUtil.showDialog view w)))

    member x.OpenMods =
        new RelayCommand(
            (fun _ -> x.HasModFiles),
            (fun _ -> x.SelectedProfile |> Option.iter MainViewUtil.openModsDir))

    member x.SetupBlender =
        new RelayCommand(
            (fun _ -> true),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    let view, _ = MainViewUtil.makeBlenderWindow w
                    MainViewUtil.showDialog view w))

    member x.OpenPreferences =
        new RelayCommand(
            (fun _ -> true),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    let view, _ = MainViewUtil.makePreferencesWindow w
                    MainViewUtil.showDialog view w))

    member x.OpenGameProfile =
        new RelayCommand(
            (fun _ -> true),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    let view, vm = MainViewUtil.makeGameProfileWindow w

                    if x.SelectedProfile.IsSome then
                        vm.Profile <- x.SelectedProfile.Value.GameProfile

                    vm.ProfileChangedCb <-
                        (fun gameProfile ->
                            x.SelectedProfile
                            |> Option.iter (fun profile -> profile.GameProfile <- gameProfile))

                    MainViewUtil.showDialog view w))

    member private x.promptCopy(mainWin: Window, debugMode: bool, selectedProfile: ProfileModel) =
        let debugText = """# DebugMode file created by MMLaunch on $DATE for $GAME

# This file will be overwritten by MMLaunch whenever "Start(Debug)" is clicked.
# It will be removed by MMLaunch whenever "Start" is clicked.
# if you want to preserve this file make it read-only.

# When this file exists, ModelMod will start in "DebugMode" which slows
# its initialization and reports extra info in the log file,
# to help catch errors.  Please include the log file in any bugs
# you report, especially if it relates to a crash or hang.
# The following settings are available and can be set to
# zero or one.  When all these are
# zero it is equivalent to running without DebugMode, but still runs
# somewhat more slowly.  For bug reports please ensure you
# have these all set to 1.

protect_mem=1
defer_rehook=1
defer_draw_hook=1
add_ref_context=1
add_ref_device=1
"""

        let res = ProcessUtil.preStartCopy selectedProfile.ExePath

        let showMessage (msg: string) =
            let view, vm = MainViewUtil.makeConfirmDialog mainWin
            vm.CheckBoxText <- ""
            vm.Text <- msg
            MainViewUtil.showDialog view mainWin

        match res with
        | Ok ProcessUtil.PreStartCopyResult.Copied ->
            let root = ProcessUtil.getMMRoot ()
            let dmFile = Path.Combine(root, "DebugMode.txt")

            try
                let debugText =
                    debugText
                        .Replace(("$DATE": string), DateTime.Now.ToString())
                        .Replace("$GAME", selectedProfile.ExePath)

                if debugMode then File.WriteAllText(dmFile, debugText)
                elif File.Exists dmFile then File.Delete dmFile
            with _ -> ()

            match ProcessUtil.launch selectedProfile.ExePath with
            | Ok _ -> ()
            | Err e -> showMessage ("Start failed: " + e.Message)
        | Ok ProcessUtil.PreStartCopyResult.UnknownExe ->
            let binPath = Path.Combine(ProcessUtil.getMMRoot (), "Bin")
            showMessage (sprintf LocStrings.Misc.StartCopy binPath binPath)
        | Err e -> showMessage ("Start failed: " + e.Message)

    member x.StartInSnapshotMode =
        new RelayCommand(
            (fun _ -> x.ProfileAreaVisible && x.LoaderIsStartable),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    x.SelectedProfile
                    |> Option.iter (fun prof -> x.promptCopy(w, false, prof))))

    member x.StartInDebugMode =
        new RelayCommand(
            (fun _ -> x.ProfileAreaVisible && x.LoaderIsStartable),
            (fun mainWin ->
                match ViewModelUtil.asWindow mainWin with
                | None -> ()
                | Some w ->
                    x.SelectedProfile
                    |> Option.iter (fun prof -> x.promptCopy(w, true, prof))))

    member x.ViewInjectionLog =
        new RelayCommand(
            (fun _ -> x.ProfileAreaVisible && x.LoaderIsStartable),
            (fun _ ->
                x.SelectedProfile
                |> Option.iter (fun profile ->
                    match ProcessUtil.openInjectionLog profile.ExePath with
                    | Ok _ -> ()
                    | Err e -> MainViewUtil.failValidation e.Message)))

    member x.ViewModelModLog =
        new RelayCommand(
            (fun _ -> x.ProfileAreaVisible),
            (fun _ ->
                x.SelectedProfile
                |> Option.iter (fun profile ->
                    match ProcessUtil.openModelModLog profile.ExePath with
                    | Ok _ -> ()
                    | Err e -> MainViewUtil.failValidation e.Message)))

type MainWindow() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        this.DataContext <- MainViewModel()
