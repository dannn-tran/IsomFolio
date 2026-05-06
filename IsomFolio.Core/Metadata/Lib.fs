namespace IsomFolio.Core.Metadata

open IsomFolio.Core.Metadata.Mac
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.Platform

type FileMetadata = {
    Xmp          : XmpMetadata option
    AppleMetadata: AppleMetadata option
}

module FileMetadata =
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
    let read (filePath: string) : Async<FileMetadata> =
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
