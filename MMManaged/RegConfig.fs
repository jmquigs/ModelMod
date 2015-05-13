#if COMPILED
namespace ModelMod
#endif

open Microsoft.Win32

open Types

module RegKeys = 
    let DocRoot = "DocRoot"
    let ProfRunModeFull = "RunModeFull"
    let ProfSnapshotProfile = "SnapshotProfile"
    let ProfInputProfile = "InputProfile"

module RegConfig =
    let private log = Logging.GetLogger("RegConfig")

    let private mmRoot = @"HKEY_CURRENT_USER\Software\ModelMod"

    let private Regget(key,value,def) =
        let res = Registry.GetValue(key, value, def)
        match res with
        | null -> def
        | _ -> res

    let GetExeHash (exePath:string):string =
        string (exePath.ToUpperInvariant().GetHashCode())

    let Load (exePath:string):Types.RunConfig = 
        let exehash = GetExeHash exePath
        let profRoot = mmRoot + "\\Profiles\\" + exehash

        let dwordAsBool dw = 
            if (int dw > 0) then true else false

        log.Info "Searching for configuration data under key %A for executable %A" profRoot exePath

        { 
            RunConfig.DocRoot = Regget(mmRoot,RegKeys.DocRoot,"") :?> string
            RunModeFull = dwordAsBool ( Regget(profRoot,RegKeys.ProfRunModeFull, 1) :?> int )
            InputProfile = Regget(profRoot,RegKeys.ProfInputProfile, "") :?> string
            SnapshotProfile = Regget(profRoot,RegKeys.ProfSnapshotProfile, "") :?> string
        }


