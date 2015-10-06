open System.IO
open System.Diagnostics

// The sole purpose of this app is to have a "ModelMod.exe" file that we can stick 
// in the root folder so that the user doesn't have to go into "Bin" and randomly
// click executables there.  Obviously an installer would be another way to 
// handle this, but installers are generally yucky (especially ones that 
// require effing elevated privileges) and I'm too lazy to make one.
[<EntryPoint>]
let main argv = 
    let search = [@"."; @".\Bin"]
    let target = "MMLaunch.exe"

    let found = search |> List.tryPick (fun p ->
        let path = Path.Combine(p,target)
        if File.Exists path then
            Some(path)
        else 
            None
    )

    match found with 
    | None -> ()
    | Some (p) ->
        let proc = new Process()
        proc.StartInfo.UseShellExecute <- false
        proc.StartInfo.FileName <- p
        proc.StartInfo.WorkingDirectory <- Path.GetDirectoryName p
        proc.Start() |> ignore
    0 
