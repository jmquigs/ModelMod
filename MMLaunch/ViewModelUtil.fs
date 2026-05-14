// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley
//
// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

namespace MMLaunch

open System
open System.Collections.Generic
open System.Threading.Tasks

open Avalonia
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Platform.Storage
open Avalonia.Threading

// When used with Exception as the fail type, this
// encourages the idiom of using pattern matching to
// handle errors, rather than require try blocks in random places.
type Result<'SuccessType, 'FailType> =
    | Ok of 'SuccessType
    | Err of 'FailType

/// Avalonia replacement for WPF's MessageBoxResult enum. The launcher only
/// distinguishes "Yes" from anything else.
type DialogResult =
    | Yes
    | No
    | Cancel

module ViewModelUtil =
    /// Avalonia has no global "design mode" flag like WPF's DesignerProperties;
    /// the design-time XAML previewer sets Design.IsDesignMode on the view, but
    /// our view-models construct themselves at runtime so this is always false.
    let DesignMode = false

    let alwaysExecutable (action: obj -> unit) =
        new RelayCommand((fun _ -> true), action)

    // ----- Window discovery helpers -----------------------------------------

    /// Returns the first open window from the classic desktop lifetime that can
    /// be used as a dialog owner, or None if no windows are open yet.
    let private firstOpenWindow () : Window option =
        match Application.Current with
        | null -> None
        | app ->
            match app.ApplicationLifetime with
            | :? IClassicDesktopStyleApplicationLifetime as desktop ->
                let windows = desktop.Windows
                if windows = null || windows.Count = 0 then
                    match desktop.MainWindow with
                    | null -> None
                    | mw -> Some mw
                else
                    Some windows.[0]
            | _ -> None

    /// Coerce a CommandParameter (which is sometimes a Window, sometimes null
    /// in design mode) into a Window option. Falls back to whatever window is
    /// currently open if the parameter is null.
    let asWindow (param: obj) : Window option =
        match param with
        | :? Window as w -> Some w
        | _ -> firstOpenWindow ()

    // ----- Dialogs (simple modal Window built on the fly) -------------------

    /// Synchronously pump the Avalonia dispatcher until `task` completes.
    /// Used so the existing synchronous command code paths can call into the
    /// async Avalonia APIs (StorageProvider, ShowDialog) without rewriting all
    /// the call sites.
    let private waitFor<'T> (task: Task<'T>) : 'T =
        Dispatcher.UIThread.RunJobs()
        let frame = DispatcherFrame()
        task.ContinueWith(fun (_: Task<'T>) -> frame.Continue <- false)
        |> ignore
        Dispatcher.UIThread.PushFrame(frame)
        task.Result

    let private waitForVoid (task: Task) : unit =
        let frame = DispatcherFrame()
        task.ContinueWith(fun (_: Task) -> frame.Continue <- false)
        |> ignore
        Dispatcher.UIThread.PushFrame(frame)

    let private showMessageDialog (title: string) (msg: string) (buttons: (string * DialogResult) list) : DialogResult =
        let owner = firstOpenWindow ()

        let mutable result = DialogResult.Cancel

        let win = Window()
        win.Title <- title
        win.Width <- 460.0
        win.SizeToContent <- SizeToContent.Height
        win.CanResize <- false
        win.WindowStartupLocation <- WindowStartupLocation.CenterOwner
        match owner with
        | Some w -> win.Icon <- w.Icon
        | None -> ()

        let panel = StackPanel(Margin = Thickness(20.0), Spacing = 12.0)

        let text = TextBlock(Text = msg, TextWrapping = TextWrapping.Wrap)
        panel.Children.Add(text)

        let btnRow = StackPanel(Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Spacing = 8.0)
        for (label, value) in buttons do
            let b = Button(Content = label, MinWidth = 80.0)
            b.Click.Add(fun _ ->
                result <- value
                win.Close())
            btnRow.Children.Add(b)
        panel.Children.Add(btnRow)

        win.Content <- panel

        match owner with
        | Some o ->
            let task = win.ShowDialog(o)
            waitForVoid task
        | None ->
            // No owner means we're being called before the main window is shown
            // (e.g. very early startup). Nothing else can be on screen, so just
            // skip the dialog rather than crash.
            result <- DialogResult.Cancel

        result

    let pushDialog (msg: string) =
        showMessageDialog "ModelMod" msg [ ("OK", DialogResult.Yes) ] |> ignore

    let pushOkCancelDialog (msg: string) : DialogResult =
        showMessageDialog "Confirm" msg [ ("Yes", DialogResult.Yes); ("No", DialogResult.No) ]

    // ----- File / folder pickers --------------------------------------------

    let private exeFileTypes =
        let ft = FilePickerFileType("Executable files")
        ft.Patterns <- [| "*.exe" |]
        ft

    let private mmObjFileTypes =
        let ft = FilePickerFileType("MMObj files")
        ft.Patterns <- [| "*.mmobj" |]
        ft

    let private parseFilter (filter: string) : FilePickerFileType[] =
        // WPF filter strings are pipe-separated "Description|*.ext" pairs.
        // Avalonia uses FilePickerFileType objects; convert.
        let parts = filter.Split('|')
        let pairs =
            [| for i in 0 .. 2 .. parts.Length - 2 ->
                let desc = parts.[i]
                let pats = parts.[i + 1].Split(';') |> Array.map (fun s -> s.Trim())
                let ft = FilePickerFileType(desc)
                ft.Patterns <- pats
                ft |]
        if pairs.Length = 0 then [| FilePickerFileType("All files", Patterns = [| "*" |]) |] else pairs

    let pushSelectFileDialog (initialDir: string option, filter: string) : string option =
        match firstOpenWindow () with
        | None -> None
        | Some owner ->
            let topLevel = TopLevel.GetTopLevel(owner)
            if isNull topLevel then None
            else
                let opts = FilePickerOpenOptions()
                opts.AllowMultiple <- false
                opts.FileTypeFilter <- parseFilter filter

                match initialDir with
                | Some dir when System.IO.Directory.Exists dir ->
                    let folderTask = topLevel.StorageProvider.TryGetFolderFromPathAsync(Uri(dir))
                    let folder = waitFor folderTask
                    if not (isNull folder) then opts.SuggestedStartLocation <- folder
                | _ -> ()

                let task = topLevel.StorageProvider.OpenFilePickerAsync(opts)
                let files = waitFor task
                if files = null || files.Count = 0 then None
                else
                    let f = files.[0]
                    let path = f.TryGetLocalPath()
                    if String.IsNullOrEmpty path then None else Some path

    let pushSelectFolderDialog (initialDir: string option) : string option =
        match firstOpenWindow () with
        | None -> None
        | Some owner ->
            let topLevel = TopLevel.GetTopLevel(owner)
            if isNull topLevel then None
            else
                let opts = FolderPickerOpenOptions()
                opts.AllowMultiple <- false

                match initialDir with
                | Some dir when System.IO.Directory.Exists dir ->
                    let folderTask = topLevel.StorageProvider.TryGetFolderFromPathAsync(Uri(dir))
                    let folder = waitFor folderTask
                    if not (isNull folder) then opts.SuggestedStartLocation <- folder
                | _ -> ()

                let task = topLevel.StorageProvider.OpenFolderPickerAsync(opts)
                let folders = waitFor task
                if folders = null || folders.Count = 0 then None
                else
                    let f = folders.[0]
                    let path = f.TryGetLocalPath()
                    if String.IsNullOrEmpty path then None else Some path
