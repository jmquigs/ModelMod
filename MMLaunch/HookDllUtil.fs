namespace MMLaunch

open System

module HookDllUtil =
    type ExeType = X86 | X64
    type TargetData = string * ExeType
    type SrcDllError = SourceDllNotFound | DllExeTypeNotFound
    type CopyError = SrcError of SrcDllError | DirNotFound of string | OtherFileExists of string | OtherException of System.Exception

    let SourceSearchDirs = [".", "Release", @"..\Native\target\release"] // TODO: need to get 32 bit path too

    let HookTargetData = 
        Map.ofList(
            [
                "gw2", ("bin", X86)
                "gw2-64", ("bin64", X64)
            ])

    let getHookTargetData (exebase:string) =
        HookTargetData |> Map.tryFind (exebase.ToLowerInvariant().Trim())

    let getSrcDllPath (mmroot:string) (td:TargetData): Result<unit, SrcDllError> =
        try
            let etype = snd td
            let dllname = 
                match etype with
                | X64 -> "mm_native.dll"
                | X86 -> raise (Exception("unsupported type"))
            ()
        with
            | e -> ()

        Ok(())

        
    let copyHookDllIntoTargetDir (mmroot:string) (targetdir:string) (dryRun:bool): Result<unit, CopyError> = 
        Ok(())
