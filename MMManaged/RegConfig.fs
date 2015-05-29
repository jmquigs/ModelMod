namespace ModelMod

open System
open System.IO
open Microsoft.Win32

open CoreTypes

module RegKeys = 
    let DocRoot = "DocRoot"
    let ProfExePath = "ExePath"
    let ProfRunModeFull = "RunModeFull"
    let ProfSnapshotProfile = "SnapshotProfile"
    let ProfInputProfile = "InputProfile"
      
module RegUtil = 
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

    let dwordAsBool dw = if (int dw > 0) then true else false
    let boolAsDword b = if (b) then 1 else 0

    let zeroPad (count:int) (s:string) =
        let numZeros = count - s.Length
        if numZeros <= 0 then 
            s
        else
            let sw = new StringWriter()
            let rec helper count = if count > 0 then sw.Write("0"); helper (count - 1) 
            helper numZeros
            sw.Write(s)
            sw.ToString()
        
module RegConfig =
    let private log = Logging.GetLogger("RegConfig")

    open RegUtil

    module private RegLocTypes = 
        type RegLoc(rootFn: unit -> string) = 
            member x.Root:string = rootFn()
            member x.Hive = Registry.CurrentUser
            member x.ProfileDefaultsKey = "ProfileDefaults"    
            member x.HiveRoot = x.Hive.Name + @"\" + rootFn()
            member x.ProfRoot = rootFn() + @"\" + "Profiles"

        let NormalRegLoc = new RegLoc(fun _ -> @"Software\ModelMod")
        let TestRegLoc = new RegLoc(fun _ -> @"Software\ModelModTEST")
        let FailsauceRegLoc = new RegLoc(fun _ -> failwith "root is not set") // must call either Init() or InitForTest()

    // mutable so that unit test can change it, via Init functions below
    let mutable private regLoc = RegLocTypes.FailsauceRegLoc

    let Init() = regLoc <- RegLocTypes.NormalRegLoc
    let InitForTest() = regLoc <- RegLocTypes.TestRegLoc

    let private Regget(key,value,def) =
        let res = Registry.GetValue(key, value, def)
        match res with
        | null -> def
        | _ -> res

    let GetProfiles() = 
        let profKey = regLoc.Hive.OpenSubKey(regLoc.ProfRoot)
        match profKey with 
        | null -> [||]
        | _ -> 
            let profiles = profKey.GetSubKeyNames()
            Array.sort profiles

    let FindProfile (exePath:string) = 
        let exePath = exePath.Trim()
        let profiles = GetProfiles() 
        profiles |> Array.tryPick (fun pName -> 
            let pBase = regLoc.ProfRoot @@ pName
            let profRoot = regLoc.Hive.Name @@ pBase
            let pExePath = Regget(profRoot, RegKeys.ProfExePath, "") :?> string |> (fun s -> s.Trim())
            if pExePath <> "" && pExePath.Equals(exePath, StringComparison.InvariantCultureIgnoreCase) then 
                Some(pBase) // exclude hive
            else None)

    // Fail with exception if write to specified key is not authorized.
    // The hardcoded string here is deliberate.
    let private checkRoot (key:string) (failMsg:string) =
        if not ((key.StartsWith(@"Software\ModelMod")) && (key.StartsWith(regLoc.Root))) then
            failwith failMsg

    let DeleteProfileValue (pKey:string) (valName:string) =
        checkRoot pKey (sprintf "Refusing to delete value from unauth key: %A" pKey)
        
        let key = regLoc.Hive.OpenSubKey(pKey,RegistryKeyPermissionCheck.ReadWriteSubTree)
        match key with
        | null -> ()
        | _ -> key.DeleteValue(valName,false)

    let DeleteProfileKey (pKey:string) = 
        checkRoot pKey (sprintf "Refusing to delete unauth key: %A" pKey)

        regLoc.Hive.DeleteSubKey pKey

    let SetProfileValue (pKey:string) valName value =
        checkRoot pKey (sprintf "Refusing to set unauth key value: %A" pKey)

        Registry.SetValue(regLoc.Hive.Name @@ pKey, valName, value);
        value

    let CreateNewProfile() = 
        let profiles = GetProfiles() 
      
        let pName = seq { 0..9999 } |> Seq.tryPick (fun i -> 
            let pname = "Profile" + zeroPad 4 (i.ToString())
            let idx = Array.BinarySearch(profiles, pname)
            match idx with
            | x when x < 0 -> Some(pname)
            | x -> None)
      
        let pName = 
            match pName with
            | None -> failwith "WHOA couldn't locate an unused profile!"
            | Some name -> regLoc.ProfRoot @@ name
      
        let key = regLoc.Hive.CreateSubKey(pName) 
        match key with 
        | null -> failwithf "Failed to create registry key: %A\%A" regLoc.Hive.Name pName
        | _ -> ()
        
        pName

    let SaveProfile (conf:RunConfig) =
        if not (File.Exists conf.ExePath) then
            failwithf "Exe path does not exist, cannot save profile: %A" conf.ExePath

        // already exist?
        let profKey = FindProfile conf.ExePath
        let profKey = 
            match profKey with 
            | Some key -> key
            | None -> CreateNewProfile()

        let profSave k v = SetProfileValue profKey k v

        // this is a syntactic trick to make sure I get a compiler error if I forget to save a field
        let _ = {
            CoreTypes.RunConfig.ExePath = profSave RegKeys.ProfExePath conf.ExePath 
            RunModeFull = profSave RegKeys.ProfRunModeFull (boolAsDword conf.RunModeFull) |> dwordAsBool
            InputProfile = profSave RegKeys.ProfInputProfile conf.InputProfile 
            SnapshotProfile = profSave RegKeys.ProfSnapshotProfile conf.SnapshotProfile 
            DocRoot = "" // custom doc root not yet supported
        }

        ()                
                    
    let Load (exePath:string):RunConfig = 
        let exePath = exePath.Trim()

        let conf = 
            // Search all profiles for a subkey that has the exe as its ExePath
            let targetProfile = FindProfile exePath

            let profPath = 
                match targetProfile with
                | None -> 
                    let pRoot = regLoc.Hive.Name @@ regLoc.ProfRoot
                    log.Info "No profile subkey located in %A for executable %A; using defaults" pRoot exePath
                    // if this defaults key is missing, then we just use the hardcoded defaults below
                    pRoot @@ regLoc.ProfileDefaultsKey
                | Some profPath -> regLoc.Hive.Name @@ profPath

            let mmHiveRoot = regLoc.HiveRoot

            { 
                CoreTypes.RunConfig.ExePath = exePath
                DocRoot = Regget(mmHiveRoot,RegKeys.DocRoot,DefaultRunConfig.DocRoot) :?> string
                RunModeFull = dwordAsBool ( Regget(profPath,RegKeys.ProfRunModeFull, (boolAsDword DefaultRunConfig.RunModeFull)) :?> int )
                InputProfile = Regget(profPath,RegKeys.ProfInputProfile, DefaultRunConfig.InputProfile) :?> string
                SnapshotProfile = Regget(profPath,RegKeys.ProfSnapshotProfile, DefaultRunConfig.SnapshotProfile) :?> string
            }

        conf        
