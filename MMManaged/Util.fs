// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

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
                    // use a try here so that we can make sure the value text is included
                    // in the error message if the extraction fails
                    try 
                        let res = xFn (v) 
                        res
                    with 
                        | ex -> failwith "Illegal value: %A: %s" v ex.Message                    

                try 
                    let endI = groups.Count - 1

                    let res = [| 
                        for i in [start .. endI] do
                            let v = groups.[i].Value.Trim()
                            yield tryExtract v
                    |]
                    Some (res)
                with 
                    | ex -> 
                        reLog.Error "Failed to extract value from groups[len %d]: %s" groups.Count ex.Message
                        None

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