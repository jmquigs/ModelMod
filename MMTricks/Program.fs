module ModelMod.MMTricks.Program

open System
open System.IO
open System.Threading.Tasks

open ModelMod

open ModelMod.MMTricks

[<EntryPoint>]
let main argv =
    try

        let subcommand = if argv.Length > 0 then argv.[0] else ""
        let subcommand = subcommand.ToLowerInvariant().Trim()
        match subcommand with
        | "cachemods" -> MMTricks.CacheMods.run argv
        | _ ->
            printfn "Unknown subcommand '%s'. Available subcommands: cachemods" subcommand
            1
    with
    | e ->
        eprintfn "FATAL: %s" e.Message
        eprintfn "%s" e.StackTrace
        1
