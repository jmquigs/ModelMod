namespace MMLaunch

open System
open System.IO
open System.Diagnostics

open Microsoft.Win32

module BlenderUtil =
    let SubKey = @"SOFTWARE\BlenderFoundation"

    let UnknownVersion = "<unknown>"

    let ModName = "io_scene_mmobj"

    let SourceScriptDir = Path.Combine("BlenderScripts",ModName)

    let PythonSetupScript = @"BlenderScripts\install.py"

    let PySuccessLine = "MMINFO: Plugin enabled"
    let PyFailLine = "MMERROR: "

    // http://www.fssnip.net/raw/gO
    let rec directoryCopy srcPath dstPath copySubDirs =

        if not <| System.IO.Directory.Exists(srcPath) then
            let msg = System.String.Format("Source directory does not exist or could not be found: {0}", srcPath)
            raise (System.IO.DirectoryNotFoundException(msg))

        if not <| System.IO.Directory.Exists(dstPath) then
            System.IO.Directory.CreateDirectory(dstPath) |> ignore

        let srcDir = new System.IO.DirectoryInfo(srcPath)

        for file in srcDir.GetFiles() do
            let temppath = System.IO.Path.Combine(dstPath, file.Name)
            file.CopyTo(temppath, true) |> ignore

        if copySubDirs then
            for subdir in srcDir.GetDirectories() do
                let dstSubDir = System.IO.Path.Combine(dstPath, subdir.Name)
                directoryCopy subdir.FullName dstSubDir copySubDirs

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
                let ver = queryKey view "ShortVersion" UnknownVersion
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

    let private getAddonsPath(blenderVer:string) = 
        let blenderVer = blenderVer.Trim()
        if blenderVer = "" then failwith "empty blender version"

        // we want to avoid installing directly in the blender directory because that probably requires admin privs.
        // going to try the easy way which is to just use the version string to construct the appdata addon path.
        // if this causes problems, though, may need to ultimately exec blender to get the actual path list and then 
        // reconstruct the appdata path from that (see "paths" command in install.py).  Note that blender won't 
        // include the appdata addon path in the list it reports if the path doesn't exist.

        let appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData)
        let addOns = Path.Combine(appData,(sprintf @"Blender Foundation\Blender\%s\scripts\addons" blenderVer))
        addOns       

    let installMMScripts(blenderVer:string):Result<string,string> =
        try
            let srcDir,pySetup = 
                let lp = ProcessUtil.getLoaderPath()
                let root = Path.Combine(Path.GetDirectoryName(lp), "..")

                Path.GetFullPath(Path.Combine(root,SourceScriptDir)), Path.GetFullPath(Path.Combine(root,PythonSetupScript))

            if not (Directory.Exists srcDir) then
                failwith "Source script directory does not exist: %s" srcDir
            if not (File.Exists pySetup) then
                failwith "Python setup script does not exist: %s" pySetup

            let found = findInstallPath()
            let idir,ver = 
                match found with
                | None -> failwith "Blender not found"
                | Some stuff -> stuff
            if ver = UnknownVersion then
                failwith "Unknown blender version"

            let exe = getExe idir

            if not (File.Exists exe) then
                failwith "Can't find blender executable"

            let addons = getAddonsPath(ver)

            // the directory may not exist yet
            if not (Directory.Exists addons) then
                Directory.CreateDirectory addons |> ignore

            // remove previous scripts
            let dest = Path.Combine(addons,ModName)
            if (Directory.Exists dest) then
                Directory.Delete(dest,true)

            // copy new scripts
            directoryCopy srcDir dest true

            // run the python script to make sure they are registered with blender
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- false 
            proc.StartInfo.FileName <- exe
            proc.StartInfo.Arguments <- sprintf "--background --python \"%s\" -- install" pySetup
            proc.StartInfo.RedirectStandardOutput <- true
            proc.StartInfo.RedirectStandardError <- true
            proc.Start() |> ignore
            proc.WaitForExit()
            let rawOut = proc.StandardOutput.ReadToEnd()
            let rawErr = proc.StandardError.ReadToEnd()

            let outLines = rawOut.Split([| "\n"; "\r\n" |], StringSplitOptions.None)

            let failed = outLines |> Array.tryFind (fun line -> line.StartsWith(PyFailLine))

            let rawMsg = sprintf "\n\nStdout:\n%s\n\nStderr:\n%s" rawOut rawErr

            match failed with
            | Some (line) ->
                let msg = line.Substring(PyFailLine.Length)
                failwithf "Error: scripts were installed in \n'%s'\nBut blender failed to register due to this error: %s%s" dest msg rawMsg
            | None -> 
                // make sure we got the "success" line
                let success = outLines |> Array.tryFind (fun line -> line.StartsWith(PySuccessLine))
                match success with
                | None -> 
                    failwithf "Error: scripts were installed in \n'%s'\nBut blender registration may have failed.%s" dest rawMsg
                | Some (_) -> Ok(dest)
        with
            | e -> Err(e.Message)
        