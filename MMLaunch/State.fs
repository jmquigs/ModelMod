// Slim State module: only DirLocator is needed by the launcher to derive the
// per-game data and snapshot directories. The full module in MMManaged also
// owns the loaded mod database and snapshot-profile cache; the launcher does
// not.

namespace ModelMod

open System.IO

open CoreTypes

module State =
    let private defaultDataDir = "Data"

    type DirLocator(rootDir: string, conf: RunConfig) =
        override x.ToString() =
            sprintf "<DirLocator: root %A, conf %A>" rootDir conf

        member x.QueryBaseDataDir() =
            if conf.DocRoot <> "" then conf.DocRoot
            else Path.Combine(rootDir, defaultDataDir)

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
                let dd = Path.Combine(x.BaseDataDir, x.ExeBaseName)
                let gameProfDP = conf.GameProfile.DataPathName

                let dirChecks =
                    [ (fun () -> if Directory.Exists dd then Some dd else None)
                      (fun () ->
                          if gameProfDP <> "" && Path.IsPathRooted(gameProfDP) && Directory.Exists(gameProfDP) then
                              Some gameProfDP
                          else None)
                      (fun () ->
                          if gameProfDP <> "" then
                              let bdSub = Path.Combine(x.BaseDataDir, gameProfDP)
                              if Directory.Exists bdSub then Some bdSub else None
                          else None) ]

                match dirChecks |> List.tryPick (fun check -> check()) with
                | Some path -> path
                | None -> dd

        member x.ExeSnapshotDir = Path.Combine(x.ExeDataDir, "snapshots")
        member x.RootDir = rootDir
