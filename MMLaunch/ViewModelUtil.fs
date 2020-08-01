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
open System.ComponentModel
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input

open Microsoft.Win32

// When used with Exception as the fail type, this 
// encourages the idiom of using pattern matching to 
// handle errors, rather than require try blocks in random places.
type Result<'SuccessType,'FailType>  =
    Ok of 'SuccessType
    | Err of 'FailType
        
module ViewModelUtil =
    let DesignMode = DesignerProperties.GetIsInDesignMode(new DependencyObject())

    type RelayCommand (canExecute:(obj -> bool), action:(obj -> unit)) =
        let event = new DelegateEvent<EventHandler>()
        interface ICommand with
            [<CLIEvent>]
            member x.CanExecuteChanged = event.Publish
            member x.CanExecute arg = canExecute(arg)
            member x.Execute arg = action(arg)

    let alwaysExecutable (action:(obj -> unit)) = 
        new RelayCommand ((fun canExecute -> true), action)

    let pushDialog(msg:string) =
        MessageBox.Show(msg) |> ignore

    let pushOkCancelDialog(msg:string) =
        MessageBox.Show(msg, "Confirm", MessageBoxButton.YesNo)

    let pushSelectFileDialog(initialDir:string option,filter:string) =
        let dlg = new OpenFileDialog()

        match initialDir with
        | None -> ()
        | Some dir ->
            dlg.InitialDirectory <- dir

        dlg.Filter <- filter
        dlg.FilterIndex <- 0
        dlg.RestoreDirectory <- true

        let res = dlg.ShowDialog() 
        if res.HasValue && res.Value then
            Some (dlg.FileName)
        else
            None

