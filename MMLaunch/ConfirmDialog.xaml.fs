namespace MMLaunch

open System
open System.IO
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input
open System.ComponentModel
open System.Collections.ObjectModel
open System.Windows.Controls
open Microsoft.Win32

open FsXaml

open ViewModelUtil

type ConfirmDialogView = XAML<"ConfirmDialog.xaml", true>

type ConfirmDialogViewModel() = 
    inherit ViewModelBase()

    let mutable view:ConfirmDialogView option = None
    let mutable confirmed = false
    let mutable displayText = ""
    let mutable checkboxText = ""
    let mutable checkboxChecked = false

    member x.View  
        with get() = 
            match view with 
            | None -> null
            | Some view -> view
        and set value =
            view <- Some(value)

    member x.Text
        with get() = displayText
        and set value = displayText <- value; x.RaisePropertyChanged("Text")

    member x.CheckBoxText 
        with get() = checkboxText
        and set value = 
            checkboxText <- value
            x.RaisePropertyChanged("CheckBoxText")
            x.RaisePropertyChanged("CheckBoxVisibility")

    member x.CheckBoxVisibility 
        with get() = 
            if ViewModelUtil.DesignMode || checkboxText.Trim() <> "" then Visibility.Visible else Visibility.Hidden

    member x.CheckboxChecked 
        with get () = checkboxChecked
        and set (value:bool) = 
            checkboxChecked <- value
            x.RaisePropertyChanged("CheckboxChecked")

    member x.Confirmed 
        with get() = confirmed

    member x.Cancel = 
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> view |> Option.iter (fun v -> 
                v.Root.Close() )))

    member x.Confirm =
        new RelayCommand (
            (fun canExecute -> true), 
            (fun action -> view |> Option.iter (fun v -> 
                confirmed <- true
                v.Root.Close() )))
