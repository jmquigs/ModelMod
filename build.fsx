// include Fake lib
#r @"packages/FAKE/tools/FakeLib.dll"
open Fake
open Fake.AssemblyInfoFile

open System.IO

let buildDir = "./build/"
let buildBin = buildDir + "/Bin"
let testDir = "./test"
let deployDir = "./deploy/"
let nativeOut = "./Release"

let baseVersion: string = "1.2.0"
let defaultBuildNumber = 0 // obtained from CI later, if possible
let mutable version = ""
let updateVersion(buildNumber) = version <- baseVersion + "." + buildNumber.ToString()
do updateVersion(defaultBuildNumber)

let replVer (vSearch:string) (formatter:unit -> string) (l:string) =
    if l.TrimStart().ToUpperInvariant().StartsWith(vSearch.ToUpperInvariant()) then
        let vidx = l.ToUpperInvariant().IndexOf(vSearch.ToUpperInvariant())
        l.Substring(0,vidx) + vSearch + formatter()
    else
        l

let updateVersionsInFile (file:string) (fnReplacer: (string) ->string) =
    let lines = File.ReadAllLines(file)
    let lines = lines |> Array.map fnReplacer
    File.WriteAllLines(file, lines)

let updateCargoVersion cargoFile =
    let fn =
        (replVer "ProductVersion = " (fun _ -> (sprintf "\"%s\"" version) ))
    updateVersionsInFile cargoFile fn

    trace ("Updating versions in cargo file: " + cargoFile)

let updateRcVersions rcFile =
    let fn =
        (replVer "VALUE \"FileVersion\", " (fun _ -> (sprintf "\"%s\"" version) ))
        >> (replVer "VALUE \"ProductVersion\", " (fun _ -> (sprintf "\"%s\"" version)))
        >> (replVer "FILEVERSION " (fun _ -> (sprintf "%s" (version.Replace(".",",") ))))
        >> (replVer "PRODUCTVERSION " (fun _ -> (sprintf "%s" (version.Replace(".",",") ))))
    updateVersionsInFile rcFile fn

    trace ("Updating versions in rc file: " + rcFile)

Target "Clean" (fun _ ->
    CleanDirs [buildDir; testDir; deployDir; ".\ModelMod\Release"; ".\MMLoader\Release"; nativeOut]
)

Target "Default" (fun _ ->
    trace "Build Complete"
)

// Utility to run a proc with captured stdout.
// stderr not captured.
// Thought fake could do this, but maybe not?
let runCaptured cmd arg (timeOut:System.TimeSpan) =
    let outLines = new ResizeArray<string>()
    let info = new System.Diagnostics.ProcessStartInfo()
    info.FileName <- cmd
    info.Arguments <- arg
    info.UseShellExecute <- false
    info.RedirectStandardOutput <- true
    let proc = System.Diagnostics.Process.Start(info)
    let reader() =
        while not proc.StandardOutput.EndOfStream do
                let line = proc.StandardOutput.ReadLine()
                outLines.Add(line)
    let t = new System.Threading.Thread(reader)
    t.Start()
    proc.WaitForExit(int timeOut.TotalMilliseconds) |> ignore
    if not proc.HasExited then failwithf "%A: waited too long for proc to exit" (cmd,arg)
    if proc.ExitCode <> 0 then failwithf "%A: non zero exit code: %A" (cmd,arg) proc.ExitCode
    outLines.ToArray()

let runUncaptured cmd arg wd timeOut =
    let result =
        ExecProcess (fun info ->
            info.FileName <- cmd
            info.Arguments <- arg
            info.WorkingDirectory <- wd
        ) timeOut
    if result <> 0 then failwithf "proc %Areturned with a non-zero exit code: %A" (cmd,arg) result

let runBuildNative() =
    let wd = System.Environment.CurrentDirectory

    if not (Directory.Exists("Release")) then Directory.CreateDirectory("Release") |> ignore

    // note current cargo toolchain, long timeout due to slow CI
    let result = runCaptured "rustup" "show" (System.TimeSpan.FromMinutes(2.00))
    let result = result |> Array.filter (fun l -> l.Contains("default")) |> Array.head
    let defToolchain = result.Split([|" "|], System.StringSplitOptions.RemoveEmptyEntries).[0]

    let dobuild bits =
        let tc =
            match bits with
            | 32 -> "stable-i686-pc-windows-msvc"
            | 64 -> "stable-x86_64-pc-windows-msvc"
            | _ -> failwithf "invalid bits: %d" bits

        if defToolchain <> tc then
            printfn "============== Warning! Switching to rust toolchain: %s; your prior toolchain will be restored on exit, but not if you Ctrl-C" tc

        if Directory.Exists(@"Native\target\release") then Directory.Delete(@"Native\target\release", true)
        if Directory.Exists(@"Native\target\debug") then Directory.Delete(@"Native\target\debug", true)

        let wd = (sprintf @"%s\Native" wd)
        // switching the toolchain needs a long timeout, because if its not installed (like on CI)
        // it will be downloaded and setup which can take a while in that slow environment
        runUncaptured "rustup" (sprintf "default %s" tc) wd (System.TimeSpan.FromMinutes(5.00))
        runUncaptured "cargo" "build --release" wd (System.TimeSpan.FromMinutes 10.00)

        let destDir = sprintf "Release\\modelmod_%d" bits
        if not (Directory.Exists(destDir)) then Directory.CreateDirectory(destDir) |> ignore
        File.Copy(@"Native\target\release\hook_core.dll", sprintf @"%s\d3d9.dll" destDir, true)
        File.Copy(@"Native\target\release\hook_core.dll", sprintf @"%s\d3d11.dll" destDir, true)

    try
        dobuild 64
        dobuild 32
    finally
        if defToolchain.Trim() <> "" then
            runCaptured "rustup" (sprintf "default %s" defToolchain) |> ignore
            printfn "==> restored prior toolchain: %A" defToolchain


// This target has no deps so that it can be run independently
Target "BuildNativeOnly" (fun _ ->
    runBuildNative()
)

Target "BuildNative" (fun _ ->
    runBuildNative()
)

Target "UpdateAssembyInfo" (fun _ ->
    printfn "version is %A" version
    CreateFSharpAssemblyInfo "./MMLaunch/AssemblyInfo.fs"
        [Attribute.Title "ModelMod launcher application"
         Attribute.Description ""
         Attribute.Guid "2ce8e338-7143-4f97-ab39-3e90ca50bdf2"
         Attribute.Product "ModelMod"
         Attribute.Version version
         Attribute.FileVersion version]

    CreateFSharpAssemblyInfo "./MMManaged/AssemblyInfo.fs"
        [Attribute.Title "ModelMod managed code library"
         Attribute.Description ""
         Attribute.Guid "13c62567-ab30-4954-9c47-213bc2a0ab7e"
         Attribute.Product "ModelMod"
         Attribute.Version version
         Attribute.FileVersion version]

    CreateFSharpAssemblyInfo "./StartupApp/AssemblyInfo.fs"
        [Attribute.Title "ModelMod launcher 'shortcut'"
         Attribute.Description ""
         Attribute.Guid "df438f0d-1e48-42d2-bc4d-7b3500c48515"
         Attribute.Product "ModelMod"
         Attribute.Version version
         Attribute.FileVersion version]

    CreateCSharpAssemblyInfo "./MMAppDomain/Properties/AssemblyInfo.cs"
        [Attribute.Title "ModelMod CLR app domain host"
         Attribute.Description ""
         Attribute.Guid "7b59b7f1-5876-4dd3-abc5-ee380144983d"
         Attribute.Product "ModelMod"
         Attribute.Version version
         Attribute.FileVersion version]
)

Target "UpdateRcVersions" (fun _ ->
    updateCargoVersion ("./Native/hook_core/Cargo.toml")
    //updateRcVersions ("./ModelMod/ModelMod.rc")
    //updateRcVersions ("./MMLoader/MMLoader.rc")
)

Target "BuildCS" (fun _ ->
    !! "**/*.csproj"
      |> MSBuildRelease buildBin "Build"
      |> Log "BuildCS-Output: "
)

Target "BuildFS" (fun _ ->
    !! "**/*.fsproj"
      -- "**/Test.*"
      -- "**/Dn5*.fsproj"
      |> MSBuildRelease buildBin "Build"
      |> Log "BuildFS-Output: "
)

Target "BuildTest" (fun _ ->
    !! "**/Test.*.fsproj"
      -- "**/Test.ManagedLaunch.fsproj"
      |> MSBuildRelease testDir "Build"
      |> Log "BuildTest-Output: "
)

Target "Test" (fun _ ->
    !! (testDir + "/Test.*.dll")
      |> NUnit (fun p ->
          {p with
             DisableShadowCopy = true;
             OutputFile = testDir + "/Test.Results.xml" })
)

Target "CopyNative" (fun _ ->
    [
        //!! (nativeOut + "/**/*.*")
        !! (nativeOut + "/modelmod_32/*.*")
        !! (nativeOut + "/modelmod_64/*.*")
        -- "**/*.iobj"
        -- "**/*.ipdb"
        -- "**/*.exp"
        -- "**/*.lib"
        -- "**/*.pdb"]
        |> CopyWithSubfoldersTo buildBin
    // The "Release" directory will be included and is unneeded,
    // so clean that up
    let moveBinDir (dirname) =
        let targ = buildBin + "/" + dirname
        if Directory.Exists(targ) then
            Directory.Delete(targ, true)
        Directory.Move((buildBin + "/Release/" + dirname), targ)
    moveBinDir "modelmod_32"
    moveBinDir "modelmod_64"
    Directory.Delete(buildBin + "/Release")
)

Target "CopyStuff" (fun _ ->
    let logsDir = buildDir + "/Logs"
    ensureDirectory (buildDir + "/Logs")
    System.IO.File.WriteAllText(logsDir + "/README.TXT", "Log files will be written here when you launch a game with ModelMod.");

    !! ("./BlenderScripts/*.*")
        |> CopyFiles (buildDir + "/BlenderScripts")
    !! ("./SnapshotProfiles/*.*")
        |> CopyFiles (buildDir + "/SnapshotProfiles")
    !! ("./BlenderScripts/io_scene_mmobj/*.*")
        |> CopyFiles (buildDir + "/BlenderScripts/io_scene_mmobj")
    !! ("./LICENSE.txt")
        |> CopyFiles (buildDir)
    !! ("./TPLib/*.*")
        |> CopyFiles (buildDir + "/TPLib")
    !! ("./Docs/binpackage/README.md")
        |> CopyFiles (buildDir)

    !! (buildBin + "/ModelMod.exe")
        |> CopyFiles buildDir
)

Target "Zip" (fun _ ->
    !! (buildDir + "/**/*.*")
        -- "**/*.xml"
        -- "**/*.zip"

        |> Zip buildDir (deployDir + "ModelMod-" + version + ".zip")
)

Target "UpdateVersions" (fun _ ->
    trace ("Version updated to: " + version)
)

// Signing stuff
// Unused, I no longer sign anything since I can't afford signing certs
Target "SignBuild" (fun _ ->
    let certExpired = System.DateTime.Parse("11/10/2016")
    if (System.DateTime.Now > certExpired) then
        failwithf "cert expired, rewind the clock to before %A or renew the cert for $$$" certExpired

    // TODO: download last build from appveyor
    let signDir = "./sign"

    let files = Directory.GetFiles(signDir, "*.zip")
    if files.Length <> 1 then
        failwithf "expected only one zip in signDir, but got: %A" files

    let targetZip = files.[0]

    let zipTemp = Path.Combine(signDir, "ziptemp")
    if (Directory.Exists zipTemp) then
        Directory.Delete(zipTemp,true)

    Directory.CreateDirectory zipTemp |> ignore

    Unzip zipTemp targetZip

    let files =
        [
            "ModelMod.exe";
            "Bin\ModelMod.exe";
            "Bin\ModelMod.dll";
            "Bin\MMLoader.exe";
            "Bin\MeshView.exe";
            "Bin\WpfInteropSample.exe";
            "Bin\MMLaunch.exe";
            "Bin\MMManaged.dll";
            "Bin\ModelModCLRAppDomain.dll";
        ] |> List.map (fun p -> Path.Combine(zipTemp,p))

    files |> List.iter (fun f ->
        if not (File.Exists f) then
            failwithf "File does not exist: %A" f
    )

    let certPathFile = "certpath.txt"
    if not (File.Exists certPathFile) then
        failwithf "PK path file not found: %s" certPathFile

    let certPath = File.ReadAllText(certPathFile).Trim()

    printfn "Signing %A" files
    printfn "Enter cert key password (shhhhh):"
    let pass = System.Console.ReadLine().Trim()

    let passFile = Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.MyDocuments),sprintf "__temp__pass_%s.txt" (System.Guid.NewGuid().ToString()))

    try
        File.WriteAllText(passFile,pass)
        SignTool @"C:\Program Files (x86)\Windows Kits\8.1\bin\x64" certPath passFile files
        File.Delete passFile
    with
        | e ->
            File.Delete passFile
            raise e

    let outDir = @".\deploy\signed"
    if (Directory.Exists outDir) then
        Directory.Delete (outDir,true)
    Directory.CreateDirectory outDir |> ignore

    let outZip = Path.Combine(outDir, Path.GetFileName(targetZip))

    CreateZip zipTemp outZip "modelmod" 9 false (Directory.GetFiles(zipTemp,"*.*",SearchOption.AllDirectories))

    printfn "Created zip with signed files: %A" outZip
)

// Top level targets

Target "FullBuild" (fun _ ->
    Run "AppveyorBuild"
    Run "AppveyorTest"
    Run "AppveyorPackage"
)

Target "Package" ignore
Target "AppveyorBuild" ignore
Target "AppveyorTest" ignore
Target "AppveyorPackage" ignore
Target "OnlyUpdateVersions" ignore
Target "BuildRoot" ignore
Target "CheckConf" ignore

// Dependencies
//   Since I tend to forget this, this is the order things should be built in.  So running fake
//   with a top level target like "BuildRoot" does nothing, because nothing depends on that
//   target.  So if you want the full "BuildRoot" chain what you actually run is
//   "AppveyorBuild" which (at this time) is its final dependency.
//   In general the last target in each chain is the are the explicit runnable targets.
"CopyNative"
    ==> "CopyStuff"
    ==> "Zip"
    ==> "Package"

"BuildRoot"
    ==> "OnlyUpdateVersions"
    ==> "Clean"
    ==> "BuildCS"
    ==> "BuildFS"
    ==> "BuildNative"
    ==> "AppveyorBuild"

"BuildTest"
    ==> "Test"
    ==> "AppveyorTest"

"Package"
    ==> "AppveyorPackage"

"UpdateVersions"
    ==> "UpdateRcVersions"
    ==> "UpdateAssembyInfo"
    ==> "OnlyUpdateVersions"

let VSSearchPaths =
    [
        @"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise" // github win 2019 runner
        @"F:\Program Files (x86)\Microsoft Visual Studio\2019\Community" // my system
    ]
let MSBuildPaths =
    [
        @"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\MSBuild\Current\Bin" // github win 2019 runner
        @"F:\Program Files (x86)\Microsoft Visual Studio\2019\Community\MSBuild\Current\Bin" // my system
    ]

let checkAndUpdateVersion = true
if checkAndUpdateVersion then
    // check for build num env var
    let buildNum = System.Environment.GetEnvironmentVariable("CI_BUILD_NUMBER")
    if buildNum <> null then
        try
            let buildNum = int (buildNum.Trim())
            updateVersion(buildNum)
            printfn "version changed to: %A" version
        with
        | e -> failwithf "failed to set build number: %A" e
    printfn "MM Version: %A" version
/// Used to setup github CI build (or debug that build by looking for stuff and printing
/// out what it finds)
let doCISetup() =
    let showEnv =
        printfn "vs vars:"
        let want =
            [
                "MSBuild"
                //"HOMEPATH"
                "VSCMD_VER"
                "VsInstallRoot"
                "VSINSTALLDIR"
                "VisualStudioVersion"
            ] |> List.map (fun f -> f.ToUpperInvariant())
        System.Environment.GetEnvironmentVariables()
        |> Seq.cast<System.Collections.DictionaryEntry>
        |> Seq.filter (fun kv -> (want |> List.contains (kv.Key.ToString().ToUpperInvariant())) )
        |> Seq.map (fun kv -> sprintf "%A=%A" kv.Key kv.Value)
        |> Seq.iter (fun s -> printfn "%s" s)
        ()

    // MSBuild will usually to fail to set up the FSharpTargetsPath variable properly, so we need to
    // manually slam it to the proper value in each project.
    // github has these targets files as of 3/10/2023 on the windows 2019 build image
    // C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\MSBuild\Microsoft\VisualStudio\v16.0\FSharp\Microsoft.FSharp.targets
    // C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\Common7\IDE\CommonExtensions\Microsoft\FSharp\Tools\Microsoft.FSharp.Targets
    let tryUpdateProj = true
    if tryUpdateProj then
        let usePath = @"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\MSBuild\Microsoft\VisualStudio\v16.0\FSharp\Microsoft.FSharp.targets"

        let updateList = [
            "MMLaunch/MMLaunch.fsproj"
            "MMManaged/MMManaged.fsproj"
            "MMView/MMView.fsproj"
            "Test.MMManaged/Test.MMManaged.fsproj"
        ]
        let mutable found = false
        updateList |> List.iter (fun proj ->
            let lines =
                System.IO.File.ReadAllLines(proj)
                |> Array.map (fun line ->
                    if line.Contains("Import Project=\"$(FSharpTargetsPath)\"") then
                        found <- true
                        sprintf """<Import Project="%s" />""" usePath
                    else
                        line
                )
            printfn "updating fsharp targets in %s: %A" proj found
            System.IO.File.WriteAllLines(proj,lines)
        )

    // look for some vs files and print them out if found.
    // this is very slow on CI (like 8 minutes) do don't do it unless you need to debug something
    let lookForFsharpTargets = false
    if lookForFsharpTargets then
        // look for fsharp targets file and print any found
        VSSearchPaths
        |> List.tryFind (fun p -> System.IO.Directory.Exists p)
        |> function
            | Some p ->
                let files = System.IO.Directory.GetFiles(p, "*", System.IO.SearchOption.AllDirectories)
                let fsharpFiles = files |> Array.filter (fun f -> f.ToLowerInvariant().Contains("fsharp.targets"))
                printfn "fsharp targets: %A" fsharpFiles
            | None -> failwithf "no vs path found, searched %A" VSSearchPaths

    printfn "action setup complete"

let getVSPath() =
    VSSearchPaths
    |> List.tryFind (fun p -> System.IO.Directory.Exists p)
    |> function
        | Some p -> p
        | None -> failwithf "no vs path found, searched %A" VSSearchPaths

// Verify that the env is set up properly and if not, try to set it up.  set SKIPENV=1 to skip this
// min env needed to build is these vars, MSBuild is optional but if there are a lot of vs studio
// versions installed Fake may find the wrong one.
    //   VSINSTALLDIR: C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise
    //   VsInstallRoot: C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise
    //   VisualStudioVersion: 16.0
    //   MSBuild: C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\MSBuild\Current\Bin
let envv x = System.Environment.GetEnvironmentVariable(x)
let checkEnv = envv("SKIPENV") <> "1"
if checkEnv then
    let vsPath = envv("VSINSTALLDIR")
    match vsPath with
    | null ->
        let vspath = getVSPath()
        printfn "setting VSINSTALLDIR: %A " vspath
        System.Environment.SetEnvironmentVariable("VSINSTALLDIR", vspath)
    | x -> printfn "VSINSTALLDIR is set to %A" x
    let vsPath = envv("VsInstallRoot")
    match vsPath with
    | null ->
        let vspath = getVSPath()
        printfn "setting VsInstallRoot: %A " vspath
        System.Environment.SetEnvironmentVariable("VsInstallRoot", vspath)
    | x -> printfn "VsInstallRoot is set to %A" x
    if envv "VSINSTALLDIR" <> envv "VsInstallRoot" then
        failwithf "VSINSTALLDIR and VsInstallRoot must be the same, but are %A and %A" (envv "VSINSTALLDIR") (envv "VsInstallRoot")
    match envv("VisualStudioVersion") with
    | null ->
        printfn "setting VisualStudioVersion: 16.0"
        System.Environment.SetEnvironmentVariable("VisualStudioVersion", "16.0")
    | x -> printfn "VisualStudioVersion is set to %A" x
    match envv("MSBuild") with
    | null ->
        let msb = MSBuildPaths |> List.tryFind (fun p -> System.IO.Directory.Exists p)
        match msb with
        | Some p ->
            printfn "setting MSBuild: %A" p
            System.Environment.SetEnvironmentVariable("MSBuild", p)
        | None ->
            printfn "default ms build not found, using whatever rando version Fake finds"

    | x -> printfn "msbuild is set to %A" x
else
    printfn "skipping env check"

// Start the run
// from git bash, run this with something like:
// TARGET=AppveyorBuild fsi build.fsx
// if run without a target it runs "CheckConf" which does init checks
let targ = envv("TARGET")
match targ with
| "actionsetup" -> doCISetup()
| null -> RunTargetOrDefault "CheckConf"
| targ -> RunTargetOrDefault targ
