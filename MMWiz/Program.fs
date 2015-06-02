// Learn more about F# at http://fsharp.net
// See the 'F# Tutorial' project for more help.

open System

[<EntryPoint>]
[<STAThread>]
let main argv = 
    ModelMod.RegConfig.init()

    System.Windows.Forms.Application.ApplicationExit.Add (fun evArgs -> 
        MMWiz.Wizapp.terminatePreviewProcess())
    System.Windows.Forms.Application.Run( MMWiz.Wizapp.initMainScreen() )
    0 // return an integer exit code
