namespace MMLaunch

open System
open System.Diagnostics
open System.Threading
open System.Windows
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

    module Misc = 
        let NewProfileName = "New Profile"
        let ConfirmRecycle = Formatters.StringAnyRetString("Recycle files in %s?\n%A")
        let ConfirmProfileRemove = Formatters.AnyRetString("Remove profile '%s'?\n\nNote: mod & snapshot files that are associated with this profile will not be removed.")
        let RecycleMoar = Formatters.StringAnyRetString("%s\n...and %d more")
        let LoaderNotStarted = "Not Started"
        let LoaderStartPending = "Start Pending..."
        let LoaderStopped = Formatters.StringAnyRetString("Exited with status: %s (target: %s)")
        let LoaderStarted = Formatters.AnyRetString("Started; waiting for exit (target: %s)")
        let ExeFilesFilter = "Executable files (*.exe)"

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

    member x.LoadModsOnStart
        with get() = config.LoadModsOnStart
        and set value = 
            config <- { config with LoadModsOnStart = value }
            save()

module MainViewUtil = 
    let pushSelectExecutableDialog(currentExe:string option) = 
        let initialDir = 
            match currentExe with
            | None -> None
            | Some exe when File.Exists(exe) -> Some(Directory.GetParent(exe).ToString())
            | Some exe -> None

        ViewModelUtil.pushSelectFileDialog (initialDir,LocStrings.Misc.ExeFilesFilter + "|*.exe")

    let private getDirLocator (profile:ProfileModel) =
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

    let pushRemoveSnapshotsDialog (profile:ProfileModel) =
        let snapdir = getSnapshotDir profile
        if not (Directory.Exists snapdir) then
            ViewModelUtil.pushDialog (sprintf LocStrings.Errors.SnapshotDirNotFound snapdir)
        else
            let files = Directory.GetFiles(snapdir);

            let ok = 
                let display = 5
                let take = Math.Min(files.Length,display)
                let moar = files.Length - take

                if files.Length > 0 then
                    let files = files |> Seq.take take |> Array.ofSeq
                    
                    let msg = sprintf LocStrings.Misc.ConfirmRecycle snapdir files
                    let msg = if moar = 0 then msg else (sprintf  LocStrings.Misc.RecycleMoar msg moar) 
                    match (ViewModelUtil.pushOkCancelDialog msg) with
                    | MessageBoxResult.Yes -> true
                    | _ -> false
                else 
                    ViewModelUtil.pushDialog (sprintf LocStrings.Errors.NoFilesInDir snapdir)
                    false
            if ok then
                for f in files do
                    FileSystem.DeleteFile(f, UIOption.OnlyErrorDialogs, RecycleOption.SendToRecycleBin)

    let pushCreateModDialog (parentWin:Window) (profile:ProfileModel) =
        let cw = new CreateModView()

        // put some stuff in its viewmodel
        let vm = cw.Root.DataContext :?> CreateModViewModel
        vm.SnapshotDir <- getSnapshotDir(profile)
        vm.DataDir <- getDataDir(profile)
        let previewHost = cw.Root.FindName("ModelPreview") :?> PreviewHost
        vm.PreviewHost <- Some(previewHost)

        cw.Root.Owner <- parentWin
        cw.Root.ShowDialog() |> ignore
        
    let failValidation (msg:string) = ViewModelUtil.pushDialog msg

    let profileDirHasFiles (p:ProfileModel) (dirSelector: ProfileModel -> string) =
        if p.ExePath = "" 
        then false
        else
            let sd = dirSelector p
            if not (Directory.Exists sd) 
            then false
            else Directory.EnumerateFileSystemEntries(sd).GetEnumerator().MoveNext()   

    let openModsDir (p:ProfileModel) =
        let hasfiles = profileDirHasFiles p (fun p -> getDataDir p)
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

type ViewModelMessage = Tick

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

    let EmptyProfile = ProfileModel(CoreTypes.DefaultRunConfig)

    let mutable selectedProfile:ProfileModel option = None
    let mutable loaderState = NotStarted

    do
        RegConfig.init() // reg config requires init to set hive root

    let observableProfiles = 
        new ObservableCollection<ProfileModel>(
            RegConfig.loadAll() |> 
                Array.fold (fun (acc: ResizeArray<ProfileModel>) rc -> acc.Add( ProfileModel(rc)); acc ) (new ResizeArray<ProfileModel>()))

    // Start an agent to give us a perodic timer
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
            | Started (proc,exe) -> if proc.HasExited then Stopped (proc,exe) else loaderState

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
            (SnapshotProfiles.ValidProfiles |> List.map (fun p -> SubProfileModel(p)))

    member x.InputProfiles = 
        new ObservableCollection<SubProfileModel>
            (InputProfiles.ValidProfiles |> List.map (fun p -> SubProfileModel(p)))

    member x.SelectedProfile 
        with get () = selectedProfile
        and set value = 
            selectedProfile <- value
   
            x.RaisePropertyChanged("DeleteProfile") 
            x.RaisePropertyChanged("SelectedProfile") 
            x.RaisePropertyChanged("SelectedProfileName") 
            x.RaisePropertyChanged("SelectedProfileExePath") 
            x.RaisePropertyChanged("SelectedProfileLoadModsOnStart") 
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

                    let pxforms = 
                        match (SnapshotTransforms.Position |> Map.tryFind profile.SnapshotProfile) with
                        | None -> LocStrings.Errors.NoSnapshotDescription
                        | Some (xforms) -> makeStringList xforms

                    let uvxforms = 
                        match (SnapshotTransforms.UV |> Map.tryFind profile.SnapshotProfile) with
                        | None -> LocStrings.Errors.NoSnapshotDescription
                        | Some (xforms) -> makeStringList xforms
                    LocStrings.Snapshot.Header + "\n" + LocStrings.Snapshot.Desc1 + "\n" + LocStrings.Snapshot.PosLabel + pxforms + "\n" 
                    + LocStrings.Snapshot.UVLabel + uvxforms
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
            | StartFailed (_,_)
            | Stopped (_,_) -> 
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
            | Started (_,_) -> false
            | StartFailed (_,_) 
            | Stopped (_,_) 
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

    member x.RemoveSnapshots =  
        new RelayCommand (
            (fun canExecute -> x.HasSnapshots), 
            (fun action ->
                x.SelectedProfile |> Option.iter (fun profile -> MainViewUtil.pushRemoveSnapshotsDialog profile)))

    member x.CreateMod = 
        new RelayCommand (
            (fun canExecute -> x.HasSnapshots), 
            (fun mainWin ->
                let mainWin = mainWin :?> Window
                x.SelectedProfile |> Option.iter (fun profile -> MainViewUtil.pushCreateModDialog mainWin profile)))

    member x.OpenMods =
        new RelayCommand (
            (fun canExecute -> x.HasModFiles),
            (fun action ->
                x.SelectedProfile |> Option.iter (fun profile -> MainViewUtil.openModsDir profile )))

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
                    x.UpdateLoaderState <|
                        match (ProcessUtil.launchWithLoader selectedProfile.ExePath) with 
                        | Ok(p) -> Started(p,selectedProfile.ExePath)
                        | Err(e) -> 
                            MainViewUtil.failValidation e.Message
                            StartFailed(e,selectedProfile.ExePath))))
                
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