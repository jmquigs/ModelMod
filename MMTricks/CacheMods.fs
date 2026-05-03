module ModelMod.MMTricks.CacheMods

open System
open System.IO
open System.Threading.Tasks

open ModelMod

open ModelMod.MeshRelation
open MMTricks.Util

let USAGE = """
MMTricks - ModelMod offline pre-cache utility

Usage:
  MMTricks cachemods <gameDataDir> [<listFile>]
                     (--base <name> | --output-dir <path>)
                     [--stdin] [--threads <N>]

Pre-populates the mesh and meshrelation binary caches for a list of mods,
so that the first in-game load of those mods is fast.  The vertex-buffer
fill cache (VBData) is intentionally NOT populated -- it requires the
running game's vertex layout and cannot be produced offline.

IMPORTANT: MMTricks.exe must be run under the SAME .NET runtime the game
will use to consume the cache.  The on-disk cache format is sensitive to
which runtime serialized it, so cache files written by one runtime are unusable by
another and vice versa.

  - Native Windows game:  run MMTricks.exe natively on Windows.
  - Game under proton:    run MMTricks.exe inside the same proton prefix
                          that runs the game (e.g. via protontricks).
                          Running it on the host Linux's dotnet/mono will
                          produce caches that proton's CLR cannot read,
                          and will fail to recognize caches the game has
                          already written.

   - Sample proton run (Replace appid and GAMEBASENAME with actual values)

   APPID=123456
   GAMEBASENAME=mygame
   protontricks-launch --appid $APPID ./ModelMod/Release/MMTricks.exe  -- cachemods /MMData/$GAMEBASENAME testmodload.txt --output-dir /users/steamuser/AppData/Local/ModelMod/BinCache/$GAMEBASENAME

Arguments:
  <gameDataDir>      Directory containing the game's ModIndex.yaml.
  <listFile>         File with one mod name per line.  Lines may be wrapped
                     in single quotes ('Name').  Blank lines and lines
                     starting with '#' are ignored.  Required unless --stdin.

Options:
  --verbose          Enable verbose logging of cache hits/misses.
  --base <name>      Cache lands in
                     %LOCALAPPDATA%/ModelMod/BinCache/<name>.
                     Must match the game's exe basename (e.g. DragonsDogma)
                     for the live Windows game to pick it up.  Resolved
                     against whatever %LOCALAPPDATA% points to under the
                     runtime MMTricks is invoked from -- which under
                     proton is the prefix's per-user AppData, not the
                     host's.
  --output-dir <p>   Cache lands in <p> directly.  Use this when you want
                     to point at an explicit path rather than rely on the
                     %LOCALAPPDATA% lookup.  Mutually exclusive with --base.
  --make-out         If specified with --output-dir, will create the output
                     directory if it does not exist.  Otherwise, the output
                     directory must already exist.

  --stdin            Read mod list from stdin instead of <listFile>.
  --threads <N>      Parallelism for the meshrelation build phase
                     (default: 1).
  --timing           Print modelmod stopwatch timings.

"""

type private Args = {
    GameDataDir: string
    ListFile: string option
    UseStdin: bool
    BaseName: string option
    OutputDir: string option
    MakeOutputDir: bool
    Threads: int
    Verbose: bool
    Timing: bool
}

let private parseArgs (argv: string[]) : Args =
    let mutable positional : string list = []
    let mutable useStdin = false
    let mutable baseName : string option = None
    let mutable outputDir : string option = None
    let mutable verbose = false
    let mutable threads = 1
    let mutable timing = false
    let mutable makeOutputDir = false

    let mutable i = 1
    while i < argv.Length do
        let a = argv.[i]
        match a with
        | "--base" ->
            if i + 1 >= argv.Length then errorExit 2 "--base requires a value"
            baseName <- Some argv.[i+1]
            i <- i + 2
        | "--output-dir" ->
            if i + 1 >= argv.Length then errorExit 2 "--output-dir requires a value"
            outputDir <- Some argv.[i+1]            
            i <- i + 2
        |  "--make-out" ->
            makeOutputDir <- true
            i <- i + 1
        | "--stdin" ->
            useStdin <- true
            i <- i + 1
        | "--verbose" ->
            verbose <- true
            i <- i + 1
        | "--timing" ->
            timing <- true
            i <- i + 1
        | "--threads" ->
            if i + 1 >= argv.Length then errorExit 2 "--threads requires a value"
            match Int32.TryParse(argv.[i+1]) with
            | true, n when n >= 1 -> threads <- n
            | _ -> errorExit 2 (sprintf "--threads must be a positive integer (got '%s')" argv.[i+1])
            i <- i + 2
        | "--help" | "-h" ->
            printfn "%s" USAGE
            exit 0
        | s when s.StartsWith("--") ->
            errorExit 2 (sprintf "Unknown option '%s'\n\n%s" s USAGE)
        | s ->
            positional <- s :: positional
            i <- i + 1

    let positional = List.rev positional

    let gameDataDir, listFileFromPos =
        match positional with
        | [g] -> g, None
        | [g; l] -> g, Some l
        | [] -> errorExit 2 (sprintf "Missing <gameDataDir>\n\n%s" USAGE)
        | _ -> errorExit 2 (sprintf "Too many positional arguments\n\n%s" USAGE)

    if useStdin && listFileFromPos.IsSome then
        errorExit 2 "Specify either --stdin or <listFile>, not both"
    if not useStdin && listFileFromPos.IsNone then
        errorExit 2 (sprintf "Missing <listFile> (or pass --stdin)\n\n%s" USAGE)

    match baseName, outputDir with
    | None, None -> errorExit 2 "One of --base or --output-dir is required"
    | Some _, Some _ -> errorExit 2 "--base and --output-dir are mutually exclusive"
    | _ -> ()

    // if the output directory does not exist, fail
    if outputDir.IsSome && not (Directory.Exists outputDir.Value) then
        if makeOutputDir then
            Directory.CreateDirectory outputDir.Value |> ignore
        else
            errorExit 2 (sprintf "Output directory does not exist: %s; use --make-out to create it" outputDir.Value)

    if not (Directory.Exists gameDataDir) then
        errorExit 2 (sprintf "gameDataDir does not exist: %s" gameDataDir)

    {
        GameDataDir = gameDataDir
        ListFile = listFileFromPos
        UseStdin = useStdin
        BaseName = baseName
        OutputDir = outputDir
        MakeOutputDir = makeOutputDir
        Threads = threads
        Verbose = verbose
        Timing = timing
    }

let private readModNames (args: Args) : string list =
    let rawLines =
        if args.UseStdin then
            Console.In.ReadToEnd().Split([| '\n'; '\r' |])
        else
            File.ReadAllLines(args.ListFile |> Option.get)

    let cleaned =
        rawLines
        |> Array.choose (fun raw ->
            let s = raw.Trim()
            if s = "" || s.StartsWith("#") then None
            else
                let stripped =
                    if s.Length >= 2 && s.[0] = '\'' && s.[s.Length - 1] = '\''
                    then s.Substring(1, s.Length - 2).Trim()
                    else s
                if stripped = "" then None else Some stripped)

    // case-insensitive dedupe, preserving first occurrence
    let seen = System.Collections.Generic.HashSet<string>(StringComparer.OrdinalIgnoreCase)
    [ for n in cleaned do if seen.Add(n) then yield n ]

let private resolveBinCacheDir (args: Args) : string =
    let dir =
        match args.OutputDir, args.BaseName with
        | Some p, _ -> p
        | None, Some b ->
            Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "ModelMod", "BinCache", b)
        | None, None -> failwith "unreachable"
    if not (Directory.Exists dir) then
        Directory.CreateDirectory dir |> ignore
    dir

/// Read the names of all active mods declared in the given ModIndex.yaml.
/// Mirrors the parsing logic in ModDB.loadIndexObjects.
let private readActiveIndexNames (indexPath: string) : System.Collections.Generic.HashSet<string> =
    let text = File.ReadAllText(indexPath)
    use input = new StringReader(text)
    let stream = new YamlDotNet.RepresentationModel.YamlStream()
    stream.Load(input)
    if stream.Documents.Count <> 1 then
        failwithf "Expected 1 document in %s, got %d" indexPath stream.Documents.Count

    let root = Yaml.toMapping "No root node in ModIndex.yaml" stream.Documents.[0].RootNode
    let typ =
        root
        |> Yaml.getValue "type"
        |> Yaml.toString
    if not (typ.Equals("index", StringComparison.OrdinalIgnoreCase)) then
        failwithf "Expected type: \"Index\" in %s, got %s" indexPath typ

    let mods =
        root
        |> Yaml.getValue "mods"
        |> Yaml.toSequence "'mods' sequence not found"

    let result = System.Collections.Generic.HashSet<string>(StringComparer.OrdinalIgnoreCase)
    for node in mods do
        let m = Yaml.toMapping "expected mapping for 'mods' element" node
        let active = m |> Yaml.getOptionalValue "active" |> Yaml.toBool true
        if active then
            let name = m |> Yaml.getValue "name" |> Yaml.toString
            result.Add(name) |> ignore
    result

let private cachemods (args: Args) : int =
    let swAll = System.Diagnostics.Stopwatch.StartNew()

    let indexPath = Path.Combine(args.GameDataDir, "ModIndex.yaml")
    if not (File.Exists indexPath) then
        errorExit 2 (sprintf "ModIndex.yaml not found in gameDataDir: %s" indexPath)

    let binCacheDir = resolveBinCacheDir args
    printfn "BinCacheDir: %s" binCacheDir

    let requested = readModNames args
    if requested.IsEmpty then
        errorExit 2 "Mod list is empty"
    printfn "Requested %d mod(s)" requested.Length

    let indexNames = readActiveIndexNames indexPath
    printfn "Game ModIndex declares %d active mod(s)" indexNames.Count

    let validated, missing =
        requested
        |> List.partition (fun n -> indexNames.Contains n)
    for m in missing do
        eprintfn "WARN: mod '%s' is not in %s; skipping" m indexPath

    if validated.IsEmpty then
        errorExit 1 "No requested mods were found in the game's ModIndex.yaml"

    let validatedSet =
        System.Collections.Generic.HashSet<string>(validated, StringComparer.OrdinalIgnoreCase)

    printfn "Loading mod database (%d mod(s) will be built; mesh cache fills for whole index)..." validated.Length
    let conf : StartConf.Conf = {
        ModIndexFile = Some indexPath
        FilesToLoad = []
        AppSettings = None
        BinCacheDir = binCacheDir
    }
    
    ModelMod.Util.setStopwatchEnabled (args.Timing)

    let mdb = ModDB.loadModDB(conf, None)
    ModelMod.CoreState.Context <- "tool"

    MeshRelation.MeshRelDiskCache.setVerboseLog (args.Verbose)
    ModelMod.MeshDiskCache.setVerboseLog (args.Verbose)

    let allMeshrels = mdb.MeshRelations |> List.toArray
    let toBuild =
        allMeshrels
        |> Array.filter (fun mr -> validatedSet.Contains mr.DBMod.Name)
    printfn "Constructed %d meshrelation(s); building %d (rest skipped)"
        allMeshrels.Length toBuild.Length

    let built = ref 0
    let errored = ref 0
    let lockObj = obj()

    let opts = ParallelOptions(MaxDegreeOfParallelism = args.Threads)
    let body =
        Action<MeshRelation>(fun mr ->
            try
                mr.Build() |> ignore
                lock lockObj (fun () ->
                    built := !built + 1
                    printfn "  [%d/%d] built meshrel: mod='%s' ref='%s'"
                        !built toBuild.Length mr.DBMod.Name mr.DBRef.Name)
            with e ->
                lock lockObj (fun () ->
                    errored := !errored + 1
                    eprintfn "ERROR: meshrel build failed for mod='%s' ref='%s': %s"
                        mr.DBMod.Name mr.DBRef.Name e.Message))
    Parallel.ForEach(toBuild, opts, body) |> ignore

    swAll.Stop()
    printfn ""
    printfn "Summary:"
    printfn "  requested:     %d" requested.Length
    printfn "  in-index:      %d" validated.Length
    printfn "  not-in-index:  %d" missing.Length
    printfn "  meshrels built: %d" !built
    printfn "  errors:        %d" !errored
    printfn "  meshes pre-cached (whole index): %d" allMeshrels.Length
    printfn "  elapsed:       %.2f s" (float swAll.ElapsedMilliseconds / 1000.0)
    printfn "  cache dir:     %s" binCacheDir

    if !errored > 0 then 1 else 0

let run argv = 
    let args = parseArgs argv
    cachemods args