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
        // Size must match MaxModTexPathLen from native code
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex0Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex1Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex2Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        Tex3Path: string
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
        PixelShaderPath = ""
    }
    
    [<StructLayout(LayoutKind.Sequential)>]
    /// Data provided by native code for snapshotting.  Most of these fields come from the DrawIndexedPrimitive() 
    /// arguments.  Some are manually filled in by the native code, because managed code can't easily obtain them 
    /// from the SharpDX device.
    type SnapshotData = {
        PrimType: int32
        BaseVertexIndex: int32
        MinVertexIndex: uint32
        NumVertices: uint32
        StartIndex: uint32
        PrimCount: uint32 

        /// Vertex buffer pointer
        VertDecl:nativeint
        /// Index buffer pointer
        IndexBuffer:nativeint
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
