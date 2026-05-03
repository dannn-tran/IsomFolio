namespace IsomFolio.Core.Metadata

open IsomFolio.Core.Metadata.Mac
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.Platform

type FileMetadata = {
    XmpSidecar: XmpMetadata option
    XmpEmbedded: XmpMetadata option
    AppleMetadata: AppleMetadata option
}

module FileMetadata =
    let getFileMetadata (filePath: string): FileMetadata =
        {
            XmpSidecar =
                filePath
                |> XmpMetadata.parseSidecar
                |> Result.defaultValue None
            XmpEmbedded =
                filePath
                |> XmpMetadata.parseEmbedded
                |> Result.defaultValue None
            AppleMetadata =
                if currentOS = MacOS then
                    filePath
                    |> AppleMetadata.fromFilePath
                    |> Some
                else None
        }
