namespace IsomFolio.Core.Metadata

open System
open System.IO
open IsomFolio.Core.Metadata.Mac
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.Platform

type FileSystemInfo = {
    CreatedAt  : DateTimeOffset
    ModifiedAt : DateTimeOffset
    SizeBytes  : int64
}

/// Merged operational view. Sidecar wins over embedded; populated during scan and persisted to DB.
type EmbeddedMetadata = {
    Xmp          : XmpMetadata option
    AppleMetadata: AppleMetadata option
}

/// Full provenance view — all sources read unconditionally, never derived from EmbeddedMetadata.
/// Used only on demand (source view UI, staleness check).
type MetadataSources = {
    Sidecar    : XmpMetadata option
    Embedded   : XmpMetadata option
    Apple      : AppleMetadata option
    FileSystem : FileSystemInfo
}

module EmbeddedMetadata =
    let private xmpSidecar filePath =
        async { return filePath |> XmpMetadata.getSidecar |> Result.defaultValue None }

    let private xmpEmbedded filePath =
        async { return filePath |> XmpMetadata.getEmbedded |> Result.defaultValue None }

    let private appleMeta filePath =
        async {
            return
                if currentOS = MacOS then filePath |> AppleMetadata.fromFilePath |> Some
                else None
        }

    /// Reads XMP and Apple metadata for a file.
    /// Sidecar is checked first; embedded is only read when no sidecar exists.
    /// Apple metadata runs in parallel with the XMP chain.
    /// Caller decides execution: Async.StartAsTask for async contexts,
    /// Async.RunSynchronously only on thread-pool threads (never on the UI thread).
    let read (filePath: string) : Async<EmbeddedMetadata> =
        async {
            let! appleChild = Async.StartChild(appleMeta filePath)
            let! sidecar = xmpSidecar filePath
            let! xmp =
                match sidecar with
                | Some _ -> async { return sidecar }
                | None   -> xmpEmbedded filePath
            let! apple = appleChild
            return { Xmp = xmp; AppleMetadata = apple }
        }

    /// Reads all sources in parallel with no sidecar-wins logic.
    /// Takes a FileInfo to avoid a redundant stat — caller must already have it.
    /// For on-demand source view only; not used in the scan hot path.
    let readSources (filePath: string) (fi: FileInfo) : Async<MetadataSources> =
        async {
            let! sidecarChild = Async.StartChild(xmpSidecar filePath)
            let! embeddedChild = Async.StartChild(xmpEmbedded filePath)
            let! appleChild =
                Async.StartChild(
                    async {
                        return
                            if currentOS = MacOS then filePath |> AppleMetadata.fromFilePath |> Some
                            else None
                    })
            let! sidecar = sidecarChild
            let! embedded = embeddedChild
            let! apple = appleChild
            return {
                Sidecar    = sidecar
                Embedded   = embedded
                Apple      = apple
                FileSystem = {
                    CreatedAt  = DateTimeOffset(fi.CreationTimeUtc)
                    ModifiedAt = DateTimeOffset(fi.LastWriteTimeUtc)
                    SizeBytes  = fi.Length
                }
            }
        }

