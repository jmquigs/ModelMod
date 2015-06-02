namespace ModelMod

open System.Text.RegularExpressions
open System.Diagnostics
open System

module REUtil =
    let private reLog = Logging.getLogger("Regex")

    let checkGroupMatch pattern count str  = 
        let m = Regex.Match(str,pattern)
        if m.Success && m.Groups.Count = count then
            Some(m.Groups)
        else
            None

    let extract start xFn (groups:GroupCollection option)  =
        match groups with 
            | None -> None
            | Some groups -> 
                let tryExtract v =
                    try
                        let ret = xFn v
                        (Some ret,None)
                    with 
                        | ex -> 
                            let err = sprintf "Failed to extract value %s from groups[len %d]: %s" v groups.Count ex.Message
                            (None,Some err)

                let endI = groups.Count - 1
                let res = [ 
                    for i in [start .. endI] do
                        let res = tryExtract (groups.[i].Value.Trim())
                        match res with
                        | (Some ret,None) -> yield ret
                        | (None,Some err) -> reLog.Error "%s" err; ()
                        | _ -> failwith "unexpected error"
                ]
                let expectedLen = groups.Count - start
                //printfn "extracted %d, expected %d" (res.Length) expectedLen
                if expectedLen <> res.Length then
                    reLog.Error "Failed to extract one or more matches from group: %s" groups.[0].Value
                    None
                else
                    Some (List.toArray res)

module Util =
    let replaceSpaceWithUnderscore (s:string) = s.Replace(' ', '_')

    let replaceUnderscoreWithSpace (s:string) = s.Replace('_', ' ')

    let private swEnabled = true

    type StopwatchTracker(name) = 
        let sw = new Stopwatch()
        do sw.Start()

        let log = Logging.getLogger("SW:" + name)

        member x.SW = sw
        member x.Name = name
        member x.StopAndPrint() = 
            if swEnabled && sw.IsRunning then 
                sw.Stop()
                log.Info "finished: %dms" sw.ElapsedMilliseconds

        interface System.IDisposable with
            member x.Dispose() = x.StopAndPrint()

    let reportMemoryUsage() =
        // log memory statistics
        let log = Logging.getLogger("Util")
        let manangedMemory = float32 (GC.GetTotalMemory(true)) / 1024.f / 1024.f
        let proc = Process.GetCurrentProcess();
        let procMemMB = float32 proc.PrivateMemorySize64 / 1024.f / 1024.f
        log.Info "Memory: (clr: %3.2fMB; process: %3.2f MB)" manangedMemory procMemMB  