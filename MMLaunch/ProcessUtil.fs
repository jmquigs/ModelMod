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

    let private LoaderSearchPath = ["."; 
        // Make a dev tree path in case it isn't found in current directory.
        // I used to have this always use Release in the dev build, but then I got bitten in the ass by the fact
        // that I was running this project in debug and thus the release MMManaged assembly wasn't getting updated.  
        // so now it uses release or debug as specified by config
#if DEBUG
        "../../../Debug" ;
#else
        "../../../Release" ;
#endif
    ]
    let private LoaderName = "MMLoader.exe"

    let private getLoaderPath() =
        let lp = 
            LoaderSearchPath 
            |> List.map (fun path -> Path.Combine(path, LoaderName))
            |> List.tryFind File.Exists
        match lp with 
        | None -> ""
        | Some (path) -> path
                
    // Returns the log directory
    let private getLogPath() =
        let lp = getLoaderPath()
        match lp with 
        | "" -> ""
        | path ->
            let loaderPar = Directory.GetParent(path);
            let logDir = Path.Combine(loaderPar.FullName, "..", "Logs")
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
            let logExeName = Path.GetFileName(exePath)
            Path.Combine(path, (sprintf "modelmod.%s.log" logExeName))
                    
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
                let lp = getLoaderPath()
                if not (File.Exists(lp))
                    then failwithf "Can't find %s; searched in %A" LoaderName LoaderSearchPath
                lp

            let proc = new Process()
            proc.StartInfo.Verb <- "runas"; // loader requires elevation for poll mode
            proc.StartInfo.UseShellExecute <- true // also required for elevation
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

    let openTextFile (filepath:string): Result<unit> =
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

    let openInjectionLog (exePath:string): Result<unit> =
        getInjectionLog exePath |> openTextFile

    let openModelModLog (exePath:string): Result<unit> =
        getModelModLog exePath |> openTextFile