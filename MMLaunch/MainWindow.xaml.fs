﻿namespace MMLaunch

open System
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input

open FsXaml

open ViewModelUtils

type MainView = XAML<"MainWindow.xaml", true>


type MainViewModel() = 
    inherit ViewModelBase()    

    let clicks = ref 0
    //member x.Button_Click(sender:Object, e:RoutedEventArgs) = 
    //        printfn "whatever"

    member x.ClickCount 
        with get() = clicks.Value
        and set(v) = clicks.Value <- v

    member x.BtnCommand = alwaysExecutable (fun action -> 
        incr clicks
        x.RaisePropertyChanged(String.Empty))

        //MessageBox.Show(sprintf "You've clicked %d times, ready to stop?" clicks.Value) |> ignore)