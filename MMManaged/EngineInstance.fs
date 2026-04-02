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

open System
open System.IO
open System.Reflection

/// Loads and manages the hot-reloadable engine assembly (MMManaged.Engine.dll).
/// Provides stable wrapper functions that the shell's delegates point to. On reload,
/// the internal callback targets are swapped to the new engine assembly's implementations.
module EngineInstance =
    let private log() = Logging.getLogger("EngineInstance")

    /// The well-known type name in the engine assembly that implements IEngineModule.
    let private EngineEntryTypeName = "ModelMod.Engine.EngineEntry"

    /// The engine assembly file name (relative to the shell assembly's directory).
    let private EngineAssemblyName = "MMManaged.Engine.dll"

    /// Current engine callbacks. Swapped on reload.
    let mutable private engineCallbacks: MMNative.ManagedCallbacks option = None

    /// Track the number of reloads for logging.
    let mutable private reloadCount = 0

    /// Track whether we've been initialized at least once.
    let mutable private initialized = false

    // -----------------------------------------------------------------------
    // Stable wrapper functions.
    // These are the targets of the shell's GC-pinned delegates passed to native code.
    // They forward calls through to the current engine implementation.
    // -----------------------------------------------------------------------

    let setPaths (dllPath: string) (exeModule: string) : InteropTypes.ConfData =
        match engineCallbacks with
        | Some(cb) -> cb.SetPaths.Invoke(dllPath, exeModule)
        | None -> failwith "EngineInstance: engine module not loaded (setPaths)"

    let loadModDB () : int =
        match engineCallbacks with
        | Some(cb) -> cb.LoadModDB.Invoke()
        | None -> InteropTypes.GenericFailureCode

    let getModCount () : int =
        match engineCallbacks with
        | Some(cb) -> cb.GetModCount.Invoke()
        | None -> 0

    let getModData (modIndex: int) : InteropTypes.ModData =
        match engineCallbacks with
        | Some(cb) -> cb.GetModData.Invoke(modIndex)
        | None -> InteropTypes.EmptyModData

    let fillModData
        (modIndex: int)
        (declData: nativeptr<byte>)
        (declSize: int32)
        (vbData: nativeptr<byte>)
        (vbSize: int32)
        (ibData: nativeptr<byte>)
        (ibSize: int32) : int =
        match engineCallbacks with
        | Some(cb) -> cb.FillModData.Invoke(modIndex, declData, declSize, vbData, vbSize, ibData, ibSize)
        | None -> InteropTypes.GenericFailureCode

    let loadModData (modIndex: int) : int =
        match engineCallbacks with
        | Some(cb) -> cb.LoadModData.Invoke(modIndex)
        | None -> InteropTypes.GenericFailureCode

    let takeSnapshot (device: nativeint) (snapData: InteropTypes.SnapshotData) : int =
        match engineCallbacks with
        | Some(cb) -> cb.TakeSnapshot.Invoke(device, snapData)
        | None -> InteropTypes.GenericFailureCode

    let getLoadingState () : int =
        match engineCallbacks with
        | Some(cb) -> cb.GetLoadingState.Invoke()
        | None -> InteropTypes.AsyncLoadNotStarted

    let getSnapshotResult () : InteropTypes.SnapshotResult =
        match engineCallbacks with
        | Some(cb) -> cb.GetSnapshotResult.Invoke()
        | None -> { Directory = ""; SnapFilePrefix = ""; DirectoryLen = 0; SnapFilePrefixLen = 0 }

    // -----------------------------------------------------------------------
    // Assembly loading
    // -----------------------------------------------------------------------

    /// Find the engine assembly DLL path based on the shell assembly's location.
    let private findEngineAssemblyPath () =
        let shellAsm = Assembly.GetExecutingAssembly()
        let shellDir = Path.GetDirectoryName(shellAsm.Location)
        let enginePath = Path.Combine(shellDir, EngineAssemblyName)
        if not (File.Exists(enginePath)) then
            failwithf "EngineInstance: engine assembly not found at %s" enginePath
        enginePath

    /// Install an AssemblyResolve handler so that when the engine assembly
    /// (loaded from bytes) tries to resolve its reference to MMManaged,
    /// it finds the already-loaded shell assembly.
    let mutable private resolverInstalled = false
    let private installAssemblyResolver () =
        if not resolverInstalled then
            let shellAsm = Assembly.GetExecutingAssembly()
            let shellDir = Path.GetDirectoryName(shellAsm.Location)
            let handler = System.ResolveEventHandler(fun _ args ->
                let name = AssemblyName(args.Name)
                // If the engine is looking for MMManaged (the shell), return the already-loaded assembly
                if name.Name = shellAsm.GetName().Name then
                    shellAsm
                else
                    // Try to load from the shell directory (for dependencies like SharpDX, YamlDotNet, etc.)
                    let path = Path.Combine(shellDir, name.Name + ".dll")
                    if File.Exists(path) then
                        Assembly.LoadFrom(path)
                    else
                        null
            )
            System.AppDomain.CurrentDomain.add_AssemblyResolve(handler)
            resolverInstalled <- true

    /// Load (or reload) the engine assembly and initialize it.
    /// Uses Assembly.Load(byte[]) to avoid file locking and to allow loading
    /// multiple versions of the same assembly (each reload creates a new assembly instance).
    let load (logFactory: Logging.LoggerFactory) (context: string) =
        installAssemblyResolver()
        let enginePath = findEngineAssemblyPath()
        let isReload = initialized

        if isReload then
            reloadCount <- reloadCount + 1
            log().Info "Hot-reloading engine assembly (reload #%d) from: %s" reloadCount enginePath
        else
            log().Info "Loading engine assembly from: %s" enginePath

        // Read as bytes to avoid file locking and enable multiple loads
        let bytes = File.ReadAllBytes(enginePath)

        // Also load PDB if available for better stack traces during development
        let pdbPath = Path.ChangeExtension(enginePath, ".pdb")
        let asm =
            if File.Exists(pdbPath) then
                let pdbBytes = File.ReadAllBytes(pdbPath)
                Assembly.Load(bytes, pdbBytes)
            else
                Assembly.Load(bytes)

        log().Info "Loaded engine assembly: %s" asm.FullName

        // Find and instantiate the IEngineModule implementation
        let entryType = asm.GetType(EngineEntryTypeName)
        if entryType = null then
            failwithf "EngineInstance: type '%s' not found in engine assembly" EngineEntryTypeName

        let instance = Activator.CreateInstance(entryType)
        let engineModule =
            match instance with
            | :? IEngineModule as m -> m
            | _ -> failwithf "EngineInstance: type '%s' does not implement IEngineModule" EngineEntryTypeName

        // Initialize the engine module
        engineModule.Initialize logFactory context

        // Get and store the callbacks
        let callbacks = engineModule.GetCallbacks()
        engineCallbacks <- Some(callbacks)
        initialized <- true

        if isReload then
            log().Info "Hot-reload complete (reload #%d). New engine assembly active." reloadCount
        else
            log().Info "Engine module loaded and initialized successfully."

    /// Whether the engine module has been loaded at least once.
    let isLoaded () = initialized
