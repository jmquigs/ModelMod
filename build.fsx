// include Fake lib
#r @"packages/FAKE/tools/FakeLib.dll"
open Fake

let buildDir = "./build/"
let buildBin = buildDir + "/Bin"
let testDir = "./test"
let deployDir = "./deploy/"
let nativeOut = "./Release"

let version = "0.1"  // or retrieve from CI server

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
)

Target "Zip" (fun _ ->
    !! (buildDir + "/**/*.*")
        -- "**/*.xml"
        -- "**/*.zip"

        |> Zip buildDir (deployDir + "ModelMod-" + version + ".zip")
)

// Dependencies
"Clean"
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