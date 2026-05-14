namespace MMLaunch

open System

open Avalonia.Controls
open Avalonia.Markup.Xaml

open ModelMod.CoreTypes

type GameProfileViewModel() =
    inherit ViewModelBase()

    let mutable profile = {
        GameProfile.ReverseNormals = false
        UpdateTangentSpace = true
        CommandLineArguments = ""
        DataPathName = ""
    }

    let mutable profileChangedCb: GameProfile -> unit = ignore

    let updateProfile newProfile =
        profile <- newProfile
        profileChangedCb profile

    member x.Profile
        with get () = profile
        and set value =
            profile <- value
            x.RaisePropertyChanged String.Empty

    member x.ProfileChangedCb
        with get () = profileChangedCb
        and set value = profileChangedCb <- value

    member x.ReverseNormals
        with get () = profile.ReverseNormals
        and set (value: bool) = updateProfile { profile with ReverseNormals = value }

    member x.UpdateTangentSpace
        with get () = profile.UpdateTangentSpace
        and set (value: bool) = updateProfile { profile with UpdateTangentSpace = value }

    member x.CommandLineArguments
        with get () = profile.CommandLineArguments
        and set (value: string) = updateProfile { profile with CommandLineArguments = value }

    member x.DataPathName
        with get () = profile.DataPathName
        and set (value: string) = updateProfile { profile with DataPathName = value }

type GameProfileWindow() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        this.DataContext <- GameProfileViewModel()
