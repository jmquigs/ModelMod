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

// Using interop makes the IL unverifiable, disable warning.
#nowarn "9"

/// Contains types that are passed back and forth over interop.  All of these types have strict layout requirements
/// which must match the native code, so changes here must be reflected in that code, otherwise crashamundo
/// (if you're lucky).
module InteropTypes =
    // the use of multibyte could be a problem here if we need to marshal strings containing unicode characters (i18n paths for example),
    // but currently the unmanaged code doesn't need to know about paths other than the MM install dir, which it already knows.

    /// Run-time configuration data.  Mostly derived from RunConfig.
    [<StructLayout(LayoutKind.Sequential, CharSet=CharSet.Ansi  )>]
    type ConfData = {
        [<MarshalAs(UnmanagedType.U1)>]
        RunModeFull: bool
        [<MarshalAs(UnmanagedType.U1)>]
        LoadModsOnStart: bool
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=512)>]
        InputProfile: string
        MinimumFPS: int
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=512)>]
        ProfileKey: string
    }

    /// A struct containing a pointer to unmanaged memory and the size of the data.
    [<Struct>]
    [<StructLayout(LayoutKind.Sequential)>]
    type NativeMemoryBuffer =
        val mutable Data:System.IntPtr
        val mutable Size:int32

    /// Various mod metadata.  Derived from Mesh, DBReference, and DBMod types.
    [<StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)>]
    type ModData = {
        ModType: int
        PrimType: int
        VertCount: int
        PrimCount: int
        IndexCount: int
        RefVertCount: int
        RefPrimCount: int
        DeclSizeBytes: int
        VertSizeBytes: int
        IndexElemSizeBytes: int
        // official end of "mod numbers" in native struct
        UpdateTangentSpace: int
        // Size must match MaxModTexPathLen from native code
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex0Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex1Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex2Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex3Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=1024)>]
        ModName: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=1024)>]
        ParentModName: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        PixelShaderPath: string
    }

    /// Default value.  Also used as an error return value, since we don't throw exceptions accross interop.
    let EmptyModData = {
        ModType = -1
        PrimType = 0
        VertCount = 0
        PrimCount = 0
        IndexCount = 0
        RefVertCount = 0
        RefPrimCount = 0
        DeclSizeBytes = 0
        VertSizeBytes = 0
        IndexElemSizeBytes = 0
        Tex0Path = ""
        Tex1Path = ""
        Tex2Path = ""
        Tex3Path = ""
        ModName = ""
        ParentModName = ""
        PixelShaderPath = ""
        UpdateTangentSpace = -1
    }

    [<StructLayout(LayoutKind.Sequential, Pack=4)>]
    type D3D9SnapshotRendData = {
        /// Vertex declaration pointer
        VertDecl:nativeint
        /// Index buffer pointer
        IndexBuffer:nativeint
    }
    [<StructLayout(LayoutKind.Sequential, Pack=4)>]
    type D3D11SnapshotRendData = {
          /// Vertex declaration pointer
          LayoutElems:nativeptr<byte>
          /// Index buffer data pointer 
          IndexData:nativeptr<byte>
          /// Vertex buffer data pointer 
          VertexData:nativeptr<byte>
          /// Size of the layout elements in bytes
          LayoutElemsSizeBytes:uint64
          /// Size of the index data in bytes
          IndexDataSizeBytes:uint64
          /// Size of the vertex data in bytes 
          VertexDataSizeBytes:uint64 
          /// Size of the indices in the index buffer
          IndexSizeBytes:uint32 
          /// Size of the verts in the vertex buffer
          VertSizeBytes:uint32
      }

    [<StructLayout(LayoutKind.Explicit, Pack=4)>]
    /// This represents a C-style union in the native code.  Only one of these fields will be valid at a time.
    /// The native representation for these fields ensures that both structs are the same size, using padding 
    /// if necessary.  The padding is not declared in the managed code (since it depends on whether native is 32 or 64 bit 
    /// and .net marshalling doesn't appear to have a good way to represent that kind of variation).  Although the managed 
    /// code does not declare the padding, since the size of this type is equal to the larger of the two fields, 
    /// the size in managed code must always equal the native size.
    /// 
    /// Pack=4 is used to make the alignment predictable for both 32/64 bit, there is a performance penalty on 64 bit,
    /// but since this struct is accessed just a few times per snapshot, it should not matter.  
    /// I tried using Pack=8 but it was causing unexpected marshalling errors in 32 bit.
    type SnapshotRendData = {
        [<FieldOffset(0)>]
        d3d9: D3D9SnapshotRendData

        [<FieldOffset(0)>]
        d3d11: D3D11SnapshotRendData
    }

    [<StructLayout(LayoutKind.Sequential, Pack=4)>]
    /// Data provided by native code for snapshotting.  Most of these fields come from the DrawIndexedPrimitive()
    /// arguments.  Some are manually filled in by the native code, because managed code can't easily obtain them
    /// from the SharpDX device.
    type SnapshotData = {
        SDSize: uint32
        PrimType: int32
        BaseVertexIndex: int32
        MinVertexIndex: uint32
        NumVertices: uint32
        StartIndex: uint32
        PrimCount: uint32
        RendData: SnapshotRendData
    }

    [<StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)>]
    type SnapshotResult = {
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Directory: string

        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        SnapFilePrefix: string

        DirectoryLen: int32
        SnapFilePrefixLen: int32
    }

    /// Get the mod count (native -> managed callback)
    type GetModCountCB = delegate of unit -> int

    /// Get the current loading state
    type GetLoadingStateCB = delegate of unit -> int

    /// Get the mod data for the mod at specified index, where index is in range 0..(modcount-1).
    /// (native -> managed callback).  If index is out of range, EmptyModData is returned.
    type GetModDataCB = delegate of int -> ModData
    /// Fill buffers associated with mod at specified index, where index is in range 0..(modcount-1).
    /// The native pointers are the destination buffers.  An exception will be logged and GenericFailureCode
    /// returned if an error occurs (for instance, buffers are too small).
    type FillModDataCB =
        delegate of
            modIndex:int *
            declData:nativeptr<byte> *
            declSize:int32 *
            vbData:nativeptr<byte> *
            vbSize:int32 *
            ibData:nativeptr<byte> *
            ibSize:int32 -> int

    /// Take a snapshot.  Managed code is responsible for all the work here, including writing the files to disk
    /// and performing any transformations.  Returns 0 on success or logs an exception and returns
    /// GenericFailureCode on error.
    type TakeSnapshotCB =
        delegate of
            device: nativeint *
            snapData: SnapshotData -> int

    type GetSnapshotResultCB = delegate of unit -> SnapshotResult

    /// Current load state.  Mod data is loaded asynchronously to minimize blocking of the
    /// render thread.
    type AsyncLoadState =
        NotStarted
        | Pending
        | InProgress
        | Complete

    /// Generic return value for failure.  Not much detail here because generally native code can't do anything
    /// about failures, but this is useful
    /// to help it avoid crashing.  Managed code should typically log detailed exception information when
    /// failures occurr.
    let GenericFailureCode = 47

    /// Returned when native code with a different version attempts to initialize this managed dll.
    /// Versions must be an exact match, otherwise a crash is possible.
    let NativeCodeMismatch = 48

    /// Return value when log initialization fails, which "should never happen" but is fundamental enough that
    /// we have a special return code for it.
    let LogInitFailed = 50

    /// Integer representation of AsyncLoadState for native code use.
    /// Must match #defines in Interop.h
    let AsyncLoadNotStarted = 51
    let AsyncLoadPending = 52
    let AsyncLoadInProgress = 53
    let AsyncLoadComplete = 54

    let Assplosion = 666

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
        GetSnapshotResult: InteropTypes.GetSnapshotResultCB
    }

module NativeImportsAsD3D11 =
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern int OnInitialized(MMNative.ManagedCallbacks callback, uint64 globalStateAddress)
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall) >]
    /// Saves a dds texture from the specified texture stage.  This is handled by native code, which has
    /// direct access to the D3DX library; no easy equivalent here in managed land.
    extern [<MarshalAs(UnmanagedType.U1)>]bool SaveTexture(int index, [<MarshalAs(UnmanagedType.LPWStr)>]string filepath)
    [< DllImport("d3d11.dll", CallingConvention = CallingConvention.StdCall) >]
    /// Fills in the specified NativeMemoryBuffer with the current pixel shader code.
    /// WARNING: the argument must be an address of a NativeMemoryBuffer.  Otherwise it will crash.
    /// WARNING: the data address in the memory buffer is only valid until the next call to GetPixelShader().
    /// If you call this function twice in succession and then use the results from the first call, it will crash.
    extern [<MarshalAs(UnmanagedType.U1)>]bool GetPixelShader(System.IntPtr buffer)

module NativeImportsAsD3D9 =
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern int OnInitialized(MMNative.ManagedCallbacks callback, uint64 globalStateAddress)
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall) >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall) >]
    /// Saves a dds texture from the specified texture stage.  This is handled by native code, which has
    /// direct access to the D3DX library; no easy equivalent here in managed land.
    extern [<MarshalAs(UnmanagedType.U1)>]bool SaveTexture(int index, [<MarshalAs(UnmanagedType.LPWStr)>]string filepath)
    [< DllImport("d3d9.dll", CallingConvention = CallingConvention.StdCall) >]
    /// Fills in the specified NativeMemoryBuffer with the current pixel shader code.
    /// WARNING: the argument must be an address of a NativeMemoryBuffer.  Otherwise it will crash.
    /// WARNING: the data address in the memory buffer is only valid until the next call to GetPixelShader().
    /// If you call this function twice in succession and then use the results from the first call, it will crash.
    extern [<MarshalAs(UnmanagedType.U1)>]bool GetPixelShader(System.IntPtr buffer)

module NativeImportsAsMMNative =
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern int OnInitialized(MMNative.ManagedCallbacks callback, uint64 globalStateAddress)
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern void LogInfo([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern void LogWarn([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern void LogError([<MarshalAs(UnmanagedType.LPStr)>]string category, [<MarshalAs(UnmanagedType.LPStr)>]string s)
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern [<MarshalAs(UnmanagedType.U1)>]bool SaveTexture(int index, [<MarshalAs(UnmanagedType.LPWStr)>]string filepath)
    [< DllImport("mm_native.dll", CallingConvention = CallingConvention.StdCall ) >]
    extern [<MarshalAs(UnmanagedType.U1)>]bool GetPixelShader(System.IntPtr buffer)