namespace ModelMod

open System.Runtime.InteropServices
open System.Diagnostics
open System.Text
open System.IO
open System

open Microsoft.Xna.Framework

#nowarn "9"
module MMNative =
    type SetPathsCB = 
        delegate of [<MarshalAs(UnmanagedType.LPWStr)>] mmDllPath: string * [<MarshalAs(UnmanagedType.LPWStr)>] exeModule: string -> InteropTypes.ConfData

    type GetDataPathCB = 
        delegate of unit -> [<MarshalAs(UnmanagedType.LPWStr)>] string

    type LoadModDBCB = delegate of unit -> int

    [<StructLayout(LayoutKind.Sequential, Pack=8)>]
    type ManagedCallbacks = {
        SetPaths: SetPathsCB
        GetDataPath: GetDataPathCB
        LoadModDB: LoadModDBCB
        GetModCount: InteropTypes.GetModCountCB
        GetModData: InteropTypes.GetModDataCB
        FillModData: InteropTypes.FillModDataCB
        TakeSnapshot: InteropTypes.TakeSnapshotCB
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
            RegConfig.Init()

            let phandle = Main.AllocPermaHandle

            let (mCallbacks:MMNative.ManagedCallbacks) = {
                SetPaths = phandle (new MMNative.SetPathsCB(ModDBInterop.SetPaths))
                GetDataPath = phandle (new MMNative.GetDataPathCB(ModDBInterop.GetDataPath))
                LoadModDB = phandle (new MMNative.LoadModDBCB(ModDBInterop.LoadFromDataPath));
                GetModCount = phandle (new InteropTypes.GetModCountCB(ModDBInterop.GetModCount));
                GetModData = phandle (new InteropTypes.GetModDataCB(ModDBInterop.GetModData));
                FillModData = phandle (new InteropTypes.FillModDataCB(ModDBInterop.FillModData));
                TakeSnapshot = phandle (new InteropTypes.TakeSnapshotCB(Snapshot.Take));
            }

            let ret = MMNative.OnInitialized(mCallbacks)

            Interop.log.Info "Init complete, native code: %d " ret 
            ret
        with 
            e ->
                //Main.WriteToFailLog e
                
                Interop.log.Error "%A" e
                InteropTypes.GenericFailureCode        

    static member Main(ignoredArgument:string) = 
        let ret = Main.InitLogging()
        if ret <> 0 then
            ret
        else
            Main.InitCallbacks()