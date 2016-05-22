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
open System.Windows.Input
open System.ComponentModel
open System.Collections.ObjectModel
open Microsoft.Win32
open FsXaml

open ViewModelUtil
open ModelMod
open ModelMod.CoreTypes

type GameProfileView = XAML<"GameProfileWindow.xaml", true>

type GameProfileViewModel() = 
    inherit ViewModelBase()

    // The viewmodel is also the actual Model for the GameProfile
    let mutable profile = { 
        GameProfile.ReverseNormals = false
        CommandLineArguments = ""
    }
    let mutable profileChangedCb: GameProfile -> unit = ignore

    let updateProfile newProfile = 
        profile <- newProfile
        profileChangedCb profile

    member x.Profile 
        with get() = profile
        and set value = 
            profile <- value
            x.RaisePropertyChanged(String.Empty)

    member x.ProfileChangedCb 
        with get() = profileChangedCb
        and set value = profileChangedCb <- value

    member x.ReverseNormals
        with get () = profile.ReverseNormals
        and set (value:bool) = updateProfile { profile with ReverseNormals = value }

    member x.CommandLineArguments
        with get () = profile.CommandLineArguments
        and set (value:string) = updateProfile { profile with CommandLineArguments = value}
