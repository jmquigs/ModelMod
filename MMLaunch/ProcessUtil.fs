namespace MMLaunch

open System.IO
open System.Diagnostics
open System.Threading

module ProcessUtil =
    let LoaderExitReasons = 
        Map.ofList
            [( 0, "Success");
             (-1, "Injection error");
             (-2, "Wait period expired"); // shouldn't see this; should be translated into a more specific code
             (-3, "Target process not found");
             (-4, "Some injection attempts failed")
            ]

    let launchWithLoader (exePath:string):Result<Process> =
        try 
            if not (File.Exists(exePath)) then
                failwithf "Exe does not exist: %s" exePath    
            // crude, but if it isn't an exe, we probably can't inject it because the loader won't find 
            // it.  and .com files aren't supported, ha ha
            if not (Path.GetExtension(exePath).ToLowerInvariant() = ".exe") then
                failwithf "Exe does not appear to be an exe: %s" exePath
            // find loader
            let loaderPath = 
                let lname = "MMLoader.exe"
                let devTreePath = Path.Combine("../../../Release", lname) // always use release version if running in a dev environment
                if File.Exists(lname) then Path.GetFullPath(lname)
                else if File.Exists(devTreePath) then Path.GetFullPath(devTreePath)
                else // dunno where it is
                    ""
            if not (File.Exists(loaderPath))
                then failwithf "Can't find %s" loaderPath

            let proc = new Process()
            proc.StartInfo.Verb <- "runas"; // loader requires elevation for poll mode
            proc.StartInfo.UseShellExecute <- true // also required for elevation
            proc.StartInfo.FileName <- loaderPath
            
            // hardcode log path to the same hardcoded path that ModelMod will use (which is relative 
            // to the MMLoader.exe dir)
            let logfile = 
                let loaderPar = Directory.GetParent(loaderPath);
                let logExeName = Path.GetFileName(exePath)
                let logDir = Path.Combine(loaderPar.FullName, "..", "Logs")
                if not (Directory.Exists logDir) then
                    Directory.CreateDirectory(logDir) |> ignore
                if not (Directory.Exists logDir) then
                    failwithf "Failed to create output log directory: %s" logDir
                Path.Combine(logDir , (sprintf "mmloader.%s.log" logExeName))

            // tell loader to exit if it hasn't attached in n seconds
            let waitperiod = 5
            proc.StartInfo.Arguments <- sprintf "\"%s\" -waitperiod %d -logfile \"%s\"" exePath waitperiod logfile
            let res = proc.Start ()
            if not res then 
                failwith "Failed to start loader process"

            let loaderProc = proc

            // pause for a bit to avoid loader's "found on first launch" heuristic;
            // this could fail if the system is really slow, though, and can't get loader up in time.
            // this is one of several race conditions here...this one still occasionally hits a CreateRemoteThread
            // problem.
            Thread.Sleep(2000)

            // ok, loader is fired up, and by the time we get here the user has already accepted the elevation
            // dialog...so launch the target exe; loader will find it and inject.  this also should handle the
            // case where the exe restarts itself because it needs to be launched from some parent process
            // (e.g. Steam)
            // in theory loader could start the game too, but then it would start as admin, which we don't want.
            
            let proc = new Process()
            proc.StartInfo.UseShellExecute <- false
            proc.StartInfo.FileName <- exePath
            let res = proc.Start()
            if not res then 
                // bummer, kill the loader
                loaderProc.Kill()
                loaderProc.WaitForExit()
                failwith "Failed to start game process"

            // we don't store a reference to the game process because we don't do anything with it at this point;
            // and anyway this process often isn't the one we'll ultimately inject - could be relaunched by itself
            // (auto-update) or another process (Steam).

            Ok(loaderProc)
        with 
            | e -> 
                Err(e)
