namespace MMLaunch

open System.IO

open Avalonia.Controls
open Avalonia.Markup.Xaml

open ModelMod

type PreferencesViewModel() =
    inherit ViewModelBase()

    let mutable docRoot = RegConfig.getDocRoot ()

    member x.DocRoot
        with get () = docRoot
        and set value =
            if not (Directory.Exists value) then
                ViewModelUtil.pushDialog (sprintf "Directory does not exist: %s" value)
            else
                docRoot <- value
                RegConfig.setDocRoot docRoot |> ignore
            x.RaisePropertyChanged "DocRoot"

    member x.Browse =
        ViewModelUtil.alwaysExecutable (fun _ ->
            let initial = if Directory.Exists docRoot then Some docRoot else None
            match ViewModelUtil.pushSelectFolderDialog initial with
            | Some path -> x.DocRoot <- path
            | None -> ())

type PreferencesWindow() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        this.DataContext <- PreferencesViewModel()
