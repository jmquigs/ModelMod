namespace MMLaunch

open System
open System.Windows
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
    let selectExecutableDialog() = 
        let dlg = new OpenFileDialog()

        //dlg.InitialDirectory <- state.Value.SnapshotRoot

        dlg.Filter <- "Executable files (*.exe)|*.exe"
        dlg.FilterIndex <- 0
        dlg.RestoreDirectory <- true

        let res = dlg.ShowDialog() 
        if res.HasValue && res.Value then
            Some (dlg.FileName)
        else
            None

type MainView = XAML<"MainWindow.xaml", true>

// Mutable wrapper around an immutable RunConfig; there are ways we could use RunConfig
// directly, but they use obtuse meta-wrappers; this is clearer at the expense of 
// some boilerplate.  We can also use this to store things that the run config won't 
// have, like logs and lists of mods.
type ProfileModel(config:CoreTypes.RunConfig) = 
    let mutable config = config

    let save() = RegConfig.saveProfile config

    member x.ProfileKeyName 
        with get() = config.ProfileKeyName

    member x.Name 
        with get() = config.ProfileName
        and set value = 
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

type MainViewModel() = 
    inherit ViewModelBase()

    let EmptyProfile = ProfileModel(CoreTypes.DefaultRunConfig)

    let DesignMode = DesignerProperties.GetIsInDesignMode(new DependencyObject())

    let mutable selectedProfile = EmptyProfile

    do
        RegConfig.init() // reg config requires init to set hive root

    member x.Profiles = 
        if DesignMode then
            new ObservableCollection<ProfileModel>([||])
        else
            new ObservableCollection<ProfileModel>
                (RegConfig.loadAll() |> Array.map (fun rc -> ProfileModel(rc)))
    
    member x.SelectedProfile 
        with get () = selectedProfile
        and set value = 
            selectedProfile <- value
            x.RaisePropertyChanged("SelectedProfile") 
            x.RaisePropertyChanged("ProfileAreaVisibility") 

    member x.ProfileAreaVisibility = 
        if  DesignMode || 
            selectedProfile.Name <> EmptyProfile.Name then
            Visibility.Visible
        else
            Visibility.Hidden

    member x.BrowseExe = alwaysExecutable (fun action ->
        match MainViewUtils.selectExecutableDialog() with
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
                MessageBox.Show("Cannot set exe path; it is already used by another profile") |> ignore
    )