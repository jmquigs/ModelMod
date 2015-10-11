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

open System
open System.IO
open Microsoft.Win32

open CoreTypes

/// List of all the reg value names that we set (in HKCU/Software/ModelMod)
module RegKeys = 
    let LastSelectedBlender = "LastSelectedBlender"
    let LastScriptInstallDir = "LastScriptInstallDir"
    let RecycleSnapshots = "RecycleSnapshots"
    let DocRoot = "DocRoot"
    let ProfExePath = "ExePath"
    let ProfName = "ProfileName"
    let ProfRunModeFull = "RunModeFull"
    let ProfLoadModsOnStart = "LoadModsOnStart"
    let ProfSnapshotProfile = "SnapshotProfile"
    let ProfInputProfile = "InputProfile"
      
/// Various registry access utilities.
module RegUtil = 
    /// Registry path concatenator.
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

    /// Convert specified dword (integer) type to bool
    let dwordAsBool dw = (int dw > 0)
    /// Convert specified bool type to dword (integer)
    let boolAsDword b = if (b) then 1 else 0

    /// Zeropad the specified string up to count places.
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
        
/// Utilities for accessing ModelMod specific configuration data.
module RegConfig =
    let private log = Logging.getLogger("RegConfig")

    open RegUtil

    /// The registry unit tests use a different ModelMod root; this type
    /// allows the root to be changed to that specific whitelisted root.
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

    /// Initialize the registry root for normal use.
    /// This must be called prior to use, otherwise
    /// all registry functions will thrown an exception.
    let init() = regLoc <- RegLocTypes.NormalRegLoc
    /// Initialize the registry root for integration test use.
    let initForTest() = regLoc <- RegLocTypes.TestRegLoc

    let private regget(key,value,def) =
        let res = Registry.GetValue(key, value, def)
        match res with
        | null -> def
        | _ -> res

    /// Returns a list of of all the profile key names
    /// "Profile0000", "Profile0001", etc
    let getProfileKeyNames() = 
        let profKey = regLoc.Hive.OpenSubKey(regLoc.ProfRoot)
        match profKey with 
        | null -> [||]
        | _ -> 
            let profiles = profKey.GetSubKeyNames()
            Array.sort profiles

    /// Given an exe path, find its profile key path, or None if not found.
    /// Each exe path must therefore map to exactly one profile.
    let findProfilePath (exePath:string) = 
        let exePath = exePath.Trim()
        let profiles = getProfileKeyNames() 
        profiles |> Array.tryPick (fun pName -> 
            let pBase = regLoc.ProfRoot @@ pName
            let profRoot = regLoc.Hive.Name @@ pBase
            let pExePath = regget(profRoot, RegKeys.ProfExePath, "") :?> string |> (fun s -> s.Trim())
            if pExePath <> "" && pExePath.Equals(exePath, StringComparison.InvariantCultureIgnoreCase) then 
                Some(pBase) // exclude hive
            else None)

    /// Fail with exception if write to specified key is not authorized.
    /// The hardcoded string here is deliberate, so that we don't end up writing
    /// to willy-nilly places.
    let private checkRoot (key:string) (failMsg:string) =
        if not ((key.StartsWith(@"Software\ModelMod")) && (key.StartsWith(regLoc.Root))) then
            failwith failMsg

    /// Remove a value from the specified profile 
    let deleteProfileValue (pKey:string) (valName:string) =
        checkRoot pKey (sprintf "Refusing to delete value from unauth key: %A" pKey)
        
        let key = regLoc.Hive.OpenSubKey(pKey,RegistryKeyPermissionCheck.ReadWriteSubTree)
        match key with
        | null -> ()
        | _ -> key.DeleteValue(valName,false)

    /// Remove an entire profile
    let deleteProfileKey (pKey:string) = 
        checkRoot pKey (sprintf "Refusing to delete unauth key: %A" pKey)

        regLoc.Hive.DeleteSubKey pKey

    /// Set a profile value
    let setProfileValue (pKey:string) valName value =
        checkRoot pKey (sprintf "Refusing to set unauth key value: %A" pKey)

        Registry.SetValue(regLoc.Hive.Name @@ pKey, valName, value);
        value

    /// Set a global (as in ModelMod root global) value
    let setGlobalValue valName value = 
        Registry.SetValue(regLoc.HiveRoot, valName, value);
        value

    /// Get a global (as in ModelMod root global) value
    let getGlobalValue valName (defValue) = 
        let v = Registry.GetValue(regLoc.HiveRoot, valName, defValue)
        v

    /// Creata a new empty profile.  Up to 10000 profiles are supported, more 
    /// available in the pro version.
    let createNewProfile() = 
        let profiles = getProfileKeyNames() 
      
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

    /// Save a profile using the values from the supplied config.  At a minimum,
    /// ExePath must be set in the config.
    let saveProfile (conf:RunConfig) =
        if not (File.Exists conf.ExePath) then
            failwithf "Exe path does not exist, cannot save profile: %A" conf.ExePath

        // already exist?
        let profKey = 
            if conf.ProfileKeyName <> "" then
                Some (regLoc.ProfRoot @@ conf.ProfileKeyName)
            else
                findProfilePath conf.ExePath

        let profKey = 
            match profKey with 
            | Some key -> key
            | None -> createNewProfile()

        let profSave k v = setProfileValue profKey k v

        // this is a syntactic trick to make sure I get a compiler error if I forget to save a field
        ignore
            ({
                ProfileKeyName = profKey
                ProfileName = profSave RegKeys.ProfName conf.ProfileName
                CoreTypes.RunConfig.ExePath = profSave RegKeys.ProfExePath conf.ExePath 
                RunModeFull = profSave RegKeys.ProfRunModeFull (boolAsDword conf.RunModeFull) |> dwordAsBool
                LoadModsOnStart = profSave RegKeys.ProfLoadModsOnStart (boolAsDword conf.LoadModsOnStart) |> dwordAsBool
                InputProfile = profSave RegKeys.ProfInputProfile conf.InputProfile 
                SnapshotProfile = profSave RegKeys.ProfSnapshotProfile conf.SnapshotProfile 
                DocRoot = "" // custom doc root not yet supported
            })

    /// Remove a profile.  Uses the profile key name in the config to locate the 
    /// profile.  Does not require ExePath to be set.
    let removeProfile (conf:RunConfig) =
        let pKey = conf.ProfileKeyName.Trim()
        if pKey = "" then failwith "Empty profile key"

        let pKey = 
            if not (pKey.StartsWith(regLoc.ProfRoot)) then
                regLoc.ProfRoot @@ pKey
            else
                pKey
        deleteProfileKey pKey

    /// Return a default name for a profile, which is just the base exe name.
    let getDefaultProfileName (exePath:String) = Path.GetFileNameWithoutExtension(exePath)
        
    /// Set a default profile name in the specified run config.  If already set, doesn't
    /// change it; otherwise, sets it to getDefaultProfileName(exepath).
    let setDefaultProfileName (rc:RunConfig):RunConfig = 
        if rc.ProfileName = "" then
            { rc with ProfileName = getDefaultProfileName rc.ExePath }
        else 
            rc

    /// Get the global document/data root.
    let getDocRoot():string = 
        regget(regLoc.HiveRoot,RegKeys.DocRoot,DefaultRunConfig.DocRoot) :?> string
    /// Set the global document/data root.
    let setDocRoot(r:string) = 
        setGlobalValue RegKeys.DocRoot r
        
    /// Load a runconfig form the specified profile path and key.
    let loadFromFullProfileKey(profPath:string) (profileKeyName:string):RunConfig = 
        let mmHiveRoot = regLoc.HiveRoot

        let mutable rc = { 
            // eventually this may come from the profile as well, right now it is global
            DocRoot = getDocRoot()

            ProfileKeyName = profileKeyName
            ProfileName = regget(profPath,RegKeys.ProfName,DefaultRunConfig.ProfileName) :?> string
            CoreTypes.RunConfig.ExePath = regget(profPath,RegKeys.ProfExePath,DefaultRunConfig.ExePath) :?> string
            RunModeFull = dwordAsBool ( regget(profPath,RegKeys.ProfRunModeFull, (boolAsDword DefaultRunConfig.RunModeFull)) :?> int )
            LoadModsOnStart = dwordAsBool ( regget(profPath,RegKeys.ProfLoadModsOnStart, (boolAsDword DefaultRunConfig.LoadModsOnStart)) :?> int)
            InputProfile = regget(profPath,RegKeys.ProfInputProfile, DefaultRunConfig.InputProfile) :?> string
            SnapshotProfile = regget(profPath,RegKeys.ProfSnapshotProfile, DefaultRunConfig.SnapshotProfile) :?> string
        }

        setDefaultProfileName rc 

    /// Load a default profile.
    let loadDefaultProfile():RunConfig =
        let profPath = regLoc.Hive.Name @@ regLoc.ProfileDefaultsKey
        loadFromFullProfileKey profPath "" // use empty string for key name when loading from default profile

    /// Load a profile.
    let loadFromProfileKey(profileKey:string):RunConfig =
        let profPath = if not (profileKey.StartsWith(regLoc.ProfRoot)) then regLoc.ProfRoot @@ profileKey else profileKey
        let profPath = regLoc.Hive.Name @@ profPath
        loadFromFullProfileKey profPath profileKey
                    
    /// Return all the available profiles.
    let loadAll (): RunConfig[] =
        getProfileKeyNames() |> Array.map loadFromProfileKey

    /// Load a profile for the specified exe.  Returns a default profile if none found.
    let load (exePath:string):RunConfig = 
        let exePath = exePath.Trim()

        let conf = 
            // Search all profiles for a subkey that has the exe as its ExePath
            let targetProfile = findProfilePath exePath

            let runConfig = 
                match targetProfile with
                | None -> 
                    let pRoot = regLoc.Hive.Name @@ regLoc.ProfRoot
                    log.Info "No profile subkey located in %A for executable %A; using defaults" pRoot exePath
                    // if this defaults key is missing, then we just use the hardcoded defaults below
                    let prof = loadDefaultProfile()
                    // the default profile won't have an exe path, so set it
                    setDefaultProfileName { prof with ExePath = exePath }
                | Some profName -> 
                    loadFromProfileKey profName

            if runConfig.ExePath <> exePath then
                failwithf "Woops, loaded profile does not match exe: (want %s, got profile: %A; loaded from key %A)" exePath runConfig targetProfile
            runConfig

        conf
