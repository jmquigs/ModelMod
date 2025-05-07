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

    type logOnceFn = (string -> unit)

    let logOnce(infoWarnOrError:int): logOnceFn = 
        let log = getLogger("LogOnce")
        let mutable logged = false
        fun msg -> 
            if not logged then 
                match infoWarnOrError with 
                | 0 -> log.Info "%s" msg
                | 1 -> log.Warn "%s" msg
                | _ -> log.Error "%s" msg
                logged <- true

    let mutable logOnceFns = new Dictionary<string, logOnceFn>()
    let getLogOnceFn(onceFnId:string,infoWarnOrError:int) = 
        let ok, ofn = logOnceFns.TryGetValue onceFnId
        if ok then 
            ofn 
        else 
            let ofn = logOnce(infoWarnOrError)
            logOnceFns.Add(onceFnId, ofn)
            ofn
