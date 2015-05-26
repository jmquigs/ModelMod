namespace ModelMod

open System.Runtime.InteropServices

#nowarn "9"
// ----------------------------------------------------------------------------
// These are types that are passed back to native land  
module InteropTypes =
    // the use of multibyte could be a problem here if we need to marshal strings containing unicode characters (i18n paths for example),
    // but currently the unmanaged code doesn't need to know about paths other than the MM install dir, which it already knows.
    [<StructLayout(LayoutKind.Sequential, Pack=8, CharSet=CharSet.Ansi)>] 
    type ConfData = {
        [<MarshalAs(UnmanagedType.I1)>]
        RunModeFull: bool
        [<MarshalAs(UnmanagedType.ByValTStr, SizeConst=512)>]
        InputProfile: string
    }

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
    type SnapshotData = {
        primType: int32
        baseVertexIndex: int32
        minVertexIndex: uint32
        numVertices: uint32
        startIndex: uint32
        primCount: uint32 

        vertDecl:nativeint
        ib:nativeint
    }

    type GetModCountCB = delegate of unit -> int 
    type GetModDataCB = delegate of int -> ModData
    type FillModDataCB = 
        delegate of 
            modIndex:int *
            declData:nativeptr<byte> *
            declSize:int32 *
            vbData:nativeptr<byte> *
            vbSize:int32 *
            ibData:nativeptr<byte> *
            ibSize:int32 -> int
            
    type TakeSnapshotCB = 
        delegate of 
            device: nativeint *
            snapData: SnapshotData -> int

    let GenericFailureCode = 47
    let LogInitFailed = 50

