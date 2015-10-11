// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
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

    [<StructLayout(LayoutKind.Sequential, Pack=8)>]
    type ManagedCallbacks = {
        SetPaths: SetPathsCB
        LoadModDB: LoadModDBCB
        GetModCount: InteropTypes.GetModCountCB
        GetModData: InteropTypes.GetModDataCB
        FillModData: InteropTypes.FillModDataCB
        TakeSnapshot: InteropTypes.TakeSnapshotCB
    }

    /// Called by managed code to provide native code with the callback interface.
    [< DllImport("ModelMod.dll") >] 
    extern int OnInitialized(ManagedCallbacks callback)
    
    [< DllImport("ModelMod.dll") >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("ModelMod.dll") >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("ModelMod.dll") >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)

module Interop =
    let NativeLogFactory category = 
        let category = "M:" + category // M: prefix means "managed", to help differentiate from native log messages

        let formatInfo (result : string ) = MMNative.LogInfo(category, result)
        let formatWarn (result : string ) = MMNative.LogWarn(category, result)
        let formatError (result : string ) = MMNative.LogError(category, result)

        category, { new Logging.ILog with
            member x.Info format = Printf.ksprintf (formatInfo) format
            member x.Warn format = Printf.ksprintf (formatWarn) format
            member x.Error format = Printf.ksprintf (formatError) format
        }
    
    let setupLogging() = Logging.setLoggerFactory NativeLogFactory

    let _,log = NativeLogFactory "Interop"
    
/// Managed entry point.  Native code is hardcoded to look for Main.Main(arg:string), and call it after 
/// loading the assembly.
type Main() = 
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

    static member InitLogging() =
        // try to set up logging and log a test message.  if we can't do that at least, we're gonna have a bad time.
        // return a code to indicate log failure.
        try
            Interop.setupLogging()
            Interop.log.Info "Initializing managed code"
            0
        with 
            e -> 
                InteropTypes.LogInitFailed

    static member InitCallbacks() = 
        // set up delegates for all the managed callbacks and call OnInitialized in native code.  It will 
        // likely call back immediately via one of the delegates on the same thread (before OnInitialized returns 
        // here).
        try
            RegConfig.init() // sets the hive root

            let phandle = Main.AllocPermaHandle

            let (mCallbacks:MMNative.ManagedCallbacks) = {
                SetPaths = phandle (new MMNative.SetPathsCB(ModDBInterop.setPaths))
                LoadModDB = phandle (new MMNative.LoadModDBCB(ModDBInterop.loadFromDataPath));
                GetModCount = phandle (new InteropTypes.GetModCountCB(ModDBInterop.getModCount));
                GetModData = phandle (new InteropTypes.GetModDataCB(ModDBInterop.getModData));
                FillModData = phandle (new InteropTypes.FillModDataCB(ModDBInterop.fillModData));
                TakeSnapshot = phandle (new InteropTypes.TakeSnapshotCB(Snapshot.take));
            }

            let ret = MMNative.OnInitialized(mCallbacks)

            Interop.log.Info "Init complete, native code: %d " ret 
            ret
        with 
            e ->
                // uncomment to debug problems with this code
                //Main.WriteToFailLog e
                
                Interop.log.Error "%A" e
                InteropTypes.GenericFailureCode

    static member Main(ignoredArgument:string) = 
        let ret = Main.InitLogging()
        if ret <> 0 then
            ret
        else
            Main.InitCallbacks()