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

    let mutable private _loggerFactory = ConsoleLoggerFactory

    let private _loggers = new Dictionary<string, ILog>()   

    let setLoggerFactory(f:LoggerFactory) =
        _loggers.Clear()
        _loggerFactory <- f

    let makeLogger x = _loggerFactory x
    
    let getLogger(category) = 
        let ok, logger = _loggers.TryGetValue(category)
        let logger = 
            if ok then logger 
            else
                let category,logger = makeLogger(category)
                // TODO: why do I keep getting "key already exists", app domain issue?
                //_loggers.Add(category,logger) 
                logger
        logger  