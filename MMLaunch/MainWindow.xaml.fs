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

module LocStrings = 
    module Input =
        let Header = "Input:"
        let Desc1 = "Press CONTROL followed by the following keys."
        let Desc2 = "There is no in-game UI that displays these, so try alt-tab if you forget them."
        let Reload = "Load (or reload) modelmod managed code, configuration, and mods"
        let Toggle = "Toggle mod display"
        let ClearTex = "Clear the active texture list (will be rebuilt from scene textures)"
        let SelectNextTex = "Select Previous Texture"
        let SelectPrevTex = "Select Next Texture"
        let DoSnapshot = "Take snapshot of current selection"

    module Snapshot = 
        let Header = "Snapshot Transforms:"
        let Desc1 = "The following transforms will be applied"
        let PosLabel = "Position: "
        let UVLabel = "UV: "

module ProfileText = 
    module Input = 
        let CommandOrder = [
            LocStrings.Input.Reload; LocStrings.Input.Toggle;
            LocStrings.Input.ClearTex; 
            LocStrings.Input.SelectNextTex; LocStrings.Input.SelectPrevTex; LocStrings.Input.DoSnapshot]
        let PunctKeys = [@"\"; "]"; 
            ";"; 
            ","; "."; "/"]
        let FKeys = ["F1"; "F2";
            "F7";
            "F3"; "F4"; "F6"]
    
        let Descriptions =
            let makeInputDesc keys = 
                List.fold2 (fun acc key text -> 
                    acc + (sprintf "%s\t%s\n" key text)
                ) "" keys CommandOrder

            Map.ofList [ (InputProfiles.PunctRock, (makeInputDesc PunctKeys)); 
                (InputProfiles.FItUp, (makeInputDesc FKeys)); ]

module MainViewUtil = 
    let popSelectExecutableDialog(currentExe:string option) = 
        let dlg = new OpenFileDialog()

        match currentExe with
        | None -> ()
        | Some exe ->
            if File.Exists(exe) then
                dlg.InitialDirectory <- Directory.GetParent(exe).ToString()

        dlg.Filter <- "Executable files (*.exe)|*.exe"
        dlg.FilterIndex <- 0
        dlg.RestoreDirectory <- true

        let res = dlg.ShowDialog() 
        if res.HasValue && res.Value then
            Some (dlg.FileName)
        else
            None

    let failValidation (msg:string) = 
        MessageBox.Show(msg) |> ignore

type MainView = XAML<"MainWindow.xaml", true>

// Mutable wrapper around an immutable RunConfig; there are ways we could use RunConfig
// directly, but we can also use this to store things that the run config won't 
// have, like logs and lists of mods.
type ProfileModel(config:CoreTypes.RunConfig) = 
    let mutable config = config

    // set defaults for empty profile values
    let mutable config = 
        if config.SnapshotProfile.Trim() = "" || not (SnapshotProfiles.isValid config.SnapshotProfile)
            then { config with SnapshotProfile = SnapshotProfiles.DefaultProfile} else config
    let mutable config = 
        if config.InputProfile.Trim() = "" || not (InputProfiles.isValid config.InputProfile)
            then { config with InputProfile = InputProfiles.DefaultProfile} else config

    let save() = RegConfig.saveProfile config

    let mutable iconSource = null

    do
        if File.Exists (config.ExePath) then
            use icon = System.Drawing.Icon.ExtractAssociatedIcon(config.ExePath)
            iconSource <-
                System.Windows.Interop.Imaging.CreateBitmapSourceFromHIcon(
                    icon.Handle,
                    System.Windows.Int32Rect.Empty,
                    System.Windows.Media.Imaging.BitmapSizeOptions.FromEmptyOptions())

    member x.Icon 
        with get() = iconSource

    member x.ProfileKeyName 
        with get() = config.ProfileKeyName

    member x.Name 
        with get() = config.ProfileName
        and set (value:string) = 
            let value = value.Trim()
            if value = "" then
                MainViewUtil.failValidation "Profile name may not be empty"
            else
                config <- {config with ProfileName = value } 
                save()

    member x.ExePath 
        with get() = config.ExePath
        and set value = 
            config <- { config with ExePath = value }
            save()

    member x.InputProfile 
        with get() = config.InputProfile
        and set value = 
            config <- { config with InputProfile = value }
            save()

    member x.SnapshotProfile
        with get() = config.SnapshotProfile
        and set value = 
            config <- { config with SnapshotProfile = value }
            save()

    member x.LoadModsOnStart
        with get() = config.LoadModsOnStart
        and set value = 
            config <- { config with LoadModsOnStart = value }
            save()

/// Used for Snapshot and Input profiles, since they both basically just have a name 
/// and description as far as the UI is concerned.
type SubProfileModel(name:string) =
    member x.Name with get() = name

type ViewModelMessage = Tick

type GameExePath = string
type LoaderState = 
    NotStarted
    | StartPending of (GameExePath)
    | StartFailed of (Exception * GameExePath)
    | Started of (Process * GameExePath)
    | Stopped of (Process * GameExePath)
    
type MainViewModel() as self = 
    inherit ViewModelBase()

    let EmptyProfile = ProfileModel(CoreTypes.DefaultRunConfig)

    let DesignMode = DesignerProperties.GetIsInDesignMode(new DependencyObject())

    let mutable selectedProfile = EmptyProfile

    let mutable loaderState = NotStarted

    // From: http://fsharpforfunandprofit.com/posts/concurrency-actor-model/
    let rec agent = MailboxProcessor.Start(fun inbox -> 
        let rec messageLoop() = async{
            let! msg = inbox.TryReceive(0)

            match msg with
            | None -> ()
            | Some (vmm) -> 
                match vmm with
                | Tick -> 
                    self.PeriodicUpdate()

            do! Async.Sleep(1000)
            agent.Post(Tick)
            
            return! messageLoop ()
        }

        messageLoop ())

    do
        RegConfig.init() // reg config requires init to set hive root

    let profiles = RegConfig.loadAll() |> Array.map (fun rc -> ProfileModel(rc))

    member x.PeriodicUpdate() = 
        x.UpdateLoaderState <|     
            match loaderState with
            | NotStarted -> loaderState
            | StartPending(_) -> loaderState
            | StartFailed (_) -> loaderState
            | Stopped (proc,exe) -> loaderState
            | Started (proc,exe) -> if proc.HasExited then Stopped (proc,exe) else loaderState

    member x.LoaderStateText 
        with get() = 
            match loaderState with
            | NotStarted -> "Not Started"
            | StartPending(_) -> "Start Pending..."
            | StartFailed (e,exe) -> sprintf "Start Failed: %s (target: %s)" e.Message exe
            | Stopped (proc,exe) -> 
                let exitReason = 
                    match ProcessUtil.LoaderExitReasons |> Map.tryFind proc.ExitCode with
                    | None -> sprintf "Unknown (code: %d)" proc.ExitCode
                    | Some (reason) -> reason
                    
                sprintf "Exited with status: %s (target: %s)" exitReason exe
            | Started (_,exe) -> sprintf "Started; waiting for exit (target: %s)" exe

    member x.Profiles = 
        if DesignMode then
            new ObservableCollection<ProfileModel>([||])
        else
            new ObservableCollection<ProfileModel>(profiles)

    member x.SnapshotProfiles = 
        new ObservableCollection<SubProfileModel>
            (SnapshotProfiles.ValidProfiles |> List.map (fun p -> SubProfileModel(p)))

    member x.InputProfiles = 
        new ObservableCollection<SubProfileModel>
            (InputProfiles.ValidProfiles |> List.map (fun p -> SubProfileModel(p)))

    member x.SelectedProfile 
        with get () = selectedProfile
        and set value = 
            selectedProfile <- value
   
            x.RaisePropertyChanged("SelectedProfile") 
            x.RaisePropertyChanged("SelectedInputProfile") 
            x.RaisePropertyChanged("ProfileAreaVisibility") 
            x.RaisePropertyChanged("ProfileDescription")
            x.UpdateLaunchUI()

    member x.SelectedInputProfile 
        with get () = x.SelectedProfile.InputProfile
        and set (value:string) = 
            x.SelectedProfile.InputProfile <- value
            x.RaisePropertyChanged("ProfileDescription")

    member x.SelectedSnapshotProfile
        with get () = x.SelectedProfile.SnapshotProfile
        and set (value:string) = 
            x.SelectedProfile.SnapshotProfile <- value
            x.RaisePropertyChanged("ProfileDescription")

    member x.ProfileDescription
        with get () =
            let inputText = 
                match (ProfileText.Input.Descriptions |> Map.tryFind x.SelectedProfile.InputProfile) with 
                | None -> "No Input description available"
                | Some (text) ->
                    LocStrings.Input.Header + "\n" + LocStrings.Input.Desc1 + "\n" + LocStrings.Input.Desc2 + "\n" + text
            let snapshotText = 
                let makeStringList (xforms:string list) =  String.Join(", ", xforms)

                let pxforms = 
                    match (SnapshotTransforms.Position |> Map.tryFind x.SelectedProfile.SnapshotProfile) with
                    | None -> "No Snapshot description available"
                    | Some (xforms) -> makeStringList xforms

                let uvxforms = 
                    match (SnapshotTransforms.UV |> Map.tryFind x.SelectedProfile.SnapshotProfile) with
                    | None -> "No Input description available"
                    | Some (xforms) -> makeStringList xforms
                LocStrings.Snapshot.Header + "\n" + LocStrings.Snapshot.Desc1 + "\n" + LocStrings.Snapshot.PosLabel + pxforms + "\n" 
                + LocStrings.Snapshot.UVLabel + uvxforms
            inputText + "\n" + snapshotText
             

    member x.LauncherProfileIcon 
        with get() = 
            match loaderState with 
            | Started (_,exe) 
            | StartPending (exe) -> 
                let profile = profiles |> Array.tryFind (fun p -> p.ExePath = exe)
                match profile with
                | None -> null
                | Some(p) -> p.Icon
            | NotStarted
            | StartFailed (_,_)
            | Stopped (_,_) -> x.SelectedProfile.Icon

    member x.ProfileAreaVisibility = 
        if  DesignMode || 
            selectedProfile.Name <> EmptyProfile.Name then
            Visibility.Visible
        else
            Visibility.Hidden

    member x.BrowseExe = alwaysExecutable (fun action ->
        match MainViewUtil.popSelectExecutableDialog(Some(x.SelectedProfile.ExePath)) with
        | None -> ()
        | Some (exePath) -> 
            // verify that the chosen path is not already claimed by another profile
            let existingProfileKey = RegConfig.findProfilePath exePath
            let ok = 
                match existingProfileKey with
                | None -> true
                | Some (key) ->
                    key.EndsWith(x.SelectedProfile.ProfileKeyName)

            if ok then
                x.SelectedProfile.ExePath <- exePath
                x.RaisePropertyChanged("SelectedProfile") 
            else
                MainViewUtil.failValidation "Cannot set exe path; it is already used by another profile")

    member x.UpdateLoaderState(newState) =
        if newState <> loaderState then
            loaderState <- newState
            x.UpdateLaunchUI()

    member x.UpdateLaunchUI() =
        x.RaisePropertyChanged("LoaderStateText") 
        x.RaisePropertyChanged("LoaderIsStartable") 
        x.RaisePropertyChanged("StartInSnapshotMode") 
        x.RaisePropertyChanged("LauncherProfileIcon")
        x.RaisePropertyChanged("ViewInjectionLog")
        x.RaisePropertyChanged("ViewModelModLog")

    member x.LoaderIsStartable
        with get() = 
            match loaderState with
            StartPending(_)
            | Started (_,_) -> false
            | StartFailed (_,_) 
            | Stopped (_,_) 
            | NotStarted -> true

    member x.StartInSnapshotMode = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible && x.LoaderIsStartable), 
            (fun action -> 
                x.UpdateLoaderState <|
                    match loaderState with 
                    | Started (proc,exe) -> 
                        // kill even in stopped case in case poll didn't catch exit
                        proc.Kill()
                        proc.WaitForExit()
                        Stopped(proc,exe)
                    | StartPending(_) -> loaderState
                    | Stopped (proc,exe) -> loaderState
                    | StartFailed (_) -> loaderState
                    | NotStarted -> loaderState

                x.UpdateLoaderState <| StartPending(x.SelectedProfile.ExePath)
                x.UpdateLoaderState <|
                    match (ProcessUtil.launchWithLoader x.SelectedProfile.ExePath) with 
                    | Ok(p) -> Started(p,x.SelectedProfile.ExePath)
                    | Err(e) -> 
                        MainViewUtil.failValidation e.Message
                        StartFailed(e,x.SelectedProfile.ExePath)))
    member x.ViewInjectionLog = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible && x.LoaderIsStartable),
            (fun action ->
                match ProcessUtil.openInjectionLog x.SelectedProfile.ExePath with
                | Ok(_) -> ()
                | Err(e) -> MainViewUtil.failValidation e.Message
            ))
    member x.ViewModelModLog = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible),
            (fun action ->
                match ProcessUtil.openModelModLog x.SelectedProfile.ExePath with
                | Ok(_) -> ()
                | Err(e) -> MainViewUtil.failValidation e.Message
            ))