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
open System.ComponentModel
open System.Collections.ObjectModel
open Microsoft.Win32

open System.Windows.Forms // just for FolderBrowserDialog !
open FsXaml

open ViewModelUtil
open ModelMod

type PreferencesView = XAML<"PreferencesWindow.xaml", true>

type PreferencesViewModel() = 
    inherit ViewModelBase()

    let mutable docRoot = RegConfig.getDocRoot()

    member x.DocRoot 
        with get() = docRoot
        and set value = 
            if not (Directory.Exists value) then
                ViewModelUtil.pushDialog (sprintf "Directory does not exist: %s" value)
            else
                docRoot <- value
                RegConfig.setDocRoot docRoot |> ignore
            x.RaisePropertyChanged("DocRoot")

    member x.Browse = alwaysExecutable (fun action ->
        use fb = new FolderBrowserDialog()

        match fb.ShowDialog() with
        | DialogResult.OK ->
            if Directory.Exists fb.SelectedPath then
                x.DocRoot <- fb.SelectedPath
        | _ -> ()
        )
        