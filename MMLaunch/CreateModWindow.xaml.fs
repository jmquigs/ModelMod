namespace MMLaunch

open System
open System.Diagnostics
open System.Threading
open System.Windows
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

type CreateModView = XAML<"CreateModWindow.xaml", true>

type CreateModViewModel() = 
    inherit ViewModelBase()
