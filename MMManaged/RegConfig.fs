namespace ModelMod

open System
open System.IO
open Microsoft.Win32

module RegKeys = 
    let DocRoot = "DocRoot"
    let ProfExePath = "ExePath"
    let ProfRunModeFull = "RunModeFull"
    let ProfSnapshotProfile = "SnapshotProfile"
    let ProfInputProfile = "InputProfile"

module RegConfig =
    let private log = Logging.GetLogger("RegConfig")

    // not using path.combine here; has some weird behavior: System.IO.Path.Combine(@"a",@"\b") -> "\b"
    // however, this doesn't handle the case where a or b has more than one consecutive \
    let (@@) (a:string) (b:string) = 
        let aEndsWithBS = a.EndsWith(@"\")
        let bStartsWithBS = b.StartsWith(@"\")
        
        match aEndsWithBS,bStartsWithBS with
        | true,true -> a + b.Substring(1) 
        | true,false 
        | false,true -> a + b
        | false,false -> a + @"\" + b

    let private mmHive = @"HKEY_CURRENT_USER"
    let private mmRoot = @"Software\ModelMod"
    let private mmHiveRoot = mmHive @@ mmRoot
    let private mmProfRoot = mmRoot @@ "Profiles"
    let private mmProfHiveRoot = mmHive @@ mmProfRoot
    let private mmProfHiveDefaultsKey = mmProfHiveRoot @@ "ProfileDefaults"

    let private Regget(key,value,def) =
        let res = Registry.GetValue(key, value, def)
        match res with
        | null -> def
        | _ -> res

    let GetProfiles() = 
        let profKey = Registry.CurrentUser.OpenSubKey(mmProfRoot)
        match profKey with 
        | null -> []
        | _ -> 
            let profiles = profKey.GetSubKeyNames()
            List.ofArray profiles
                    
    let Load (exePath:string):CoreTypes.RunConfig = 
        let exePath = exePath.Trim()

        let conf = 
            // Search all profiles for a subkey that has the exe as its ExePath
            let targetProfile = 
                let profiles = GetProfiles()
                profiles |> List.tryPick (fun pName -> 
                                let profRoot = mmProfHiveRoot @@ pName
                                let pExePath = Regget(profRoot, RegKeys.ProfExePath, "") :?> string |> (fun s -> s.Trim())
                                if pExePath <> "" && pExePath.Equals(exePath, StringComparison.InvariantCultureIgnoreCase) then 
                                    Some(profRoot)
                                else None)

            let dwordAsBool dw = 
                if (int dw > 0) then true else false

            let profPath = 
                match targetProfile with
                | None -> 
                    log.Info "No profile subkey located in %A for executable %A; using defaults" mmProfHiveRoot exePath
                    // if this defaults key is missing, then we just use the hardcoded defaults below
                    mmProfHiveDefaultsKey
                | Some profPath -> profPath

            { 
                CoreTypes.RunConfig.DocRoot = Regget(mmHiveRoot,RegKeys.DocRoot,"") :?> string
                CoreTypes.RunConfig.RunModeFull = dwordAsBool ( Regget(profPath,RegKeys.ProfRunModeFull, 1) :?> int )
                CoreTypes.RunConfig.InputProfile = Regget(profPath,RegKeys.ProfInputProfile, "") :?> string
                CoreTypes.RunConfig.SnapshotProfile = Regget(profPath,RegKeys.ProfSnapshotProfile, "") :?> string
            }

        conf        
