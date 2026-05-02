module IsomFolio.Core.Models

open System

/// SHA-256 hex digest of the file's absolute path — stable identifier across renames tracked in DB
type FileId = string

type AssetFile = {
    Id          : FileId
    Path        : string
    Name        : string    // filename with extension
    Folder      : string    // parent directory path
    Ext         : string    // lowercase, no leading dot e.g. "jpg"
    SizeBytes   : int64
    MTimeUnix   : int64     // UTC Unix timestamp seconds
    IsOrphaned  : bool
    OrphanedAt  : int64 option
}

type ThumbnailState =
    | NotRequested
    | Pending
    | Ready     of cachePath: string
    | Failed    of retryCount: int

type TileSize = Small | Medium | Large  // 128 / 256 / 512 px

type SortField = Name | Date | Size | Ext

type SearchQuery = {
    Text        : string option
    FolderPath  : string option
    Tags        : string list
    Extensions  : string list
    DateRange   : (DateTime * DateTime) option
    SortBy      : SortField
    SortAsc     : bool
}

type AppError =
    | DbError           of message: string
    | ScanError         of message: string
    | ThumbnailError    of fileId: FileId * message: string
    | XmpWriteError     of path: string * message: string
    | WatcherError      of message: string
