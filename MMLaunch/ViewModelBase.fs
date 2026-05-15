// Hand-rolled MVVM primitives for the launcher: an INotifyPropertyChanged base
// and an ICommand impl. The original WPF launcher used FSharp.ViewModule for
// these; FSharp.ViewModule is WPF-only, so we replace it with this ~50 line
// equivalent rather than take a full dependency on ReactiveUI.

namespace MMLaunch

open System
open System.ComponentModel
open System.Windows.Input

[<AllowNullLiteral>]
type ViewModelBase() =
    let propertyChanged = Event<PropertyChangedEventHandler, PropertyChangedEventArgs>()

    interface INotifyPropertyChanged with
        [<CLIEvent>]
        member _.PropertyChanged = propertyChanged.Publish

    member x.RaisePropertyChanged(propertyName: string) =
        propertyChanged.Trigger(x, PropertyChangedEventArgs(propertyName))

type RelayCommand(canExecute: obj -> bool, action: obj -> unit) =
    let canExecuteChanged = Event<EventHandler, EventArgs>()

    member _.RaiseCanExecuteChanged() =
        canExecuteChanged.Trigger(null, EventArgs.Empty)

    interface ICommand with
        [<CLIEvent>]
        member _.CanExecuteChanged = canExecuteChanged.Publish
        member _.CanExecute(arg) = canExecute arg
        member _.Execute(arg) = action arg
