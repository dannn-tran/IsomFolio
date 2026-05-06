module IsomFolio.Core.Indexing.Types

open IsomFolio.Core.Models


type FileEvent =
    | Created        of path: string
    | Deleted        of path: string
    | Renamed        of oldPath: string * newPath: string
    | Modified       of path: string
    | SidecarChanged of imagePath: string   // .xmp created or modified → resolved image path
    | SidecarRemoved of imagePath: string   // .xmp deleted → resolved image path

type ReconcileResult = {
    NewOrModified  : string list   // full re-index needed
    Orphaned       : string list   // mark orphaned in DB
    SidecarChanged : string list   // metadata-only refresh
}

type ThumbnailRequest = {
    FileId      : FileId
    FilePath    : string
    Priority    : int       // lower = higher priority; visible tiles = 0
}

type ScanProgress = {
    TotalFound  : int
    Inserted    : int
    FolderName  : string
}

type ScanResult = {
    TotalCount  : int
}
