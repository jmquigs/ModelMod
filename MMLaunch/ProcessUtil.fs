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
open System.Threading
open System.Runtime.InteropServices

open KnownGames

module ProcessUtil =
    let LoaderExitReasons =
        Map.ofList
            [( 0, "Success");
             (-1, "Injection error");
             (-2, "Wait period expired"); // shouldn't see this; should be translated into a more specific code
             (-3, "Target process not found");
             (-4, "Some injection attempts failed")
             (-5, "Could not create mutex, another instance of target may be running")
            ]

    let LoaderName = "MMLoader.exe"

    let mutable lastRoot:string option = None

    let getMMRoot() =
        // MMRoot by convention its where one of the files/directories below lives.
        // This value is also stored in the registry (PeriodicUpdate puts it there)
        // so that the native code can find this directory 
        // and its associated files.

        let rootSearchPath = ["."; "..";
            // Make a dev tree path in case it isn't found in current directory.
            "../../.." ;
        ]
        let rootFiles = [
            "MMDotNet.sln"; // for dev runs
            "TPLib"
            "Logs"
            "Bin"]

        let root =
            rootSearchPath
            |> List.tryPick (fun rootpath ->
                rootFiles
                |> List.map (fun filepath -> Path.Combine(rootpath, filepath))
                |> List.tryPick (fun filepath ->
                    //printfn "try %A" (Path.GetFullPath(filepath))
                    if (Directory.Exists(filepath) || File.Exists(filepath)) then Some(filepath) else None)
            )
        
        let fullRoot = 
            match root with
            | None -> failwithf "Unable to find MM root from working dir %A" (System.Environment.CurrentDirectory)
            | Some(dir) -> Path.GetFullPath(Path.GetDirectoryName(dir))

        if lastRoot <> Some(fullRoot) then 
            lastRoot <- Some(fullRoot)
            printfn "MM root changed: %A" root
        fullRoot

    /// Loader isn't used anymore so this is just a placeholder until I remove the related code.
    let getLoaderPath() =
        Path.Combine([|getMMRoot(); "Bin"; LoaderName |])

    // Returns the log directory
    let private getLogPath() =
        let root = getMMRoot()
        let logDir = Path.Combine(root, "Logs")
        logDir

    let private getInjectionLog (exePath:string) =
        let lp = getLogPath()
        match lp with
        | "" -> ""
        | path ->
            let logExeName = Path.GetFileName(exePath)
            Path.Combine(path, (sprintf "mmloader.%s.log" logExeName))

    let private getModelModLog (exePath:string) =
        let lp = getLogPath()
        match lp with
        | "" -> ""
        | path ->
            let logExeName = Path.GetFileNameWithoutExtension(exePath)
            Path.Combine(path, (sprintf "modelmod.%s.log" logExeName))

    let getLoaderExitReason (proc:Process) (defReason:string) =
        if not proc.HasExited then
            "Proc has not exited"
        else
            match LoaderExitReasons |> Map.tryFind proc.ExitCode with
            | None -> defReason
            | Some (reason) -> reason

    let launch (exePath):Result<Process,System.Exception> =
        try
            let proc = new Process()
            proc.StartInfo.FileName <- exePath
            proc.StartInfo.WorkingDirectory <- Path.GetDirectoryName(exePath)
            if proc.Start() then
                Ok(proc)
            else
                failwithf "failed to start proc: %A" exePath
        with
        | e -> Err(e)

    let launchWithLoader (exePath:string) (args:string) (waitperiod:int) :Result<Process,System.Exception> =
        try
            if not (File.Exists(exePath)) then
                failwithf "Exe does not exist: %s" exePath
            // crude, but if it isn't an exe, we probably can't inject it because the loader won't find
            // it.  and .com files aren't supported, ha ha
            if Path.GetExtension(exePath).ToLowerInvariant() <> ".exe" then
                failwithf "Exe does not appear to be an exe: %s" exePath
            // find loader
            let loaderPath =
                let lp = getLoaderPath()
                if not (File.Exists(lp))
                    then failwithf "Can't find %s" LoaderName
                lp

            let proc = new Process()
            // For now we assume game doesn't need to run as admin, therefore we don't need admin to inject.
            //proc.StartInfo.Verb <- "runas"; // required for elevation
            //proc.StartInfo.UseShellExecute <- true // also required for elevation
            proc.StartInfo.FileName <- loaderPath

            // hardcode log path to the same hardcoded path that ModelMod will use (which is relative
            // to the MMLoader.exe dir)
            let logfile =
                let logDir = getLogPath()

                if not (Directory.Exists logDir) then
                    Directory.CreateDirectory(logDir) |> ignore
                if not (Directory.Exists logDir) then
                    failwithf "Failed to create output log directory: %s" logDir
                getInjectionLog exePath

            // tell loader to exit if it hasn't attached in n seconds
            proc.StartInfo.Arguments <- sprintf "\"%s\" -waitperiod %d -logfile \"%s\"" exePath waitperiod logfile
            let res = proc.Start ()
            if not res then
                failwith "Failed to start loader process"

            let loaderProc = proc

            // pause for a bit to avoid loader's "found on first launch" heuristic;
            // this could fail if the system is really slow, and can't get loader up in time.
            // the whole injection process is rather racey and could use some improvement.
            // most of the races are internal to MMLoader.exe, however.
            Thread.Sleep(2000)

            // ok, loader is fired up, and by the time we get here the user has already accepted the elevation
            // dialog.  so launch the target exe; loader will find it and inject.  this also should handle the
            // case where the exe restarts itself because it needs to be launched from some parent process
            // (e.g. Steam)
            // we don't store a reference to the game process because we don't do anything with it at this point;
            // and anyway this process often isn't the one we'll ultimately inject due
            // to relaunches.

            // in theory loader could start the game too, but then it would start as admin, which we don't want.

            // make sure loader hasn't died while we slept
            if loaderProc.HasExited then
                failwithf "%s" (getLoaderExitReason loaderProc "Unknown")

            let proc = new Process()
            proc.StartInfo.UseShellExecute <- false
            proc.StartInfo.FileName <- exePath
            proc.StartInfo.Arguments <- args
            let res = proc.Start()
            if not res then
                // bummer, kill the loader
                loaderProc.Kill()
                loaderProc.WaitForExit()
                failwith "Failed to start game process"

            Ok(loaderProc)
        with
            | e ->
                Err(e)

    let openWebBrowser (url:string) =
        try
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- true
            proc.StartInfo.FileName <- url
            proc.Start() |> ignore
        with | e ->
            ()

    let openTextFile (filepath:string): Result<unit,System.Exception> =
        try
            if not (File.Exists(filepath)) then
                failwithf "Injection log not found: %s" filepath

            let proc = new Process()
            proc.StartInfo.UseShellExecute <- true
            proc.StartInfo.FileName <- filepath
            let res = proc.Start()
            if not res then
                failwith "Failed to open log file"
            Ok(())
        with
            | e ->
                Err(e)

    let openInjectionLog (exePath:string): Result<unit,System.Exception> =
        getInjectionLog exePath |> openTextFile

    let openModelModLog (exePath:string): Result<unit,System.Exception> =
        getModelModLog exePath |> openTextFile

    open System.IO
    open System.Reflection.PortableExecutable

    let getBitness (path: string) =
        use stream = File.OpenRead(path)
        use pe = new PEReader(stream)
        let header = pe.PEHeaders.CoffHeader

        match header.Machine with
        | Machine.I386  -> (32, "32-bit (x86)")
        | Machine.Amd64 -> (64, "64-bit (x64)")
        | Machine.Arm64 -> (64, "64-bit (ARM64)")
        | Machine.Arm   -> (32, "32-bit (ARM)")
        | Machine.IA64  -> (64, "64-bit (Itanium)")
        | m             -> (0, sprintf "unknown (%A)" m)

    type PreStartCopyResult =
    | Copied
    | UnknownExe

    /// If the exe is "known" (we know where to put d3d files) then copy those in and return
    /// the `Copied` value as a result.  Otherwise return `UnknownExe`.  If a problem happens
    /// return the exception.  This function also checks if any existing target d3d dlls
    /// belong to modelmod, if not, it does not overwrite them (and returns an exception).
    let preStartCopy(exePath:string): Result<PreStartCopyResult,System.Exception> =
        try
            let fileName = Path.GetFileNameWithoutExtension(exePath)
            let mmRoot = getMMRoot()
            let knownProfile = AllKnownGames |> List.tryFind (fun kg -> kg.ExeBaseName.ToLowerInvariant() = fileName.ToLowerInvariant())
            match knownProfile with
            | None -> Result.Ok(PreStartCopyResult.UnknownExe)
            | Some(prof) ->
                // if there are d3d dlls in the directory, verify that they belong to modelmod, if so
                // can overwrite them.  if not, fail
                let dirBase = Path.GetDirectoryName(exePath)

                let toCheck =
                    prof.D3DPaths
                    |> List.map (function
                        | D3D9(subdir) -> D3D9(Path.Combine(dirBase, subdir, "d3d9.dll"))
                        | D3D11(subdir) -> D3D11(Path.Combine(dirBase, subdir, "d3d11.dll")))

                let checkIsMM (targFile) =
                    if File.Exists(targFile) then
                        let fvi = FileVersionInfo.GetVersionInfo(targFile)
                        if fvi.FileDescription.Contains("ModelMod") then
                            printfn "%A is a ModelMod file" targFile
                        else
                            failwithf "Can't overwrite file that doesn't belong to modelmod: %A is %A" targFile fvi.FileDescription
                toCheck |> List.iter (function | D3D11(somefile) | D3D9(somefile) -> checkIsMM somefile)

                let (bitness,bitStr) = getBitness exePath
                if bitness = 0 then 
                    failwithf "Can't determine exe type for target: %A (%A)" exePath bitStr
                let is64 = (bitness = 64)
                printfn "target %A bitness: %A" exePath bitness

                // if we get here then one or both paths are ok to overwrite
                // source files could be in either of these dirs depending on where we are running from
                let bitDirs = 
                    [
                        if is64 then "modelmod_64" else "modelmod_32";
                        ""
                    ]

                // copy from working dir if we are running from there, otherwise use release package folders
                let workDirOut = @"Native\target\release"
                let dirs = [ workDirOut; "Bin"; "Release";]
                let dirs = bitDirs |> List.map (fun srcBits -> 
                        dirs |> List.map (fun d -> Path.Combine(mmRoot, d, srcBits)))
                        |> List.concat
                
                let sourceDir = dirs |> List.tryFind(fun d -> Directory.Exists(d))
                let sourceDir =
                    match sourceDir with
                    | None -> failwithf "Can't find source files for copy (root %A, looked in %A)" mmRoot dirs
                    | Some(sd) -> sd

                let copyD11 (outpath:string) =

                    let baseDllName = Path.GetFileName(outpath)
                    let src = Path.Combine(sourceDir, baseDllName)
                    let src = 
                        if not (File.Exists src) && src.ToLowerInvariant().Contains(workDirOut.ToLowerInvariant()) then 
                            
                            
                            let hookCore = Path.Combine(sourceDir, "hook_core.dll")
                            if File.Exists(hookCore) then 
                                // make sure bitness on hook core matches as it can be 32 or 64
                                let hcBitness = fst (getBitness hookCore)
                                if hcBitness <> bitness then 
                                    failwithf "built hook_core.dll %A (%d) does not match target bitness %A (%d)"
                                        hookCore hcBitness exePath  bitness 
                                else 
                                    hookCore
                            else 
                                failwithf "cannot find source file for copy: %A" src
                        else 
                            src

                    printfn "copy %A => %A" src outpath
                    File.Copy(src, outpath, true)

                toCheck |> List.iter (function | D3D9(outpath) | D3D11(outpath) -> copyD11 outpath)
                Ok(Copied)
        with
        | e -> Result.Err(e)



