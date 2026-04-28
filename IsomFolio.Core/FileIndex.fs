module IsomFolio.FileIndex

open System
open System.IO
open System.Security.Cryptography
open System.Text
open IsomFolio.Models
open IsomFolio.PathUtils

let private supportedExtensions =
    Set.ofList [ "jpg"; "jpeg"; "png"; "webp"; "gif" ]

/// SHA-256 of the UTF-8 encoded absolute path — used as stable FileId
let computeFileId (absolutePath: string) : FileId =
    use sha = SHA256.Create()
    sha.ComputeHash(Encoding.UTF8.GetBytes(absolutePath))
    |> Array.map (fun b -> b.ToString("x2"))
    |> String.concat ""

let isSupportedExtension (ext: string) : bool =
    supportedExtensions.Contains(ext.TrimStart('.').ToLowerInvariant())

/// Build an AssetFile from a FileInfo — does not touch the DB
let assetFileFromInfo (fi: FileInfo) : AssetFile =
    let normalizedPath = normalizePath fi.FullName
    let ext = fi.Extension.TrimStart('.').ToLowerInvariant()
    {
        Id         = computeFileId normalizedPath
        Path       = normalizedPath
        Name       = fi.Name
        Folder     = normalizePath fi.DirectoryName
        Ext        = ext
        SizeBytes  = fi.Length
        MTimeUnix  = DateTimeOffset(fi.LastWriteTimeUtc).ToUnixTimeSeconds()
        IsOrphaned = false
        OrphanedAt = None
    }

/// tileSizePx returns the pixel size for a given TileSize
let tileSizePx (ts: TileSize) =
    match ts with
    | Small  -> 128
    | Medium -> 256
    | Large  -> 512
