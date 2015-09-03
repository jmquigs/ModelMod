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

open ViewModelUtils
open ModelMod

module MainViewUtils = 
    let selectExecutableDialog(currentExe:string option) = 
        let dlg = new OpenFileDialog()

        match currentExe with
        | None -> ()
        | Some exe ->
            if File.Exists(exe) then
                dlg.InitialDirectory <- Directory.GetParent(exe).ToString()

        //dlg.InitialDirectory <- state.Value.SnapshotRoot

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

    let launchWithLoader (exePath:string) =
        try 
            if not (File.Exists(exePath)) then
                failwithf "Exe does not exist: %s" exePath    
            // crude, but if it isn't an exe, we probably can't inject it because the loader won't find 
            // it.  and .com files aren't supported, ha ha
            if not (Path.GetExtension(exePath).ToLowerInvariant() = ".exe") then
                failwithf "Exe does not appear to be an exe: %s" exePath
            // find loader
            let loaderPath = 
                let lname = "MMLoader.exe"
                let devTreePath = Path.Combine("../../../Release", lname) // always use release version if running in a dev environment
                if File.Exists(lname) then Path.GetFullPath(lname)
                else if File.Exists(devTreePath) then Path.GetFullPath(devTreePath)
                else // dunno where it is
                    ""
            if not (File.Exists(loaderPath))
                then failwithf "Can't find %s" loaderPath

            let proc = new Process()
            proc.StartInfo.Verb <- "runas"; // loader requires elevation for poll mode
            proc.StartInfo.UseShellExecute <- true // also required for elevation
            proc.StartInfo.FileName <- loaderPath
            proc.StartInfo.Arguments <- sprintf "\"%s\"" exePath
            let res = proc.Start()
            if not res then 
                failwith "Failed to start loader process"

            let loaderProc = proc

            // pause for a second to avoid loader's "found on first launch" heuristic;
            // this could fail if the system is really slow, though, and can't get loader up in a second.
            // this is one of several race conditions here...
            Thread.Sleep(1000)

            // ok, loader is fired up, and by the time we get here the user has already accepted the elevation
            // dialog...so launch the target exe; loader will find it and inject.  this also should handle the
            // case where the exe restarts itself because it needs to be launched from some parent process
            // (e.g. Steam)
            // in theory loader could start the game too, but then it would start as admin, which we don't want.
            
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- false
            proc.StartInfo.FileName <- exePath
            let res = proc.Start()
            if not res then 
                // bummer, kill the loader
                loaderProc.Kill()
                loaderProc.WaitForExit()
                failwith "Failed to start game process"
            // we don't store a reference to the game process because we don't do anything with it at this point

            Some(loaderProc)
        with 
            | e -> 
                failValidation(e.Message)
                None

type MainView = XAML<"MainWindow.xaml", true>

// Mutable wrapper around an immutable RunConfig; there are ways we could use RunConfig
// directly, but they use obtuse meta-wrappers; this is clearer at the expense of 
// some boilerplate.  We can also use this to store things that the run config won't 
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

    member x.ProfileKeyName 
        with get() = config.ProfileKeyName

    member x.Name 
        with get() = config.ProfileName
        and set (value:string) = 
            let value = value.Trim()
            if value = "" then
                MainViewUtils.failValidation "Profile name may not be empty"
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

/// Used for Snapshot and Input profiles, since they both basically just have a name 
/// and description as far as the UI is concerned.
type SubProfileModel(name:string) =
    member x.Name with get() = name
    
type MainViewModel() = 
    inherit ViewModelBase()

    let EmptyProfile = ProfileModel(CoreTypes.DefaultRunConfig)

    let DesignMode = DesignerProperties.GetIsInDesignMode(new DependencyObject())

    let mutable selectedProfile = EmptyProfile

    let currentLaunchedProcess:Process option ref = ref None

    do
        RegConfig.init() // reg config requires init to set hive root

    member x.Profiles = 
        if DesignMode then
            new ObservableCollection<ProfileModel>([||])
        else
            new ObservableCollection<ProfileModel>
                (RegConfig.loadAll() |> Array.map (fun rc -> ProfileModel(rc)))

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
            x.RaisePropertyChanged("ProfileAreaVisibility") 
            x.RaisePropertyChanged("StartSnapshot") 

    member x.ProfileAreaVisibility = 
        if  DesignMode || 
            selectedProfile.Name <> EmptyProfile.Name then
            Visibility.Visible
        else
            Visibility.Hidden

    member x.BrowseExe = alwaysExecutable (fun action ->
        match MainViewUtils.selectExecutableDialog(Some(x.SelectedProfile.ExePath)) with
        | None -> ()
        | Some (exePath) -> 
            // verify that the chosen path is not already claimed by another profile
            let existingProfileKey = RegConfig.findProfileKeyName exePath
            let ok = 
                match existingProfileKey with
                | None -> true
                | Some (key) ->
                    key.EndsWith(x.SelectedProfile.ProfileKeyName)

            if ok then
                x.SelectedProfile.ExePath <- exePath
                x.RaisePropertyChanged("SelectedProfile") 
            else
                MainViewUtils.failValidation "Cannot set exe path; it is already used by another profile"
    )

    member x.StartSnapshot = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible), 
            (fun action -> 
                match currentLaunchedProcess.Value with 
                | Some (proc) ->
                    if not proc.HasExited then
                        proc.Kill()
                        proc.WaitForExit()
                | None -> ()

                currentLaunchedProcess.Value <- MainViewUtils.launchWithLoader x.SelectedProfile.ExePath
            ))

