namespace MMLaunch

open System
open System.Windows
open FSharp.ViewModule
open FSharp.ViewModule.Validation
open System.Windows.Input

module ViewModelUtils =
    type RelayCommand (canExecute:(obj -> bool), action:(obj -> unit)) =
        let event = new DelegateEvent<EventHandler>()
        interface ICommand with
            [<CLIEvent>]
            member x.CanExecuteChanged = event.Publish
            member x.CanExecute arg = canExecute(arg)
            member x.Execute arg = action(arg)

    let alwaysExecutable (action:(obj -> unit)) = 
        new RelayCommand ((fun canExecute -> true), action)

