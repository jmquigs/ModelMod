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

let version = "1.0.0.13"  // or retrieve from CI server

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

Target "BuildNative" (fun _ ->
    !! "**/ModelMod.sln"
      |> MSBuildRelease buildBin "Build" // note, native code ignores the buildBin override, so we have to copy manually later
      |> Log "BuildNative-Output: "
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
    updateRcVersions ("./ModelMod/ModelMod.rc")
    updateRcVersions ("./MMLoader/MMLoader.rc")
)

Target "BuildCS" (fun _ ->
    !! "**/*.csproj"
      |> MSBuildRelease buildBin "Build"
      |> Log "BuildCS-Output: "
)

Target "BuildFS" (fun _ ->
    !! "**/*.fsproj"
      -- "**/Test.*"
      |> MSBuildRelease buildBin "Build"
      |> Log "BuildFS-Output: "
)

Target "BuildTest" (fun _ ->
    !! "**/Test.*.fsproj"
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
    !! (nativeOut + "/**/*.*")
        -- "**/*.iobj"
        -- "**/*.ipdb"
        -- "**/*.exp"
        -- "**/*.lib"
        -- "**/*.pdb"
        |> CopyFiles buildBin
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

// start build
RunTargetOrDefault "UpdateVersions"
