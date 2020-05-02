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

open System.IO

open CoreTypes

/// Contains mutable state, including the current configuration and data for all loaded mods.
/// This is stored here so that we don't have to pass it all over the interop barrier, which
/// would be totally nasty (and is also largely irrelevant to code on that side).
module State =
    // log is a function here because otherwise it gets initialized too early and the 
    // log messages get lost.  Making it lazy is not sufficient.  
    // TODO: Might need to do this with other modules...
    let private log() = Logging.getLogger("State")

    // DLL context, set by Interop.Main
    let mutable Context = ""

    /// The data directory contains all data for all games, as well as the selection texture.
    let private defaultDataDir = "Data"

    /// Helper type for finding various directories
    type DirLocator(rootDir:string, conf:RunConfig) =
        override x.ToString() = 
            sprintf "<DirLocator: root %A, conf %A>" rootDir conf

        member x.QueryBaseDataDir() =
            // this is set from registry; if not set, use RootDir + DefaultDataDir
            if conf.DocRoot <> "" then
                conf.DocRoot
            else
                Path.Combine(rootDir, defaultDataDir)

        // This throws an exception if the base data dir does not exist; the exception is intended
        // to stop loading; we don't try to create it or otherwise proceed if it isn't found.
        // To just query the data directory without risk of exception, use
        // QueryBaseDataDir()
        member x.BaseDataDir
            with get() =
                let dataDir = x.QueryBaseDataDir()
                if not (Directory.Exists dataDir) then
                    failwithf "Data directory does not exist: %s" dataDir

                dataDir
        member x.ExeBaseName
            with get() = Path.GetFileNameWithoutExtension(conf.ExePath.ToLowerInvariant())
        member x.ExeDataDir
            with get() = 
                let dd = Path.Combine(x.BaseDataDir,x.ExeBaseName)
                let gameProfDP = conf.GameProfile.DataPathName

                let dirChecks = [
                    fun () -> if Directory.Exists(dd) then Some(dd) else None
                    fun () -> if gameProfDP <> "" && Path.IsPathRooted(gameProfDP) && Directory.Exists(gameProfDP) then Some(gameProfDP) else None
                    fun () -> 
                        if gameProfDP <> "" then 
                            let bdSub = Path.Combine(x.BaseDataDir, gameProfDP)
                            if Directory.Exists(bdSub) then Some(bdSub) else None
                        else 
                            None
                ]

                let dir = dirChecks |> List.tryPick (fun check -> check())
                match dir with 
                | None -> 
                    if gameProfDP <> "" then 
                        log().Warn 
                            "Found data path %A in profile, but it is not extant absolute path or a subdirectory of %A" 
                                conf.GameProfile.DataPathName x.BaseDataDir
                    dd 
                | Some(path) -> path
                
        member x.ExeSnapshotDir
            with get() = Path.Combine(x.ExeDataDir,"snapshots")
        member x.RootDir = rootDir

    // various muties
    let mutable private _moddb = new ModDB.ModDB([],[],[])
    let mutable private _rootDir = "."
    let mutable private _conf = CoreTypes.DefaultRunConfig
    let mutable private _locator = DirLocator(_rootDir,_conf)
    let mutable private _loadState = InteropTypes.AsyncLoadState.NotStarted
    let mutable private _snapProfiles:Map<string,SnapshotProfile.Profile> = Map.ofList []

    // access to the muties out side of the module goes through this, via the "Data" field below.
    type StateDateAccessor() =
        member x.Moddb
            with get() = _moddb
            and set value = _moddb <- value
        member x.Conf
            with get() = _conf
        member x.LoadState
            with get() = _loadState
            and set value = _loadState <- value
        member x.SnapshotProfiles
            with get() = _snapProfiles

    /// Contains all publically accessible data in the State module.
    let Data = new StateDateAccessor()

    /// Verify the specified confiuration and install it in state.  Does not load the Moddb.
    /// Throws exception if confiuration is invalid.
    let validateAndSetConf (rootDir:string) (conf:CoreTypes.RunConfig): CoreTypes.RunConfig =
        if not (Directory.Exists rootDir) then
            failwithf "Root directory does not exist: %s" rootDir

        _rootDir <- rootDir

        let snapProfile =
            try
                let sprofiles = SnapshotProfile.GetAll(_rootDir)
                _snapProfiles <- sprofiles
                if not (sprofiles |> Map.containsKey conf.SnapshotProfile) then
                    log().Error "Unrecognized snapshot profile: %A; no snapshot transforms will be applied" conf.SnapshotProfile
                    log().Info "The following snapshot profiles are available: %A" _snapProfiles
                    ""
                else
                    conf.SnapshotProfile
            with
            | e ->
                log().Error "Error loading snapshot profiles: %A; no snapshot transforms will be applied" e
                ""

        let conf =
            { conf with
                SnapshotProfile = snapProfile
            }
        log().Info "Root dir: %A" (Path.GetFullPath(_rootDir))
        log().Info "Conf: %A" conf

        _conf <- conf
        _locator <- DirLocator(_rootDir,_conf)
        conf

    /// Returns the base directory for document storage (often <MyDocuments>\ModelMod, but controlled from registry)
    let getBaseDataDir() = _locator.BaseDataDir
    /// Returns the base name of the game executable (e.g. "Awesome.exe" -> "Awesome")
    let getExeBaseName() = _locator.ExeBaseName
    /// Returns the executable-specific data directory, which is a combination of the base data directory and
    /// the exe base name.  (e.g. "<MyDocuments>\ModelMod\Awesome")
    let getExeDataDir() = _locator.ExeDataDir
    /// Returns the executable-specific directory snapshot storage.
    /// (e.g. "<MyDocuments>\ModelMod\Awesome\snapshots")
    let getExeSnapshotDir() = _locator.ExeSnapshotDir
    /// Returns the root directory of the ModelMod installation ("c:\modelmod" or whatever)
    let getRootDir() = _locator.RootDir

    let getLocator() = _locator