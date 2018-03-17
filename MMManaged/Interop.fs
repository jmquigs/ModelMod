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

// Using interop makes the IL unverifiable, disable warning.
#nowarn "9"
/// Defines the main native->managed interface.
module MMNative =
    /// Called by native code to initialize managed code and configuration.
    type SetPathsCB =
        delegate of [<MarshalAs(UnmanagedType.LPWStr)>] mmDllPath: string * [<MarshalAs(UnmanagedType.LPWStr)>] exeModule: string -> InteropTypes.ConfData

    type LoadModDBCB = delegate of unit -> int

    [<StructLayout(LayoutKind.Sequential)>]
    type ManagedCallbacks = {
        SetPaths: SetPathsCB
        LoadModDB: LoadModDBCB
        GetModCount: InteropTypes.GetModCountCB
        GetModData: InteropTypes.GetModDataCB
        FillModData: InteropTypes.FillModDataCB
        TakeSnapshot: InteropTypes.TakeSnapshotCB
        GetLoadingState: InteropTypes.GetLoadingStateCB
    }

module NativeImportsAsD3D9 =
    [< DllImport("d3d9.dll") >]
    extern int OnInitialized(MMNative.ManagedCallbacks callback, uint64 globalStateAddress)
    [< DllImport("d3d9.dll") >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d9.dll") >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d9.dll") >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)

module NativeImportsAsMMNative =
    [< DllImport("mm_native.dll") >]
    extern int OnInitialized(MMNative.ManagedCallbacks callback, uint64 globalStateAddress)
    [< DllImport("mm_native.dll") >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("mm_native.dll") >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("mm_native.dll") >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)

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

/// Managed entry point.  Native code is hardcoded to look for Main.Main(arg:string), and call it after
/// loading the assembly.
type Main() =
    static let mutable oninitialized: ((MMNative.ManagedCallbacks * uint64) -> int) option = None
    static let mutable log:Logging.ILog option = None

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
        try
            // args are | delimited, first arg is nativeGlobalState handle (opaque to managed code)
            // second is load context (i.e is the native code in d3d9.dll or mm_native.dll)
            let args = args.Split([|"|"|], StringSplitOptions.RemoveEmptyEntries) |> Array.map (fun x -> x.Trim())

            let nativeGlobalState = uint64 (args.[0])
            let context = args.[1]

            Main.InitNativeInterface(context)

            let ret = Main.IdentifyInLog()
            let r =
                if ret <> 0 then
                    ret
                else
                    Main.InitCallbacks(nativeGlobalState,context)
            r
        with
            | e ->
                // print it, but it will likely go nowhere
                printfn "Exception: %A" e
                InteropTypes.Assplosion