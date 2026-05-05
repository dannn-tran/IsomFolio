namespace IsomFolio.Core.Metadata

open System.Threading.Tasks
open IsomFolio.Core.Metadata.Mac
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.Platform

type FileMetadata = {
    XmpSidecar: XmpMetadata option
    XmpEmbedded: XmpMetadata option
    AppleMetadata: AppleMetadata option
}

module FileMetadata =
    let private getXmpSidecar filePath =
        filePath
        |> XmpMetadata.getSidecar
        |> Result.defaultValue None
    let private getXmpEmbedded filePath =
        filePath
        |> XmpMetadata.getEmbedded
        |> Result.defaultValue None
    
    let getFileMetadata (filePath: string): FileMetadata =
        {
            XmpSidecar = getXmpSidecar filePath
            XmpEmbedded = getXmpEmbedded filePath
            AppleMetadata =
                if currentOS = MacOS then
                    filePath
                    |> AppleMetadata.fromFilePath
                    |> Some
                else None
        }
        
    let getFileMetadataAsync (filePath: string): Task<FileMetadata> =
        task {
            let xmpSidecarTask =
                Task.Run(fun () -> getXmpSidecar filePath)
            let xmpEmbeddedTask =
                Task.Run(fun () -> getXmpEmbedded filePath)
            let appleMetadataTask =
                if currentOS = MacOS then
                    Task.Run(fun () -> filePath |> AppleMetadata.fromFilePath |> Some)
                else
                    Task.FromResult(None)
                    
            let! xmpSidecar = xmpSidecarTask
            let! xmpEmbedded = xmpEmbeddedTask
            let! appleMetadata = appleMetadataTask
            
            return {
                XmpSidecar = xmpSidecar
                XmpEmbedded = xmpEmbedded
                AppleMetadata = appleMetadata
            }
        }
