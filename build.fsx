// include Fake lib
#r @"packages/FAKE/tools/FakeLib.dll"
open Fake
open Fake.AssemblyInfoFile

let buildDir = "./build/"
let buildBin = buildDir + "/Bin"
let testDir = "./test"
let deployDir = "./deploy/"
let nativeOut = "./Release"

let version = "1.0.0.1"  // or retrieve from CI server

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
    !! ("./BlenderScripts/io_scene_mmobj/*.*")
        |> CopyFiles (buildDir + "/BlenderScripts/io_scene_mmobj")

    !! (buildBin + "/ModelMod.exe")
        |> CopyFiles buildDir
)

Target "Zip" (fun _ ->
    !! (buildDir + "/**/*.*")
        -- "**/*.xml"
        -- "**/*.zip"

        |> Zip buildDir (deployDir + "ModelMod-" + version + ".zip")
)

// Dependencies
"Clean"
  ==> "MakeAssInfo" 
  ==> "BuildCS"
  ==> "BuildFS"
  ==> "BuildTest"
  ==> "Test"
  ==> "BuildNative"
  ==> "CopyNative"
  ==> "CopyStuff"
  ==> "Zip"
  ==> "Default"

// start build
RunTargetOrDefault "Default"