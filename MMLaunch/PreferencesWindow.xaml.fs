// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

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
        