open System
open ModelMod

(*
This is intended to be a stub/test launcher for the managed assembly library, used for 
development only.

It illustrates how the native code is expected to initialize the assembly using the 
interop interface.
While a managed launcher like this could just bypass the interop interface and call the API inside the 
library directly, doing it this way lets us test (and debug if needed) that interface without having
native code involved.

Some of the following hardcoded options will most likely need to be changed before it runs.

Note that this project hard references the Release dll of MMManaged so you should build that first 
to get rid of the red wiggles.

*)

/// ModelMod root.  In a source checkout this is the root of the tree.
let MMRoot = @"M:\ModelMod"
/// Game whose data you want to load.  The game won't actually be launched, managed code doesn't do that, 
/// but the name is used to find a profile in the registry and then the data associated with that profile.
/// Use MMLaunch to make a profile for the game.  If you don't, this script may not load assets properly.
let GameExe = @"F:\Guild Wars 2\Gw2.exe"

/// You probably shouldn't change this.  In a normal native launch it would be something like "38291382|d3d9".  
/// `|` is a delimiter.  The first part is the integer address of a native-owned memory blob.  Managed code doesn't 
/// look into this and simply passes it back to the native code in a callback (OnInitialized).  
/// The second part tells MMManaged the context it is running in, so that it knows how to locate certain native 
/// functions it expects to be available in its own native context.  
/// Here we use 0 and "standalone" because there is no native context.
let MainArgs = "0|standalone"

[<EntryPoint>]
let main argv =
    printfn "ModelMod Main Stub starting"
    // initialize in standalone mode with a null global state pointer (don't need that, only native uses it).
    let r = ModelMod.Main.Main(MainArgs)
    if r <> 0 then
        failwithf "Interop main failed, code %A" r
    match ModelMod.Main.StandaloneState with
    | None -> failwithf "Standalone state not initialized"
    | Some(state) ->
        let cd = state.callbacks.SetPaths.Invoke(MMRoot, GameExe)
        // the returned conf data is always valid, but it might just contain defaults if a profile could not be loaded.
        // SetPaths() doesn't return an error in that case for whatever reason.
        // oh well, just try loading
        let res = state.callbacks.LoadModDB.Invoke()
        let mutable loading = true
        while loading do
            let loadstate = state.callbacks.GetLoadingState.Invoke()
            loading <- loadstate = ModelMod.InteropTypes.AsyncLoadInProgress || loadstate = ModelMod.InteropTypes.AsyncLoadPending
            if loading then
                System.Threading.Thread.Sleep(250)

    printfn "ModelMod Main Stub exiting, code: %A" r
    0