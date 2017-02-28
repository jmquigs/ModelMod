// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace MMLaunch

open System
open System.IO
open System.Diagnostics
open Microsoft.Win32

module BlenderUtil =
    let SubKey = @"SOFTWARE\BlenderFoundation"
    let UnknownVersion = "<unknown>"
    let ModName = "io_scene_mmobj"
    let BlenderExe = "blender.exe"
    let SourceScriptDir = Path.Combine("BlenderScripts",ModName)
    let PythonSetupScript = @"BlenderScripts\install.py"
    let PySuccessLine = "MMINFO: Plugin enabled"
    let PyFailLine = "MMERROR: "

    type ScriptStatus =
        NotFound
        | UpToDate
        | Diverged

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

    let getExe (idir:string) = Path.Combine(idir,BlenderExe)

    let detectBlenderInFS():string option = 
        // hardcode some obvious places
        let drives = ["C:"; "D:"]
        let paths = [
            @"\Program Files\Blender Foundation\Blender\blender.exe";
            @"\Program Files (x86)\Blender Foundation\Blender\blender.exe"]

        paths |> List.tryPick (fun p ->
            drives |> List.tryPick (fun d ->
                let fp = d + p
                if File.Exists(fp) then Some(Directory.GetParent(fp).ToString()) else None
        ))

    let detectInstallPath():string option = 
        // look in registry.  use 64-bit registry first
        let views = [RegistryView.Registry64; RegistryView.Registry32]
        let found = views |> List.tryPick (fun view ->
            let idir = queryKey view "Install_Dir" ""
            match idir with
            | "" -> 
                // as of 2.75 they don't seem to write Install_Dir anymore
                detectBlenderInFS()
            | path ->
                Some(idir)
        )

        match found with
        | None -> None
        | Some(idir) ->
            // make sure exe actually exists
            if not (File.Exists (getExe idir)) then
                None
            else
                found

    let private runBlender (exe:string) (cmd:string) = 
        if not (File.Exists exe) then
            failwithf "Can't find blender executable: %s" exe

        let pySetup = Path.GetFullPath(Path.Combine(ProcessUtil.getMMRoot(),PythonSetupScript))
        if not (File.Exists pySetup) then
            failwithf "Can't find setup script: %s" pySetup

        let proc = new Process()
        proc.StartInfo.UseShellExecute <- false 
        proc.StartInfo.FileName <- exe
        proc.StartInfo.Arguments <- sprintf "--background --python \"%s\" -- %s" pySetup cmd
        proc.StartInfo.RedirectStandardOutput <- true
        proc.StartInfo.RedirectStandardError <- true
        proc.Start() |> ignore
        proc.WaitForExit()
        let rawOut = proc.StandardOutput.ReadToEnd()
        let rawErr = proc.StandardError.ReadToEnd()
        rawOut,rawErr
        
    let getAddonsPath (exe:string):Result<string,string> =
        try
            // exec blender with install.py to get the addon paths.
            // if the appdata path is in the list, use it.
            // if its not in the list (probably because it doesn't exist or is empty), construct it from parts of the
            // install dir path (so that we get the correct version, etc), and return that.
            // (we need to use an appdata path, because otherwise we likely need admin privs to install)
            let rawOut,rawErr = runBlender exe "paths"

            let outLines = rawOut.Split([| "\n"; "\r\n" |], StringSplitOptions.None)

            let pathLine = "MMPATH:"
            let paths = 
                outLines 
                |> Array.filter (fun line -> line.StartsWith(pathLine))
                |> Array.map (fun line -> line.Substring(pathLine.Length).Trim())

            if paths.Length = 0 then
                let rawMsg = sprintf "\n\nStdout:\n%s\n\nStderr:\n%s" rawOut rawErr
                failwithf "No addon paths detected; install script may not be compatible with this version of blender:%s" rawMsg

            let appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData)
            let found = paths |> Array.tryFind (fun p -> p.ToLowerInvariant().StartsWith(appData.ToLowerInvariant()))
            match found with
            | Some p -> Ok(p)
            | None ->
                // rebuild a new appData path, using a relative part from one of the existing paths
                let relRoot = "Blender Foundation"
                let found = paths |> Array.tryFind (fun p -> p.Contains(relRoot))
                let path = 
                    match found with
                    | None -> 
                        let rawMsg = sprintf "\n\nStdout:\n%s\n\nStderr:\n%s" rawOut rawErr
                        failwithf "Unable to locate a suitable addon path:%s" rawMsg
                    | Some path -> path
                let path = path.Substring(path.IndexOf(relRoot))
                let path = Path.Combine(appData,path)
                Ok(path)
            with
        | e -> Err(e.Message)

    let getScriptSourceDir() = 
        let lp = ProcessUtil.getLoaderPath()
        let root = Path.Combine(Path.GetDirectoryName(lp), "..")

        Path.GetFullPath(Path.Combine(root,SourceScriptDir))

    let checkScriptStatus (currInstallDir:string):Result<ScriptStatus,string> = 
        try
            if not (Directory.Exists currInstallDir) then
                Ok(NotFound)
            else
                let srcDir = getScriptSourceDir()

                if not (Directory.Exists srcDir) then
                    failwithf "Source script directory does not exist: %s" srcDir

                let noPyCache (fn:string) = not (fn.Contains("__pycache__"))

                let srcFiles = Directory.GetFiles(srcDir,"*.*", SearchOption.AllDirectories) |> Array.filter noPyCache
                let currFiles = srcFiles |> Array.map (fun f -> f.Replace(srcDir,currInstallDir)) 

                let diffFound = 
                    Array.zip srcFiles currFiles 
                    |> Array.tryPick (fun (src,curr) ->
                        if not (File.Exists curr) then
                            Some(curr)
                        else
                            let srcData = File.ReadAllBytes(src)
                            let currData = File.ReadAllBytes(curr)
                            if srcData <> currData then
                                Some(curr)
                            else
                                None
                    )
                match diffFound with
                | None -> Ok(UpToDate)
                | Some diff -> Ok(Diverged)
        with
            | e -> Err(e.Message)

    let installMMScripts (exe:string):Result<string,string> =
        try
            let srcDir = getScriptSourceDir()

            if not (Directory.Exists srcDir) then
                failwithf "Source script directory does not exist: %s" srcDir

            if not (File.Exists exe) then
                failwith "Can't find blender executable"

            let addons = 
                match (getAddonsPath exe) with
                | Ok(path) -> path
                | Err(s) -> failwithf "%s" s

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
            let rawOut,rawErr = runBlender exe "install"

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
        