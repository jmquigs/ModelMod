// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

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

/// Regexp utilities, mostly used during .mmobj file load (see MeshUtil).
module REUtil =
    let private reLog = Logging.getLogger("Regex")

    /// Checks that a regex matches the specified pattern and subgroup count; if so returns the groups, if not,
    /// returns None.  Note that the group count is one higher than the number of apparent groups in the regexp, 
    /// because there is an implicit first group that matches everything on success.
    /// e.g. "baz(.*)foo(.*)bar" = 3 expected groups
    let checkGroupMatch pattern count str  = 
        let m = Regex.Match(str,pattern)
        if m.Success && m.Groups.Count = count then
            Some(m.Groups)
        else
            None

    /// Extract values from a list of groups, starting at the specified index and continuing to the end of the
    /// groups.  Uses the specified extraction function to 
    /// transform each group value.  Returns an array of all the extract values.
    /// If any value fails to extract, returns None.  Also returns None if the starting index is >= the
    /// number of groups.
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
                        | ex -> failwithf "Illegal value: %A: %s" v ex.Message

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

/// General utilities
module Util =
    let replaceSpaceWithUnderscore (s:string) = s.Replace(' ', '_')

    let replaceUnderscoreWithSpace (s:string) = s.Replace('_', ' ')

    let private swEnabled = true

    /// Use for basic timing measurements.  If you create it with a "use" statement, it will print the 
    /// elapsed time to the log when it goes out of scope.  Otherwise you can manually print the elapsed time
    /// with StopAndPrint().
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

    /// Report the memory usage of the current process.  Reports both the CLR memory and
    /// total process memory (which includes CLR memory).
    let reportMemoryUsage() =
        // log memory statistics
        let log = Logging.getLogger("Util")
        let manangedMemory = float32 (GC.GetTotalMemory(true)) / 1024.f / 1024.f
        let proc = Process.GetCurrentProcess();
        let procMemMB = float32 proc.PrivateMemorySize64 / 1024.f / 1024.f
        log.Info "Memory: (clr: %3.2fMB; process: %3.2f MB)" manangedMemory procMemMB

// Source: https://gist.github.com/haf/8140280
module CRC32 = 
  let IEEE = 0xedb88320u

  /// The seed value default: all ones, CRC depends fully on its input.
  let seed = 0xffffffffu

  let inline (!!) v = v ^^^ 0xFFFFFFFFu

  let crc_table = Array.init 256 (fun i ->
    (uint32 i, [0..7])
    ||> List.fold (fun value _ ->
      match value &&& 1u with
      | 0u -> value >>> 1
      | _  -> (value >>> 1) ^^^ IEEE))

  let step state buffer =
    (state, buffer)
    ||> Array.fold (fun crc byt -> crc_table.[int(byt ^^^ byte crc)] ^^^ (crc >>> 8))

  let finalise (state : uint32) : byte [] =
    !! state |> BitConverter.GetBytes

  let single_step = finalise << step seed

  let toU32 (x:byte[]) = BitConverter.ToUInt32(x,0)
