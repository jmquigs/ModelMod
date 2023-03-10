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

let version = "1.1.0.4"  // or retrieve from CI server

let updateRcVersions rcFile =
    let lines = File.ReadAllLines(rcFile)
    let replVer (vSearch:string) (formatter:unit -> string) (l:string) =
        if l.TrimStart().ToUpperInvariant().StartsWith(vSearch.ToUpperInvariant()) then
            let vidx = l.ToUpperInvariant().IndexOf(vSearch.ToUpperInvariant())
            l.Substring(0,vidx) + vSearch + formatter()
        else
            l

    let fn =
        (replVer "VALUE \"FileVersion\", " (fun _ -> (sprintf "\"%s\"" version) ))
        >> (replVer "VALUE \"ProductVersion\", " (fun _ -> (sprintf "\"%s\"" version)))
        >> (replVer "FILEVERSION " (fun _ -> (sprintf "%s" (version.Replace(".",",") ))))
        >> (replVer "PRODUCTVERSION " (fun _ -> (sprintf "%s" (version.Replace(".",",") ))))
    let lines = lines |> Array.map fn
    File.WriteAllLines(rcFile, lines)
    trace ("Updating versions in rc file: " + rcFile)
    ()

Target "Clean" (fun _ ->
    CleanDirs [buildDir; testDir; deployDir; ".\ModelMod\Release"; ".\MMLoader\Release"; nativeOut]
)

Target "Default" (fun _ ->
    trace "Build Complete"
)

// Utility to run a proc with captured stdout.  10 second time limit.
// stderr not captured.
// Thought fake could do this, but maybe not?
let runCaptured cmd arg =
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
    proc.WaitForExit(10000) |> ignore
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

    // note current cargo toolchain
    let result = runCaptured "rustup" "show"
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
        runUncaptured "rustup" (sprintf "default %s" tc) wd (System.TimeSpan.FromSeconds(8.88))
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

Target "MakeAssInfo" (fun _ ->
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
    ()
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

// Dependencies
"MakeAssInfo"
    ==> "UpdateRcVersions"
    ==> "UpdateVersions"

"CopyNative"
    ==> "CopyStuff"
    ==> "Zip"
    ==> "Package"

"UpdateVersions"
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

let targ = System.Environment.GetEnvironmentVariable("TARGET")
if targ <> null
then RunTargetOrDefault targ
else
    // start build
    RunTargetOrDefault "UpdateVersions"
