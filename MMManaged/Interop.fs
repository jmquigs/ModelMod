namespace ModelMod

open System.Runtime.InteropServices
open System.Diagnostics
open System.Text
open System.IO
open System

open Microsoft.Xna.Framework

#nowarn "9"
module MMNative =
    type SetPathsCallback = 
        delegate of [<MarshalAs(UnmanagedType.LPWStr)>] mmDllPath: string * [<MarshalAs(UnmanagedType.LPWStr)>] exeModule: string -> InteropTypes.ConfData

    type GetDataPathCB = 
        delegate of unit -> [<MarshalAs(UnmanagedType.LPWStr)>] string

    type LoadModDBCallback = delegate of unit -> int

    [<StructLayout(LayoutKind.Sequential, Pack=8)>]
    type ManagedCallbacks = {
        SetPaths: SetPathsCallback
        GetDataPath: GetDataPathCB
        LoadModDB: LoadModDBCallback
        GetModCountCB: InteropTypes.GetModCountCB
        GetModDataCB: InteropTypes.GetModDataCB
        FillModDataCB: InteropTypes.FillModDataCB
        TakeSnapshotCB: InteropTypes.TakeSnapshotCB
    }

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
    
    let setupLogging() = Logging.SetLoggerFactory NativeLogFactory    

    let _,log = NativeLogFactory "Interop"
    
type Main() = 
    static member PermaHandles = new System.Collections.Generic.List<GCHandle>()

    static member Main(ignoredArgument:string) = 
        let ad = AppDomain.CurrentDomain
        let location = ad.BaseDirectory
        let failLogPath = Path.Combine(location, "MMManaged.error.log")

        let allocPermaHandle thing =
            // NOTE: these are unpinned.  For delegates, trying to use GCHandleType.Pinned throws an exception.
            // according to this, pin is not needed:
            // https://msdn.microsoft.com/en-us/library/367eeye0%28VS.80%29.aspx
            Main.PermaHandles.Add(GCHandle.Alloc(thing))
            thing


        Interop.setupLogging()
        Interop.log.Info "Initializing; if there is a problem, will write information here if possible, otherwise to %s" failLogPath
        let logOK = true // didn't throw an exception, at least!

        try
            let (mCallbacks:MMNative.ManagedCallbacks) = {
                SetPaths = allocPermaHandle (new MMNative.SetPathsCallback(ModDBInterop.SetPaths))
                GetDataPath = allocPermaHandle (new MMNative.GetDataPathCB(ModDBInterop.GetDataPath))
                LoadModDB = allocPermaHandle (new MMNative.LoadModDBCallback(ModDBInterop.LoadFromDataPath));
                GetModCountCB = allocPermaHandle (new InteropTypes.GetModCountCB(ModDBInterop.GetModCount));
                GetModDataCB = allocPermaHandle (new InteropTypes.GetModDataCB(ModDBInterop.GetModData));
                FillModDataCB = allocPermaHandle (new InteropTypes.FillModDataCB(ModDBInterop.FillModData));
                TakeSnapshotCB = allocPermaHandle (new InteropTypes.TakeSnapshotCB(Snapshot.Take));
            }

            let ret = MMNative.OnInitialized(mCallbacks)

            Interop.log.Info "Init complete, native code: %d " ret 
            ret
        with 
            e ->
                // write to separate log file 
                File.WriteAllText(failLogPath, e.ToString())  
                // try to log to native code
                if (logOK) then Interop.log.Error "%A" e
                47