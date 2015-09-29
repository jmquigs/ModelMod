namespace MMLaunch

open System.IO

open Microsoft.Win32

module BlenderUtil =
    let SubKey = @"SOFTWARE\BlenderFoundation"

    let queryKey view name defVal = 
        try
            let key = RegistryKey.OpenBaseKey(Microsoft.Win32.RegistryHive.LocalMachine,view)
            if key = null then failwith "can't open reg key"

            let bKey = key.OpenSubKey SubKey
            if bKey = null then failwith "can't open blender key"

            let v = bKey.GetValue(name,defVal)
            if v = null then failwith "name not found"

            (v :?> string).Trim()
        with 
            | e -> 
                printfn "%A" e.Message
                defVal

    let getExe (idir:string) = Path.Combine(idir,"blender.exe")

    let findInstallPath():(string*string) option =        
        // prefer 64-bit
        let views = [RegistryView.Registry64; RegistryView.Registry32]
        let found = views |> List.tryPick (fun view ->
            let idir = queryKey view "Install_Dir" ""
            match idir with
            | "" -> None
            | path ->
                let ver = queryKey view "ShortVersion" "<unknown>"
                Some(idir,ver)
        )

        match found with
        | None -> None
        | Some(idir,ver) ->
            // make sure exe actually exists
            if not (File.Exists (getExe idir)) then
                None
            else
                found