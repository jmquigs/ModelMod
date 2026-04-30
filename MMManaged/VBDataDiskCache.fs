namespace ModelMod

open System
open System.IO
open System.IO.Compression
open System.Security.Cryptography
open System.Text

open MBrace.FsPickler

open CoreTypes
open BinCacheTypes

/// Binary disk cache for the bytes that fillModData writes into the
/// vertex buffer (and, for DX9, the bytes written into the vertex
/// declaration buffer).  On a hit, the caller can blit the cached bytes
/// in one shot instead of re-walking the mod triangle list.
///
/// Modeled on MeshDiskCache / MeshRelDiskCache; same FsPickler / GZip
/// approach and same FSharp.Core sensitivity caveats.
module VBDataDiskCache =
    let private log() = Logging.getLogger("VBDataDiskCache")
    let private ser = FsPickler.CreateBinarySerializer()
    let private cacheVersion = 1

    /// Identifies every input that can affect the bytes fillModData
    /// writes into the destination vertex buffer.  If any of these change
    /// the cache entry must be discarded.
    type VBDataSig = {
        Mod: MeshSig
        Ref: MeshSig
        VertSizeBytes: int
        VbSize: int
        DeclSize: int
        DeclHash: string
        Context: string
        WeightMode: WeightMode
        VecEncoding: string
        BlendIndexInColor1: bool
        BlendWeightInColor2: bool
        ReverseNormals: bool
    }

    type Entry = {
        Version: int
        Sig: VBDataSig
        Decl: byte[]
        Vb: byte[]
    }

    let private sha256Hex (bytes: byte[]) =
        use sha = SHA256.Create()
        sha.ComputeHash(bytes)
        |> Seq.map (fun b -> b.ToString("x2"))
        |> String.concat ""

    /// Hash of arbitrary string content (used for D3D11 element list).
    let hashString (s: string) =
        sha256Hex (Encoding.UTF8.GetBytes(s))

    /// Hash of a raw byte buffer (used for D3D9 declaration data).
    let hashBytes (b: byte[]) = sha256Hex b

    let private key (modName: string) (refName: string) (declHash: string) (context: string) (vertSize: int) =
        let s =
            sprintf "%s|%s|%s|%s|%d"
                (modName.ToLowerInvariant())
                (refName.ToLowerInvariant())
                declHash
                context
                vertSize
        hashString s

    let relPath (cacheDir: string) (modName: string) (refName: string) (declHash: string)
                (context: string) (vertSize: int) =
        Path.Combine(cacheDir, "VBData", key modName refName declHash context vertSize + ".bin.gz")

    let tryLoad (cacheDir: string) (modName: string) (refName: string) (sg: VBDataSig) : Entry option =
        let p = relPath cacheDir modName refName sg.DeclHash sg.Context sg.VertSizeBytes
        if not (File.Exists(p)) then None
        else
            use fs = File.OpenRead(p)
            use gz = new GZipStream(fs, CompressionMode.Decompress)
            let e = ser.Deserialize<Entry>(gz)
            if e.Version <> cacheVersion then None
            elif e.Sig <> sg then None
            elif e.Vb.Length <> sg.VbSize then None
            elif e.Decl.Length <> sg.DeclSize then None
            else Some e

    let save (cacheDir: string) (modName: string) (refName: string)
             (sg: VBDataSig) (decl: byte[]) (vb: byte[]) =
        let dir = Path.Combine(cacheDir, "VBData")
        Directory.CreateDirectory(dir) |> ignore
        let p = relPath cacheDir modName refName sg.DeclHash sg.Context sg.VertSizeBytes
        let tmp = p + ".tmp"

        log().Info "[vbdatacache]: creating bincache entry: %A for mod=%A ref=%A" tmp modName refName

        let e =
            {
                Entry.Version = cacheVersion
                Sig = sg
                Decl = decl
                Vb = vb
            }

        use fs = File.Create(tmp)
        use gz = new GZipStream(fs, CompressionMode.Compress)
        ser.Serialize(gz, e)
        gz.Close()
        fs.Close()

        if File.Exists(p) then File.Delete(p)
        File.Move(tmp, p)
