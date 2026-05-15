namespace MMLaunch

open Avalonia.Controls
open Avalonia.Markup.Xaml

type ConfirmDialogViewModel() =
    inherit ViewModelBase()

    let mutable view: ConfirmDialog option = None
    let mutable confirmed = false
    let mutable displayText = ""
    let mutable checkboxText = ""
    let mutable checkboxChecked = false

    member x.View
        with get () = view
        and set value =
            view <- value

    member x.Text
        with get () = displayText
        and set value =
            displayText <- value
            x.RaisePropertyChanged "Text"

    member x.CheckBoxText
        with get () = checkboxText
        and set value =
            checkboxText <- value
            x.RaisePropertyChanged "CheckBoxText"
            x.RaisePropertyChanged "CheckBoxVisible"

    member x.CheckBoxVisible = checkboxText.Trim() <> ""

    member x.CheckboxChecked
        with get () = checkboxChecked
        and set (value: bool) =
            checkboxChecked <- value
            x.RaisePropertyChanged "CheckboxChecked"

    member x.Confirmed = confirmed

    member x.Cancel =
        new RelayCommand((fun _ -> true), (fun _ ->
            view |> Option.iter (fun v -> v.Close())))

    member x.Confirm =
        new RelayCommand((fun _ -> true), (fun _ ->
            confirmed <- true
            view |> Option.iter (fun v -> v.Close())))

and ConfirmDialog() as this =
    inherit Window()

    do
        AvaloniaXamlLoader.Load(this)
        let vm = ConfirmDialogViewModel()
        vm.View <- Some this
        this.DataContext <- vm

    member x.ViewModel = x.DataContext :?> ConfirmDialogViewModel
