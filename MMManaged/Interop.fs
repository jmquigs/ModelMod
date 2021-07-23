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

open System.Runtime.InteropServices
open System.Diagnostics
open System.Text
open System.IO
open System

open Microsoft.Xna.Framework

module NativeLogging =
    let factory ninfo nwarn nerror category =
        let category = "M:" + category // M: prefix means "managed", to help differentiate from native log messages

        let formatInfo (result : string ) = ninfo(category, result)
        let formatWarn (result : string ) = nwarn(category, result)
        let formatError (result : string ) = nerror(category, result)

        category, { new Logging.ILog with
            member x.Info format = Printf.ksprintf (formatInfo) format
            member x.Warn format = Printf.ksprintf (formatWarn) format
            member x.Error format = Printf.ksprintf (formatError) format
        }

/// Only valid in "standalone" (development) mode
type StandaloneState = {
    callbacks: MMNative.ManagedCallbacks
    globalStatePointer: uint64
}

[<Struct>]
[<StructLayout(LayoutKind.Sequential)>]
/// Used when loaded via Core CLR api.
type InteropInitStruct =
    [<MarshalAs(UnmanagedType.LPWStr)>]
    val mutable SParam:string
    val mutable IParam:int32

/// Managed entry point.  Native code is hardcoded to look for Main.Main(arg:string), and call it after
/// loading the assembly.
type Main() =
    /// The native code version that this managed code is compatible with.  This should be bumped each
    /// time the interop interface (e.g struct layouts) change.
    static let NativeCodeVersion = 1

    static let mutable oninitialized: ((MMNative.ManagedCallbacks * uint64) -> int) option = None
    static let mutable log:Logging.ILog option = None
    static let mutable standaloneState:StandaloneState option = None

    /// Will be None in non-development mode
    static member StandaloneState with get() = standaloneState

    /// The OnInitialized callback provided by Native code.  This is set lazily once we know what
    /// module name (context) the native code is using.
    static member OnInitialized
        with set(v) = oninitialized <- v
        and get() = oninitialized

    /// Log interface for the native code.  This is set lazily once we know what
    /// module name (context) the native code is using.
    static member Log
        with set(v) = log <- Some(v)
        and get() =
            // lazy init.  or explode.
            match log with
            | Some(log) -> log
            | None ->
                let l = Logging.getLogger("Interop")
                log <- Some(l)
                l

    /// Perma handles prevent the GC from moving around managed memory that native code is pointing at.
    static member PermaHandles = new System.Collections.Generic.List<GCHandle>()

    /// Creates a perma handle.  These are static, so live until the assembly is reloaded.
    static member AllocPermaHandle thing =
        // NOTE: these are unpinned.  For delegates, trying to use GCHandleType.Pinned throws an exception.
        // according to this, pin is not needed:
        // https://msdn.microsoft.com/en-us/library/367eeye0%28VS.80%29.aspx
        Main.PermaHandles.Add(GCHandle.Alloc(thing))
        thing

    // Write specified object to alternate fail log.  Haven't needed this in a while.  It replaces the file
    // (doesn't append), so just call it once.
    static member WriteToFailLog x =
        let ad = AppDomain.CurrentDomain
        let location = ad.BaseDirectory
        let failLogPath = Path.Combine(location, "MMManaged.error.log")
        File.WriteAllText(failLogPath, x.ToString())

    static member InitNativeInterface(context:string) =
        let oninitialized,logfactory =
            match context with
            | "mm_native" ->
                (NativeImportsAsMMNative.OnInitialized,
                    NativeLogging.factory NativeImportsAsMMNative.LogInfo NativeImportsAsMMNative.LogWarn NativeImportsAsMMNative.LogError)
            | "d3d9" ->
                (NativeImportsAsD3D9.OnInitialized,
                    NativeLogging.factory NativeImportsAsD3D9.LogInfo NativeImportsAsD3D9.LogWarn NativeImportsAsD3D9.LogError)
            | "standalone" ->
                let oninit (callbacks,gsp):int =
                    printfn "ONINITIALIZED";
                    standaloneState <- Some({ callbacks = callbacks; globalStatePointer = gsp })
                    0
                let infof (cat:string,msg:string) = printfn "Info[%s]: %s" cat msg
                let warnf (cat:string,msg:string) = printfn "Warn[%s]: %s" cat msg
                let errf (cat:string,msg:string) =  printfn "ERR [%s]: %s" cat msg
                (oninit, NativeLogging.factory infof warnf errf)
            | s ->
                failwithf "unrecognized context: %s" s
        Main.OnInitialized <- Some(oninitialized)
        Logging.setLoggerFactory logfactory

    static member IdentifyInLog() =
        // try to log a test message.  if we can't do that, we're gonna have a bad time.
        // return a code to indicate success/failure
        try
            let asm = System.Reflection.Assembly.GetExecutingAssembly()
            Main.Log.Info "Managed asm: %s" asm.FullName
            Main.Log.Info "Initializing managed code"
            0
        with
            e ->
                InteropTypes.LogInitFailed

    static member InitCallbacks(globalStateAddress:uint64,context:string) =
        // set up delegates for all the managed callbacks and call OnInitialized in native code.  It will
        // likely call back immediately via one of the delegates on the same thread (before OnInitialized returns
        // here).
        try
            RegConfig.init() // sets the hive root

            let phandle = Main.AllocPermaHandle

            let (mCallbacks:MMNative.ManagedCallbacks) = {
                SetPaths = phandle (new MMNative.SetPathsCB(ModDBInterop.setPaths))
                LoadModDB = phandle (new MMNative.LoadModDBCB(ModDBInterop.loadFromDataPathAsync));
                GetModCount = phandle (new InteropTypes.GetModCountCB(ModDBInterop.getModCount));
                GetModData = phandle (new InteropTypes.GetModDataCB(ModDBInterop.getModData));
                FillModData = phandle (new InteropTypes.FillModDataCB(ModDBInterop.fillModData));
                TakeSnapshot = phandle (new InteropTypes.TakeSnapshotCB(Snapshot.take));
                GetLoadingState = phandle (new InteropTypes.GetLoadingStateCB(ModDBInterop.getLoadingState))
                GetSnapshotResult = phandle (new InteropTypes.GetSnapshotResultCB(Snapshot.getResult))
            }

            let ret =
                match Main.OnInitialized with
                | None -> failwithf "OnInitialized callback has not been, uh, Initialized!"
                | Some(cb) -> cb(mCallbacks, globalStateAddress)

            Main.Log.Info "Init complete, native code: %d " ret
            ret
        with
            e ->
                // uncomment to debug problems with this code
                //Main.WriteToFailLog e

                Main.Log.Error "%A" e
                InteropTypes.GenericFailureCode

    static member Main(args:string) =
        let mutable ret = InteropTypes.Assplosion
        try
            // args are | delimited, first arg is nativeGlobalState handle (opaque to managed code)
            // second is load context (i.e is the native code in d3d9.dll or mm_native.dll)
            let args = args.Split([|"|"|], StringSplitOptions.RemoveEmptyEntries) |> Array.map (fun x -> x.Trim())

            let nativeGlobalState = uint64 (args.[0])
            let context = args.[1]

            let mutable versionChecked = false
            if args.Length > 1 then
                let callNativeVersion = int (args.[2])
                if callNativeVersion <> NativeCodeVersion then
                    // can't even try to log if this doesn't match so just get out of here
                    ret <- InteropTypes.NativeCodeMismatch
                    failwithf "Bad native code version"
                versionChecked <- true

            State.Context <- context

            Main.InitNativeInterface(context)

            ret <- Main.IdentifyInLog()
            if ret <> 0 then
                failwithf "Log init failed: %A" ret

            if not versionChecked then
                Main.Log.Warn "Native code did not pass a version, a crash is possible."

            ret <- Main.InitCallbacks(nativeGlobalState,context)
            ret
        with
            | e ->
                // print it, but it will likely go nowhere
                printfn "An exception occured."
                printfn "Exception: %A" e
                if ret = 0 then
                    InteropTypes.Assplosion
                else
                    ret

    /// Weirdly named function use to ad-hoc test load from coreclr.
    static member WankTest(MainArgs, MMRoot, GameExe) =
        printfn "ModelMod Main Stub starting"
        // initialize in standalone mode with a null global state pointer (don't need that, only native uses it).
        let r = Main.Main(MainArgs)
        if r <> 0 then
            failwithf "Interop main failed, code %A" r
        match Main.StandaloneState with
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

    /// Entrypoint use to ad-hoc test load from coreclr.
    static member MainCoreClr(arg:IntPtr, argLen:int):int =
        let expectedSize = sizeof<InteropInitStruct>
        if argLen <> expectedSize then
            printfn "init struct size mismatch: %d/%d" argLen expectedSize
            1
        else if arg = System.IntPtr.Zero then
            printfn "null struct pointer"
            2
        else
            let strct = Marshal.PtrToStructure(arg, typeof<InteropInitStruct>) :?> InteropInitStruct
            printfn "Initializing with %A (ignored: %d)" strct.SParam strct.IParam
            //Main.Main(strct.SParam)
            Main.WankTest(strct.SParam, @"M:\ModelMod", @"F:\Guild Wars 2\Gw2.exe")
