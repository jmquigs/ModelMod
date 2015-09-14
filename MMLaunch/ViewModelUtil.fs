namespace MMLaunch

open System
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input

open Microsoft.Win32

// This is a bit of weird Result type, but lets us focus on the idiom of using pattern matching to 
// handle errors, rather than require try blocks in random places
type Result<'T> = 
    Ok of 'T
    | Err of Exception
        
module ViewModelUtil =
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

