module ModelMod.MMTricks.Util

let errorExit (code:int) (msg:string) : 'a =
    if code = 0 then
        printfn "%s" msg
    else
        eprintfn "%s" msg
    exit code