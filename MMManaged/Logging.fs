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

namespace ModelMod

open System.Collections.Generic

module Logging =
    type ILog =
        abstract member Info : Printf.StringFormat<'a,unit> -> 'a
        abstract member Warn : Printf.StringFormat<'a,unit> -> 'a
        abstract member Error : Printf.StringFormat<'a,unit> -> 'a

    type CategoryName = string
    type NewCategoryName = string
    type LoggerFactory = CategoryName -> NewCategoryName * ILog

    let ConsoleLoggerFactory category =
        // based on
        // http://stackoverflow.com/questions/5277902/printf-style-logging-for-f?lq=1
        let formatInfo (result : string ) = printfn "info [%s]: %s" category result
        let formatWarn (result : string ) = printfn "warn [%s]: %s" category result
        let formatError (result : string ) = printfn "error [%s]: %s" category result

        category, { new ILog with
            member x.Info format = Printf.ksprintf (formatInfo) format
            member x.Warn format = Printf.ksprintf (formatWarn) format
            member x.Error format = Printf.ksprintf (formatError) format
        }

    let mutable private loggerFactory = ConsoleLoggerFactory

    let private loggers = new Dictionary<string, ILog>()

    let setLoggerFactory(f:LoggerFactory) =
        loggers.Clear()
        loggerFactory <- f

    /// Returns the current logger factory. Used by BulkLoader to pass the factory
    /// to the bulk implementation assembly so it can share the same logging setup.
    let currentLoggerFactory() = loggerFactory

    let makeLogger x = loggerFactory x

    let getLogger(category) =
        let ok, logger = loggers.TryGetValue(category)
        let logger =
            if ok then logger
            else
                // ignore the returned category; it may have changed, and we want to store it in the dict
                // using the input name so that we can reuse it for future uses of the same category.
                let _,logger = makeLogger(category)
                loggers.Add(category,logger)
                logger
        logger

    let getNullLogger() =
        { new ILog with
            member x.Info format = Printf.ksprintf (fun _ -> ()) format
            member x.Warn format = Printf.ksprintf (fun _ -> ()) format
            member x.Error format = Printf.ksprintf (fun _ -> ()) format
        }

    /// Log once functions take a thunk.  The message function is only invoked
    /// the first time the log fires, so callers in hot loops avoid paying
    /// sprintf/%A reflection cost on every call.
    type logOnceFn = ((unit -> string) -> unit)

    type private LogOnceEntry = {
        Fn: logOnceFn
        /// Whether the function has actually been invoked (logged its message).
        /// Shared ref cell so the closure and the dictionary entry see the same value.
        Called: bool ref
    }

    let private logOnceEntry(infoWarnOrError:int): LogOnceEntry =
        let log = getLogger("LogOnce")
        let called = ref false
        let fn = fun msgFn ->
            if not !called then
                let msg = msgFn()
                match infoWarnOrError with
                | 0 -> log.Info "%s" msg
                | 1 -> log.Warn "%s" msg
                | _ -> log.Error "%s" msg
                called := true
        { Fn = fn; Called = called }

    let logOnce(infoWarnOrError:int): logOnceFn =
        (logOnceEntry infoWarnOrError).Fn

    let mutable private logOnceFnEntries = new Dictionary<string, LogOnceEntry>()

    /// After a hot-reload, mark any logOnceFn entries that have already fired as
    /// eligible for reinit. The next call to getLogOnceFn for those IDs will
    /// create a fresh function. Entries that have never fired are left alone.
    let reinitLogOnceFns() =
        let toRemove =
            logOnceFnEntries
            |> Seq.filter (fun kv -> !(kv.Value.Called))
            |> Seq.map (fun kv -> kv.Key)
            |> Seq.toList
        for key in toRemove do
            logOnceFnEntries.Remove(key) |> ignore

    let getLogOnceFn(onceFnId:string,infoWarnOrError:int) =
        let ok, entry = logOnceFnEntries.TryGetValue onceFnId
        if ok then
            entry.Fn
        else
            let entry = logOnceEntry(infoWarnOrError)
            logOnceFnEntries.Add(onceFnId, entry)
            entry.Fn
