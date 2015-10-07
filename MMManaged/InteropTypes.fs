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

// Using interop makes the IL unverifiable, disable warning.
#nowarn "9"

/// Contains types that are passed back and forth over interop.  All of these types have strict layout requirements
/// which must match the native code, so changes here must be reflected in that code, otherwise crashamundo 
/// (if you're lucky).
module InteropTypes =
    // the use of multibyte could be a problem here if we need to marshal strings containing unicode characters (i18n paths for example),
    // but currently the unmanaged code doesn't need to know about paths other than the MM install dir, which it already knows.

    /// Run-time configuration data.  Mostly derived from RunConfig.
    [<StructLayout(LayoutKind.Sequential, Pack=8, CharSet=CharSet.Ansi  )>] 
    type ConfData = {
        [<MarshalAs(UnmanagedType.U1)>]
        RunModeFull: bool
        [<MarshalAs(UnmanagedType.U1)>]
        LoadModsOnStart: bool
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=512)>]
        InputProfile: string
    }

    /// Various mod metadata.  Derived from Mesh, DBReference, and DBMod types.
    [<StructLayout(LayoutKind.Sequential, Pack=8, CharSet=CharSet.Unicode)>]
    type ModData = {
        modType: int 
        primType: int
        vertCount: int
        primCount: int
        indexCount: int
        refVertCount: int
        refPrimCount: int
        declSizeBytes: int
        vertSizeBytes: int
        indexElemSizeBytes: int
        // Size must match MaxModTexPathLen from native code
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        tex0Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        tex1Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        tex2Path: string
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=8192)>]
        tex3Path: string
    }

    /// Default value.  Also used as an error return value, since we don't throw exceptions accross interop.
    let EmptyModData = {
        modType = -1
        primType = 0
        vertCount = 0
        primCount = 0
        indexCount = 0
        refVertCount = 0
        refPrimCount = 0
        declSizeBytes = 0
        vertSizeBytes = 0
        indexElemSizeBytes = 0
        tex0Path = ""
        tex1Path = ""
        tex2Path = ""
        tex3Path = ""
    }
    
    [<StructLayout(LayoutKind.Sequential, Pack=8)>]
    /// Data provided by native code for snapshotting.  Most of these fields come from the DrawIndexedPrimitive() 
    /// arguments.  Some are manually filled in by the native code, because managed code can't easily obtain them 
    /// from the SharpDX device.
    type SnapshotData = {
        primType: int32
        baseVertexIndex: int32
        minVertexIndex: uint32
        numVertices: uint32
        startIndex: uint32
        primCount: uint32 

        /// Vertex buffer pointer
        vertDecl:nativeint
        /// Index buffer pointer
        ib:nativeint
    }

    /// Get the mod count (native -> managed callback)
    type GetModCountCB = delegate of unit -> int 
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

    /// Generic return value for failure.  Not much detail here because generally native code can't do anything 
    /// about failures, but this is useful
    /// to help it avoid crashing.  Managed code should typically log detailed exception information when 
    /// failures occurr.
    let GenericFailureCode = 47

    /// Return value when log initialization fails, which "should never happen" but is fundamental enough that
    /// we have a special return code for it.
    let LogInitFailed = 50

