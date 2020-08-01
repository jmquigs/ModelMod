// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
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
open System.Windows.Controls
open System.Windows.Input
open System.ComponentModel
open System.Collections.ObjectModel
open Microsoft.Win32

open Microsoft.VisualBasic.FileIO // for recycling bin thing

open FsXaml

open ViewModelUtil
open ModelMod

type MainView = XAML<"MainWindow.xaml", true>

// Helper module for using parameterized loc strings with failwith, etc 
// (since we can't pass them directly as an argument):
// http://stackoverflow.com/questions/18551851/why-does-fs-printfn-work-with-literal-strings-but-not-values-of-type-string
module Formatters =
    type String = Printf.StringFormat<string -> unit>
    type StringRetString = Printf.StringFormat<string -> string>
    type StringAnyRetString<'a> = Printf.StringFormat<string -> 'a -> string>
    type AnyRetString<'a> = Printf.StringFormat<'a -> string>

module LocStrings = 
    // Ideally these strings would actually be localized someday using whatever method.
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
        let Header = "Snapshot Transforms:"
        let Desc1 = "The following transforms will be applied"
        let PosLabel = "Position: "
        let UVLabel = "UV: "

    module Errors = 
        let ProfileNameRequired = "Profile name may not be empty"
        let NoFilesInDir = Formatters.AnyRetString("No files in %s")
        let SnapshotDirNotFound = Formatters.StringRetString("Snapshot directory does not exist: %s")
        let CantFindLoader = Formatters.String("Unable to find loader path; check that %s is built for this configuration")
        let LoaderStartFailed = Formatters.StringAnyRetString("Start Failed: %s (target: %s)")
        let LoaderUnknownExit = Formatters.AnyRetString("Unknown (code: %d)")
        let NoInputDescription = "No Input description available"
        let NoSnapshotDescription = "No Snapshot description available"
        let BadExePath = "Cannot set exe path; it is already used by another profile"
        let NoCRuntime = "The Visual C++ Runtime was not detected; ModelMod cannot work without it.  Would you open a browser to Microsoft's web site to download it?"
        let NoCRuntimeRestart = "Please install the visual C++ runtime and then restart ModelMod."

    module Misc = 
        let NewProfileName = "New Profile"
        let ConfirmRecycle = Formatters.StringAnyRetString("Remove files in %s?\n%A")
        let ConfirmProfileRemove = Formatters.AnyRetString("Remove profile '%s'?\n\nNote: mod & snapshot files that are associated with this profile will not be removed.")
        let RecycleMoar = Formatters.StringAnyRetString("%s\n...and %d more")
        let RecycleSlow = "Use Recycle Bin (can be slow,blocks UI; yeah lame I know)"
        let LoaderNotStarted = "Not Started"
        let LoaderStartPending = "Start Pending..."
        let LoaderStopped = Formatters.StringAnyRetString("Exited with status: %s (target: %s)")
        let LoaderStarted = Formatters.AnyRetString("Started; waiting for exit (target: %s)")
        let ExeFilesFilter = "Executable files (*.exe)"

module ProfileText = 
    module Input = 
        let CommandOrder = [
            LocStrings.Input.ReloadMods; LocStrings.Input.Toggle;
            LocStrings.Input.ClearTex; 
            LocStrings.Input.SelectNextTex; LocStrings.Input.SelectPrevTex; LocStrings.Input.DoSnapshot
            LocStrings.Input.Reload]
        let PunctKeys = [@"\"; "]"; 
            ";"; 
            ","; "."; "/"; "-"]
        let FKeys = ["F1"; "F2";
            "F6";
            "F3"; "F4"; "F7"; "F10"]
    
        let Descriptions =
            let makeInputDesc keys = 
                List.fold2 (fun acc key text -> 
                    acc + (sprintf "%s\t%s\n" key text)
                ) "" keys CommandOrder

            Map.ofList [ (InputProfiles.PunctRock, (makeInputDesc PunctKeys)); 
                (InputProfiles.FItUp, (makeInputDesc FKeys)); ]

// Mutable wrapper around an immutable RunConfig; there are ways we could use RunConfig
// directly, but we can also use this to store things that the run config won't 
// have, like logs and lists of mods.
type ProfileModel(config:CoreTypes.RunConfig) = 
    let mutable config = config

    // set defaults for empty profile values
    let mutable config = 
        if config.SnapshotProfile.Trim() = ""
            then { config with SnapshotProfile = SnapshotProfile.DefaultProfileName } else config
    let mutable config = 
        if config.InputProfile.Trim() = "" || not (InputProfiles.isValid config.InputProfile)
            then { config with InputProfile = InputProfiles.DefaultProfile} else config

    let save() = 
        try 
            RegConfig.saveProfile config
        with 
            | e -> ViewModelUtil.pushDialog (sprintf "%s" e.Message)

    let mutable iconSource = null

    do
        if File.Exists (config.ExePath) then
            use icon = System.Drawing.Icon.ExtractAssociatedIcon(config.ExePath)
            iconSource <-
                System.Windows.Interop.Imaging.CreateBitmapSourceFromHIcon(
                    icon.Handle,
                    System.Windows.Int32Rect.Empty,
                    System.Windows.Media.Imaging.BitmapSizeOptions.FromEmptyOptions())

    member x.Config = config

    member x.Icon 
        with get() = iconSource

    member x.ProfileKeyName 
        with get() = config.ProfileKeyName

    member x.Name 
        with get() = config.ProfileName
        and set (value:string) = 
            let value = value.Trim()
            if value = "" then
                ViewModelUtil.pushDialog LocStrings.Errors.ProfileNameRequired
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

    member x.LaunchWindow
        with get() = config.LaunchWindow
        and set value = 
            config <- { config with LaunchWindow = value }
            save()

    member x.LoadModsOnStart
        with get() = config.LoadModsOnStart
        and set value = 
            config <- { config with LoadModsOnStart = value }
            save()

    member x.GameProfile 
        with get() = config.GameProfile
        and set value = 
            config <- { config with GameProfile = value }
            save()

module MainViewUtil = 
    let pushSelectExecutableDialog(currentExe:string option) = 
        let initialDir = 
            match currentExe with
            | None -> None
            | Some exe when File.Exists(exe) -> Some(Directory.GetParent(exe).ToString())
            | Some exe -> None

        ViewModelUtil.pushSelectFileDialog (initialDir,LocStrings.Misc.ExeFilesFilter + "|*.exe")

    let getDirLocator (profile:ProfileModel) =
        let lp = ProcessUtil.getLoaderPath()
        if lp = "" then failwithf LocStrings.Errors.CantFindLoader ProcessUtil.LoaderName
        let root = Directory.GetParent(lp).FullName
        let root = Path.Combine(root, "..")
        let dl = State.DirLocator(root, profile.Config)
        dl

    let getSnapshotDir (profile:ProfileModel) =
        Path.GetFullPath(getDirLocator(profile).ExeSnapshotDir)

    let getDataDir (profile:ProfileModel) =
        Path.GetFullPath(getDirLocator(profile).ExeDataDir)

    let pushDeleteProfileDialog (profile:ProfileModel) =
        let msg = sprintf LocStrings.Misc.ConfirmProfileRemove profile.Name
        match (ViewModelUtil.pushOkCancelDialog msg) with
        | MessageBoxResult.Yes -> true
        | _ -> false

    let makeBlenderWindow (parentWin:Window) =
        let view = new BlenderView()
        view.Root.Owner <- parentWin
        let vm = view.Root.DataContext :?> BlenderViewModel
        (view,vm)
        
    let makePreferencesWindow (parentWin:Window) =
        let view = new PreferencesView()
        view.Root.Owner <- parentWin
        let vm = view.Root.DataContext :?> PreferencesViewModel
        (view,vm)

    let makeGameProfileWindow (parentWin:Window) =
        let view = new GameProfileView()
        view.Root.Owner <- parentWin
        let vm = view.Root.DataContext :?> GameProfileViewModel
        (view,vm)

    let makeConfirmDialog (parentWin:Window) =
        let view = new ConfirmDialogView()
        view.Root.Owner <- parentWin
        let vm = view.Root.DataContext :?> ConfirmDialogViewModel
        vm.View <- view
        (view,vm)

    let pushRemoveSnapshotsDialog (mainWin:Window) (profile:ProfileModel) =
        let snapdir = getSnapshotDir profile
        if not (Directory.Exists snapdir) then
            ViewModelUtil.pushDialog (sprintf LocStrings.Errors.SnapshotDirNotFound snapdir)
        else
            let files = Directory.GetFiles(snapdir);

            let (ok,recycle) = 
                let display = 5
                let take = Math.Min(files.Length,display)
                let moar = files.Length - take

                if files.Length > 0 then
                    let files = files |> Seq.take take |> Array.ofSeq
                    
                    let msg = sprintf LocStrings.Misc.ConfirmRecycle snapdir files
                    let msg = if moar = 0 then msg else (sprintf  LocStrings.Misc.RecycleMoar msg moar) 
                    let view,vm = makeConfirmDialog mainWin
                    vm.CheckBoxText <- LocStrings.Misc.RecycleSlow
                    vm.Text <- msg
                    vm.CheckboxChecked <- RegConfig.getGlobalValue RegKeys.RecycleSnapshots 1 |> (fun v -> v :?> int) |> RegUtil.dwordAsBool
                    view.Root.ShowDialog() |> ignore
                    vm.Confirmed,vm.CheckboxChecked
                else 
                    ViewModelUtil.pushDialog (sprintf LocStrings.Errors.NoFilesInDir snapdir)
                    false,true
            if ok then
                // save the recycle pref
                RegConfig.setGlobalValue RegKeys.RecycleSnapshots (recycle |> RegUtil.boolAsDword) |> ignore

                if recycle then
                    // invoke the power of VB!
                    // dunno why this is sofa king slow, but probably we should be pushing a progress dialog or doing it async.
                    // (if we do it async, and the user is on the create mods dialog, the user will get to watch the 
                    // snapshot list slowly drain away, which could be useful, or annoying)
                    for f in files do
                        FileSystem.DeleteFile(f, UIOption.OnlyErrorDialogs, RecycleOption.SendToRecycleBin)
                else
                    // smoke 'em
                    for f in files do
                        File.Delete(f)

    let makeCreateModDialog (parentWin:Window) (profile:ProfileModel) =
        let cw = new CreateModView()

        // put some stuff in its viewmodel
        let vm = cw.Root.DataContext :?> CreateModViewModel
        vm.SnapshotDir <- getSnapshotDir(profile)
        vm.DataDir <- getDataDir(profile)
        let modNameTB = cw.Root.FindName("ModName") :?> TextBox
        let previewHost = cw.Root.FindName("ModelPreview") :?> PreviewHost
        vm.PreviewHost <- Some(previewHost)
        vm.ModNameTB <- Some(modNameTB)

        cw.Root.Owner <- parentWin
        (cw,vm)
        
    let failValidation (msg:string) = ViewModelUtil.pushDialog msg

    let profileDirHasFiles (p:ProfileModel) (dirSelector: ProfileModel -> string) =
        // dir locator may throw exception if data dir does not exist
        try 
            if p.ExePath = "" 
            then false
            else
                let sd = dirSelector p
                if not (Directory.Exists sd) 
                then false
                else Directory.EnumerateFileSystemEntries(sd).GetEnumerator().MoveNext()
        with 
            | e -> false

    let openModsDir (p:ProfileModel) =
        let hasfiles = profileDirHasFiles p getDataDir
        if hasfiles then
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- true
            proc.StartInfo.FileName <- getDataDir p 
            proc.Start() |> ignore

    // utility method to faciliate various hacks with the Profiles list box
    let findProfilesListBox (mainWin:obj):ListBox option =
        let mainWin = mainWin :?> Window
        match mainWin with 
        | null -> None
        | win ->
            let lb = win.FindName("ProfilesListBox") :?> System.Windows.Controls.ListBox
            match lb with
            | null -> None
            | _ -> Some(lb)

/// Used for Snapshot and Input profiles, since they both basically just have a name 
/// and description as far as the UI is concerned.
type SubProfileModel(name:string) =
    member x.Name with get() = name

type LaunchWindowModel(name:string,time:int) =
    member x.Name with get() = name
    member x.Time with get() = time

type GameExePath = string
type LoaderState = 
    NotStarted
    | StartPending of (GameExePath)
    | StartFailed of (Exception * GameExePath)
    | Started of (Process * GameExePath)
    | Stopped of (Process * GameExePath)

type ProfileModelConverter() =
    inherit Converter<ProfileModel option, obj>(
        (fun value cparams ->
            match value with
            | None -> null
            | Some p -> box p),
        ProfileModel(CoreTypes.DefaultRunConfig),
        (fun value cparams ->
            match value with
            | null -> None
            | p -> Some(p :?> ProfileModel)),
        None)

#nowarn "40"
type MainViewModel() as self = 
    inherit ViewModelBase()

    let emptyProfile = ProfileModel(CoreTypes.DefaultRunConfig)

    let mutable selectedProfile:ProfileModel option = None
    let mutable loaderState = NotStarted

    do
        RegConfig.init() // reg config requires init to set hive root

    let snapshotProfileDefs,snapshotProfileNames = 
        try 
            let defs = SnapshotProfile.GetAll (ProcessUtil.getMMRoot())
            let names = defs |> Map.toList |> List.map fst
            defs,names
        with 
            | e -> Map.ofList [],[]

    let observableProfiles = 
        new ObservableCollection<ProfileModel>(
            RegConfig.loadAll() 
            |> Array.sortBy (fun gp -> gp.ProfileName.ToLowerInvariant().Trim())
            |> Array.fold (fun (acc: ResizeArray<ProfileModel>) rc -> acc.Add( ProfileModel(rc)); acc ) (new ResizeArray<ProfileModel>()))

    let timer = new DispatcherTimer()
    do 
        timer.Interval <- new TimeSpan(0,0,1)
        timer.Tick.Add(fun (args) -> self.PeriodicUpdate())
        timer.Start()

    let launchWindows = [new LaunchWindowModel("5 Seconds", 5); new LaunchWindowModel("15 Seconds", 15); new LaunchWindowModel("30 Seconds", 30); new LaunchWindowModel("45 Seconds", 45);]

    let getSelectedProfileField (getter:ProfileModel -> 'a) (devVal:'a) = 
         match selectedProfile with
         | None -> devVal
         | Some profile -> getter profile
    let setSelectedProfileField (setter:ProfileModel -> unit) =
        match selectedProfile with
        | None -> ()
        | Some profile -> setter profile 

    member x.PeriodicUpdate() = 

        x.UpdateLoaderState <|
            match loaderState with
            | NotStarted 
            | StartPending(_) 
            | StartFailed (_) -> loaderState
            | Stopped (proc,exe) -> loaderState
            | Started (proc,exe) -> 
                if proc.HasExited 
                then 
                    try
                        File.Delete(Path.Combine(Path.GetDirectoryName(exe), @"ModelModCLRAppDomain.dll"))
                    with 
                        | e -> ()

                    Stopped (proc,exe) 
                else loaderState

        x.UpdateProfileButtons()

    member x.LoaderStateText 
        with get() = 
            match loaderState with
            | NotStarted -> LocStrings.Misc.LoaderNotStarted 
            | StartPending(_) -> LocStrings.Misc.LoaderStartPending 
            | StartFailed (e,exe) -> sprintf LocStrings.Errors.LoaderStartFailed e.Message exe 
            | Stopped (proc,exe) -> 
                let exitReason = 
                    ProcessUtil.getLoaderExitReason proc (sprintf LocStrings.Errors.LoaderUnknownExit proc.ExitCode)
                    
                sprintf LocStrings.Misc.LoaderStopped exitReason exe
            | Started (_,exe) -> sprintf LocStrings.Misc.LoaderStarted exe 

    member x.Profiles = 
        if ViewModelUtil.DesignMode then
            new ObservableCollection<ProfileModel>([||])
        else
            observableProfiles

    member x.SnapshotProfiles = 
        new ObservableCollection<SubProfileModel>
            (snapshotProfileNames |> List.map (fun p -> SubProfileModel(p)))

    member x.InputProfiles = 
        new ObservableCollection<SubProfileModel>
            (InputProfiles.ValidProfiles |> List.map (fun p -> SubProfileModel(p)))

    member x.LaunchWindows = 
        new ObservableCollection<LaunchWindowModel>(launchWindows)

    member x.SelectedProfile 
        with get () = selectedProfile
        and set value = 
            selectedProfile <- value
   
            x.RaisePropertyChanged("DeleteProfile") 
            x.RaisePropertyChanged("SelectedProfile") 
            x.RaisePropertyChanged("SelectedProfileName") 
            x.RaisePropertyChanged("SelectedProfileExePath") 
            x.RaisePropertyChanged("SelectedProfileLoadModsOnStart") 
            x.RaisePropertyChanged("SelectedProfileLaunchWindow") 
            x.RaisePropertyChanged("SelectedInputProfile") 
            x.RaisePropertyChanged("SelectedSnapshotProfile") 
            x.RaisePropertyChanged("ProfileAreaVisibility") 
            x.RaisePropertyChanged("ProfileDescription")
            x.UpdateLaunchUI()
            x.UpdateProfileButtons()

    // Various boilerplate accessors for the selected profile: I put these in
    // because the XAML field notation (e.g. SelectedProfile.Name) won't 
    // trigger the converter on SelectedProfile before looking for .Name,
    // so it fails, because SelectedProfile is an option type.
    // There is probably a better way to do this, since this kinda sucks.
    member x.SelectedProfileName 
        with get () = getSelectedProfileField (fun profile -> profile.Name) ""
        and set (value:string) = setSelectedProfileField (fun profile -> profile.Name <- value)

    member x.SelectedProfileExePath
        with get () = getSelectedProfileField (fun profile -> profile.ExePath) ""
        and set (value:string) = setSelectedProfileField (fun profile -> profile.ExePath <- value)

    member x.SelectedProfileLoadModsOnStart
        with get () = getSelectedProfileField (fun profile -> profile.LoadModsOnStart) CoreTypes.DefaultRunConfig.LoadModsOnStart
        and set (value:bool) = setSelectedProfileField (fun profile -> profile.LoadModsOnStart <- value)

    member x.SelectedProfileLaunchWindow 
        with get() = 
            let time = getSelectedProfileField (fun profile -> profile.LaunchWindow) CoreTypes.DefaultRunConfig.LaunchWindow
            let found = launchWindows |> List.tryFind (fun lt -> lt.Time = time)
            match found with
            | None -> launchWindows.Head.Time
            | Some (lw) -> lw.Time

        and set (value:int) = 
            setSelectedProfileField (fun profile -> profile.LaunchWindow <- value)

    member x.SelectedInputProfile 
        with get () = getSelectedProfileField (fun profile -> profile.InputProfile) CoreTypes.DefaultRunConfig.InputProfile
        and set (value:string) = 
            setSelectedProfileField (fun profile -> profile.InputProfile <- value)
            x.RaisePropertyChanged("ProfileDescription")

    member x.SelectedSnapshotProfile
        with get () = getSelectedProfileField (fun profile -> profile.SnapshotProfile) CoreTypes.DefaultRunConfig.SnapshotProfile
        and set (value:string) = 
            setSelectedProfileField (fun profile -> profile.SnapshotProfile <- value)
            x.RaisePropertyChanged("ProfileDescription")

    member x.ProfileDescription
        with get () =
            match x.SelectedProfile with
            | None -> ""
            | Some profile ->
                let inputText = 
                    match (ProfileText.Input.Descriptions |> Map.tryFind profile.InputProfile) with 
                    | None -> LocStrings.Errors.NoInputDescription
                    | Some (text) ->
                        LocStrings.Input.Header + "\n" + LocStrings.Input.Desc1 + "\n" + LocStrings.Input.Desc2 + "\n" + text
                let snapshotText = 
                    let makeStringList (xforms:string list) =  String.Join(", ", xforms)

                    let stext = 
                        snapshotProfileDefs
                        |> Map.tryFind profile.SnapshotProfile 
                        |> function
                            | None -> LocStrings.Errors.NoSnapshotDescription
                            | Some profile -> 
                                LocStrings.Snapshot.Desc1 + "\n" + LocStrings.Snapshot.PosLabel + (makeStringList <| profile.PosXForm()) + "\n" 
                                + LocStrings.Snapshot.UVLabel + (makeStringList <| profile.UVXForm())
                                    
                    LocStrings.Snapshot.Header + "\n" + stext

                inputText + "\n" + snapshotText

    member x.LauncherProfileIcon 
        with get() = 
            match loaderState with 
            | Started (_,exe) 
            | StartPending (exe) -> 
                let profile = observableProfiles |> Seq.tryFind (fun p -> p.ExePath = exe)
                match profile with
                | None -> null
                | Some(p) -> p.Icon
            | NotStarted
            | StartFailed (_)
            | Stopped (_) -> 
                match x.SelectedProfile with
                | None -> null
                | Some(p) -> p.Icon

    member x.ProfileAreaVisibility = 
        if  DesignMode || 
            selectedProfile.IsSome then
            Visibility.Visible
        else
            Visibility.Hidden

    member x.BrowseExe = alwaysExecutable (fun mainWin ->
        selectedProfile |> Option.iter (fun selectedProfile ->
            match MainViewUtil.pushSelectExecutableDialog(Some(selectedProfile.ExePath)) with
            | None -> ()
            | Some (exePath) -> 
                // verify that the chosen path is not already claimed by another profile
                let existingProfileKey = RegConfig.findProfilePath exePath
                let ok = 
                    match existingProfileKey with
                    | None -> true
                    | Some (key) ->
                        key.EndsWith(selectedProfile.ProfileKeyName)

                if ok then
                    selectedProfile.ExePath <- exePath
                    if selectedProfile.Name = "" || selectedProfile.Name.StartsWith(LocStrings.Misc.NewProfileName) then
                        selectedProfile.Name <- RegConfig.getDefaultProfileName exePath

                        // update the name in the list box using this heavy-handed method; raising changed 
                        // events doesn't seem to be enough
                        MainViewUtil.findProfilesListBox mainWin |> Option.iter (fun lb -> lb.Items.Refresh())

                    // force an update to the selectedProfile to update view model stuff (lame)
                    x.SelectedProfile <- x.SelectedProfile
                else
                    MainViewUtil.failValidation LocStrings.Errors.BadExePath))


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

    member x.UpdateProfileButtons() =
        x.RaisePropertyChanged("RemoveSnapshots")
        x.RaisePropertyChanged("CreateMod")
        x.RaisePropertyChanged("OpenMods")

    member x.HasModFiles 
        with get() = 
            match x.SelectedProfile with
            | None -> false
            | Some profile -> MainViewUtil.profileDirHasFiles profile (fun prof -> (MainViewUtil.getDataDir prof))

    member x.HasSnapshots 
        with get() = 
            match x.SelectedProfile with
            | None -> false
            | Some profile -> MainViewUtil.profileDirHasFiles profile (fun prof -> (MainViewUtil.getSnapshotDir prof))

    member x.LoaderIsStartable
        with get() = 
            match loaderState with
            StartPending(_)
            | Started (_) -> false
            | StartFailed (_) 
            | Stopped (_) 
            | NotStarted -> true

    member x.NewProfile = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun mainWin ->
                // find temp profile name
                let seqNextName = seq {
                    let rec next count = 
                        let nextName = if count = 0 then LocStrings.Misc.NewProfileName else (sprintf "%s (%d)" LocStrings.Misc.NewProfileName count)
                        let found = observableProfiles |> Seq.tryFind (fun pm -> pm.Name = nextName)
                        match found with
                        | None -> nextName
                        | Some v -> next (count+1)
                    yield (next 0)
                }

                let profile = new ProfileModel( { RegConfig.loadDefaultProfile()  with ProfileName = (Seq.head seqNextName) })
                observableProfiles.Add(profile)
                
                x.RaisePropertyChanged("Profiles")
                x.SelectedProfile <- Some(profile)
                
                // force the lb to scroll manually
                MainViewUtil.findProfilesListBox mainWin |> Option.iter (fun lb -> lb.ScrollIntoView(profile))))

    member x.DeleteProfile = 
        new RelayCommand (
            (fun canExecute -> selectedProfile.IsSome ), 
            (fun action ->
                x.SelectedProfile |> Option.iter (fun profile ->
                    if (MainViewUtil.pushDeleteProfileDialog profile) then
                        try 
                            if profile.Config.ProfileKeyName <> "" then
                                RegConfig.removeProfile profile.Config
                        
                            if (observableProfiles.Count <= 1) then
                                x.SelectedProfile <- None
                                observableProfiles.Clear()
                            else
                                x.SelectedProfile <- Some(observableProfiles.Item(observableProfiles.Count - 1))
                                x.UpdateLaunchUI()
                                x.UpdateProfileButtons()

                                observableProfiles.Remove(profile) |> ignore
                        with
                            | e -> 
                                ViewModelUtil.pushDialog e.Message)))

    member x.DoRemoveSnapshots (mainWin:Window) = 
        x.SelectedProfile |> Option.iter (fun profile ->
            MainViewUtil.pushRemoveSnapshotsDialog mainWin profile)

    member x.RemoveSnapshots =
        new RelayCommand (
            (fun canExecute -> x.HasSnapshots), 
            (fun mainWin -> x.DoRemoveSnapshots(mainWin :?> Window)))

    member x.CreateMod = 
        new RelayCommand (
            (fun canExecute -> x.HasSnapshots), 
            (fun mainWin ->
                let mainWin = mainWin :?> Window
                x.SelectedProfile |> Option.iter (fun profile ->
                    let view,vm = MainViewUtil.makeCreateModDialog mainWin profile
                    vm.RemoveSnapshotsFn <- ( fun _ -> x.DoRemoveSnapshots(mainWin))
                    view.Root.ShowDialog() |> ignore)))

    member x.OpenMods =
        new RelayCommand (
            (fun canExecute -> x.HasModFiles),
            (fun action ->
                x.SelectedProfile |> Option.iter MainViewUtil.openModsDir))

    member x.SetupBlender = 
        new RelayCommand (
            (fun canExecute -> true),
            (fun mainWin ->
                let mainWin = mainWin :?> Window
                let view,_ = MainViewUtil.makeBlenderWindow(mainWin)
                view.Root.ShowDialog() |> ignore
            ))

    member x.OpenPreferences = 
        new RelayCommand (
            (fun canExecute -> true),
            (fun mainWin ->
                let mainWin = mainWin :?> Window
                let view,_ = MainViewUtil.makePreferencesWindow(mainWin)
                view.Root.ShowDialog() |> ignore
            ))

    member x.OpenGameProfile = 
        new RelayCommand (
            (fun canExecute -> true),
            (fun mainWin ->
                let mainWin = mainWin :?> Window
                let view,vm = MainViewUtil.makeGameProfileWindow(mainWin)
                if x.SelectedProfile.IsSome then
                    vm.Profile <- x.SelectedProfile.Value.GameProfile

                vm.ProfileChangedCb <- (fun gameProfile ->
                    x.SelectedProfile |> Option.iter (fun profile ->
                        profile.GameProfile <- gameProfile
                    ))
                view.Root.ShowDialog() |> ignore
            ))

    member x.StartInSnapshotMode = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible && x.LoaderIsStartable), 
            (fun action -> 
                x.SelectedProfile |> Option.iter (fun selectedProfile -> 
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

                    x.UpdateLoaderState <| StartPending(selectedProfile.ExePath)
                    
                    try
                        // make sure the data dir exists
                        let dir = MainViewUtil.getDirLocator(selectedProfile).QueryBaseDataDir()
                        if dir <> "" && not (Directory.Exists dir) then
                            Directory.CreateDirectory(dir) |> ignore

                        let dir = MainViewUtil.getDataDir selectedProfile
                        if dir <> "" && not (Directory.Exists dir) then
                            Directory.CreateDirectory(dir) |> ignore

                        let launchWindow = selectedProfile.LaunchWindow

                        // start it 
                        x.UpdateLoaderState <|
                            match (ProcessUtil.launchWithLoader selectedProfile.ExePath selectedProfile.GameProfile.CommandLineArguments launchWindow) with 
                            | Ok(p) -> Started(p,selectedProfile.ExePath)
                            | Err(e) -> 
                                MainViewUtil.failValidation e.Message
                                StartFailed(e,selectedProfile.ExePath)
                    with 
                        | e -> MainViewUtil.failValidation (sprintf "Failed to start: %s" e.Message)
            )))

                
    member x.ViewInjectionLog = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible && x.LoaderIsStartable),
            (fun action ->
                x.SelectedProfile |> Option.iter (fun profile ->
                    match ProcessUtil.openInjectionLog profile.ExePath with
                    | Ok(_) -> ()
                    | Err(e) -> MainViewUtil.failValidation e.Message)))
    member x.ViewModelModLog = 
        new RelayCommand (
            (fun canExecute -> x.ProfileAreaVisibility = Visibility.Visible),
            (fun action ->
                x.SelectedProfile |> Option.iter (fun profile ->
                    match ProcessUtil.openModelModLog profile.ExePath with
                    | Ok(_) -> ()
                    | Err(e) -> MainViewUtil.failValidation e.Message)))