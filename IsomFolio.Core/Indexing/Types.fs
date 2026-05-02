module IsomFolio.Core.Indexing.Types

open IsomFolio.Core.Models


type FileEvent =
    | Created   of path: string
    | Deleted   of path: string
    | Renamed   of oldPath: string * newPath: string
    | Modified  of path: string

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
