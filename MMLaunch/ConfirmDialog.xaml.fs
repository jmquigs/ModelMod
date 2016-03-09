// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

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
