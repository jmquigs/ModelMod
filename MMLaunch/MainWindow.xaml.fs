namespace MMLaunch

open System
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input
open System.ComponentModel
open System.Collections.ObjectModel

open FsXaml

open ViewModelUtils
open ModelMod

type MainView = XAML<"MainWindow.xaml", true>

type MainViewModel() = 
    inherit ViewModelBase()

    let EmptyProfile = CoreTypes.DefaultRunConfig

    let DesignMode = DesignerProperties.GetIsInDesignMode(new DependencyObject())

    let mutable selectedProfile = EmptyProfile

    do
        RegConfig.init() // reg config requires init to set hive root

    member x.Profiles = 
        new ObservableCollection<CoreTypes.RunConfig>(
            RegConfig.loadAll())
    
    member x.SelectedProfile 
        with get () = selectedProfile
        and set value = 
            selectedProfile <- value
            x.RaisePropertyChanged("SelectedProfile") 
            x.RaisePropertyChanged("ProfileAreaVisibility") 

    member x.ProfileAreaVisibility = 
        if  DesignMode || 
            selectedProfile.ProfileName <> EmptyProfile.ProfileName then
            Visibility.Visible
        else
            Visibility.Hidden
