module IsomFolio.Core.Storage.Db

open System
open System.IO
open System.Text.Json
open Microsoft.Data.Sqlite
open IsomFolio.Core.Models
open IsomFolio.Core.Metadata
open IsomFolio.Core.Metadata.Mac
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Search

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

let openDatabase (dbPath: string) : Async<SqliteConnection> =
    async {
        Directory.CreateDirectory(Path.GetDirectoryName(dbPath)) |> ignore
        let c = new SqliteConnection($"Data Source={dbPath};Mode=ReadWriteCreate")
        c.Open()
        for pragma in Schema.pragmas.Split(';', StringSplitOptions.RemoveEmptyEntries) do
            let trimmed = pragma.Trim()
            if trimmed.Length > 0 then
                use cmd = c.CreateCommand()
                cmd.CommandText <- trimmed
                cmd.ExecuteNonQuery() |> ignore
        for migration in Schema.migrations do
            try
                use cmd = c.CreateCommand()
                cmd.CommandText <- migration
                cmd.ExecuteNonQuery() |> ignore
            with _ -> ()  // already applied — safe to ignore
        for ddl in Schema.allDdl do
            use cmd = c.CreateCommand()
            cmd.CommandText <- ddl
            cmd.ExecuteNonQuery() |> ignore
        return c
    }

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let private readAssetFile (reader: SqliteDataReader) : AssetFile =
    {
        Id            = reader.GetString(0)
        Path          = reader.GetString(1)
        Name          = reader.GetString(2)
        Folder        = reader.GetString(3)
        Ext           = reader.GetString(4)
        SizeBytes     = reader.GetInt64(5)
        MTimeUnix     = reader.GetInt64(6)
        IsOrphaned    = reader.GetInt32(7) = 1
        OrphanedAt    = if reader.IsDBNull(8) then None else Some(reader.GetInt64(8))
        CreatedAtUnix = reader.GetInt64(9)
    }

let private descendantPrefix (rootFolder: string) =
    rootFolder.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar)
    + string Path.DirectorySeparatorChar
    + "%"

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// Batch upsert — inserts or replaces in transactions of 500. Returns total rows affected.
let upsertFiles (c: SqliteConnection) (files: AssetFile list) : Async<int> =
    async {
        let mutable total = 0
        for batch in files |> List.chunkBySize 500 do
            use tx = c.BeginTransaction()
            for f in batch do
                use cmd = c.CreateCommand()
                cmd.Transaction <- tx
                cmd.CommandText <- """
                    INSERT OR REPLACE INTO files
                        (id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix)
                    VALUES
                        (@id, @path, @filename, @folder, @ext, @size, @mtime, @orphaned, @orphanedAt, @createdAt)
                """
                cmd.Parameters.AddWithValue("@id",        f.Id)              |> ignore
                cmd.Parameters.AddWithValue("@path",      f.Path)            |> ignore
                cmd.Parameters.AddWithValue("@filename",  f.Name)            |> ignore
                cmd.Parameters.AddWithValue("@folder",    f.Folder)          |> ignore
                cmd.Parameters.AddWithValue("@ext",       f.Ext)             |> ignore
                cmd.Parameters.AddWithValue("@size",      f.SizeBytes)       |> ignore
                cmd.Parameters.AddWithValue("@mtime",     f.MTimeUnix)       |> ignore
                cmd.Parameters.AddWithValue("@orphaned",  if f.IsOrphaned then 1 else 0) |> ignore
                cmd.Parameters.AddWithValue("@orphanedAt",
                    match f.OrphanedAt with Some v -> box v | None -> box DBNull.Value) |> ignore
                cmd.Parameters.AddWithValue("@createdAt", f.CreatedAtUnix)   |> ignore
                total <- total + cmd.ExecuteNonQuery()
            tx.Commit()
        return total
    }

let getFilesByFolder (c: SqliteConnection) (folder: string) : Async<AssetFile list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix
            FROM files
            WHERE folder = @folder AND is_orphaned = 0
            ORDER BY filename
        """
        cmd.Parameters.AddWithValue("@folder", normalizePath folder) |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<AssetFile>()
        while reader.Read() do
            results.Add(readAssetFile reader)
        return results |> Seq.toList
    }

let getFilesByFolderRecursive (c: SqliteConnection) (rootFolder: string) : Async<AssetFile list> =
    async {
        let rootFolder = normalizePath rootFolder
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix
            FROM files
            WHERE (folder = @folder OR folder LIKE @prefix) AND is_orphaned = 0
            ORDER BY filename
        """
        cmd.Parameters.AddWithValue("@folder", rootFolder) |> ignore
        cmd.Parameters.AddWithValue("@prefix", descendantPrefix rootFolder) |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<AssetFile>()
        while reader.Read() do
            results.Add(readAssetFile reader)
        return results |> Seq.toList
    }

let getFileById (c: SqliteConnection) (fileId: FileId) : Async<AssetFile option> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix
            FROM files WHERE id = @id
        """
        cmd.Parameters.AddWithValue("@id", fileId) |> ignore
        use reader = cmd.ExecuteReader()
        if reader.Read() then return Some(readAssetFile reader)
        else return None
    }

let deleteFilesByRootFolder (c: SqliteConnection) (rootFolder: string) : Async<unit> =
    async {
        let rootFolder = normalizePath rootFolder
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            DELETE FROM files WHERE folder = @folder OR folder LIKE @prefix
        """
        cmd.Parameters.AddWithValue("@folder", rootFolder) |> ignore
        cmd.Parameters.AddWithValue("@prefix", descendantPrefix rootFolder) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let markOrphaned (c: SqliteConnection) (fileId: FileId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            UPDATE files SET is_orphaned = 1, orphaned_at = @now WHERE id = @id
        """
        cmd.Parameters.AddWithValue("@id",  fileId)                                   |> ignore
        cmd.Parameters.AddWithValue("@now", DateTimeOffset.UtcNow.ToUnixTimeSeconds()) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let unmarkOrphaned (c: SqliteConnection) (fileId: FileId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            UPDATE files SET is_orphaned = 0, orphaned_at = NULL WHERE id = @id
        """
        cmd.Parameters.AddWithValue("@id", fileId) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let updateFilePath (c: SqliteConnection) (oldPath: string) (newFile: AssetFile) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            UPDATE files
            SET id = @newId, path = @newPath, filename = @filename, folder = @folder,
                extension = @ext, size = @size, modified_time = @mtime, created_at_unix = @createdAt
            WHERE path = @oldPath
        """
        cmd.Parameters.AddWithValue("@newId",     newFile.Id)            |> ignore
        cmd.Parameters.AddWithValue("@newPath",   newFile.Path)          |> ignore
        cmd.Parameters.AddWithValue("@filename",  newFile.Name)          |> ignore
        cmd.Parameters.AddWithValue("@folder",    newFile.Folder)        |> ignore
        cmd.Parameters.AddWithValue("@ext",       newFile.Ext)           |> ignore
        cmd.Parameters.AddWithValue("@size",      newFile.SizeBytes)     |> ignore
        cmd.Parameters.AddWithValue("@mtime",     newFile.MTimeUnix)     |> ignore
        cmd.Parameters.AddWithValue("@createdAt", newFile.CreatedAtUnix) |> ignore
        cmd.Parameters.AddWithValue("@oldPath",   oldPath)               |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let deleteFile (c: SqliteConnection) (fileId: FileId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "DELETE FROM files WHERE id = @id"
        cmd.Parameters.AddWithValue("@id", fileId) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let getFolderCounts (c: SqliteConnection) : Async<Map<string, int>> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT folder, COUNT(*) FROM files WHERE is_orphaned = 0 GROUP BY folder"
        use reader = cmd.ExecuteReader()
        let mutable result = Map.empty
        while reader.Read() do
            result <- result |> Map.add (reader.GetString(0)) (reader.GetInt32(1))
        return result
    }

let countOrphans (c: SqliteConnection) : Async<int> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT COUNT(*) FROM files WHERE is_orphaned = 1"
        return cmd.ExecuteScalar() :?> int64 |> int
    }

let purgeOldOrphans (c: SqliteConnection) (olderThanDays: int) : Async<int> =
    async {
        let cutoff = DateTimeOffset.UtcNow.AddDays(-float olderThanDays).ToUnixTimeSeconds()
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            DELETE FROM files WHERE is_orphaned = 1 AND orphaned_at IS NOT NULL AND orphaned_at < @cutoff
        """
        cmd.Parameters.AddWithValue("@cutoff", cutoff) |> ignore
        return cmd.ExecuteNonQuery()
    }

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

/// Replace all tags for a file atomically (DELETE + INSERT in one transaction)
let upsertTags (c: SqliteConnection) (fileId: FileId) (tags: string list) : Async<unit> =
    async {
        use tx = c.BeginTransaction()
        use delCmd = c.CreateCommand()
        delCmd.Transaction <- tx
        delCmd.CommandText <- "DELETE FROM tags WHERE file_id = @fileId"
        delCmd.Parameters.AddWithValue("@fileId", fileId) |> ignore
        delCmd.ExecuteNonQuery() |> ignore
        for tag in tags do
            use insCmd = c.CreateCommand()
            insCmd.Transaction <- tx
            insCmd.CommandText <- "INSERT INTO tags (file_id, tag) VALUES (@fileId, @tag)"
            insCmd.Parameters.AddWithValue("@fileId", fileId) |> ignore
            insCmd.Parameters.AddWithValue("@tag",    tag)    |> ignore
            insCmd.ExecuteNonQuery() |> ignore
        tx.Commit()
    }

let getTagsForFile (c: SqliteConnection) (fileId: FileId) : Async<string list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT tag FROM tags WHERE file_id = @fileId ORDER BY tag"
        cmd.Parameters.AddWithValue("@fileId", fileId) |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<string>()
        while reader.Read() do
            results.Add(reader.GetString(0))
        return results |> Seq.toList
    }

/// Returns all tags with usage counts, sorted by count descending
let getAllTags (c: SqliteConnection) : Async<(string * int) list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT tag, COUNT(*) as cnt FROM tags GROUP BY tag ORDER BY cnt DESC, tag
        """
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<string * int>()
        while reader.Read() do
            results.Add(reader.GetString(0), reader.GetInt32(1))
        return results |> Seq.toList
    }

/// Rename an exact tag across all files. Returns number of rows affected.
let renameTag (c: SqliteConnection) (oldTag: string) (newTag: string) : Async<int> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "UPDATE tags SET tag = @new WHERE tag = @old"
        cmd.Parameters.AddWithValue("@new", newTag) |> ignore
        cmd.Parameters.AddWithValue("@old", oldTag) |> ignore
        return cmd.ExecuteNonQuery()
    }

/// Rename a tag hierarchy prefix across all files.
/// Renames the exact prefix tag and all descendants (e.g. old → new, old/x → new/x).
/// Returns number of rows affected.
let renamePrefixedTags (c: SqliteConnection) (oldPrefix: string) (newPrefix: string) : Async<int> =
    async {
        use tx = c.BeginTransaction()
        use exactCmd = c.CreateCommand()
        exactCmd.Transaction <- tx
        exactCmd.CommandText <- "UPDATE tags SET tag = @new WHERE tag = @old"
        exactCmd.Parameters.AddWithValue("@new", newPrefix) |> ignore
        exactCmd.Parameters.AddWithValue("@old", oldPrefix) |> ignore
        let exactCount = exactCmd.ExecuteNonQuery()
        use prefixCmd = c.CreateCommand()
        prefixCmd.Transaction <- tx
        prefixCmd.CommandText <- "UPDATE tags SET tag = @newPrefix || SUBSTR(tag, @oldLen + 1) WHERE tag LIKE @pattern ESCAPE '\\'"
        prefixCmd.Parameters.AddWithValue("@newPrefix", newPrefix) |> ignore
        prefixCmd.Parameters.AddWithValue("@oldLen", oldPrefix.Length) |> ignore
        let escaped = oldPrefix.Replace("\\", "\\\\").Replace("%", "\\%").Replace("_", "\\_")
        prefixCmd.Parameters.AddWithValue("@pattern", escaped + "/%") |> ignore
        let prefixCount = prefixCmd.ExecuteNonQuery()
        tx.Commit()
        return exactCount + prefixCount
    }

/// Delete a tag and all its descendants across all files. Returns number of rows deleted.
let deleteTagWithDescendants (c: SqliteConnection) (tag: string) : Async<int> =
    async {
        use tx = c.BeginTransaction()
        use exactCmd = c.CreateCommand()
        exactCmd.Transaction <- tx
        exactCmd.CommandText <- "DELETE FROM tags WHERE tag = @tag"
        exactCmd.Parameters.AddWithValue("@tag", tag) |> ignore
        let exactCount = exactCmd.ExecuteNonQuery()
        use prefixCmd = c.CreateCommand()
        prefixCmd.Transaction <- tx
        prefixCmd.CommandText <- "DELETE FROM tags WHERE tag LIKE @pattern ESCAPE '\\'"
        let escaped = tag.Replace("\\", "\\\\").Replace("%", "\\%").Replace("_", "\\_")
        prefixCmd.Parameters.AddWithValue("@pattern", escaped + "/%") |> ignore
        let prefixCount = prefixCmd.ExecuteNonQuery()
        tx.Commit()
        return exactCount + prefixCount
    }

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

let executeRaw (c: SqliteConnection) (sql: string) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- sql
        cmd.ExecuteNonQuery() |> ignore
    }

/// Returns all FileIds currently in the DB (for thumbnail cache sweep)
let getAllFileIds (c: SqliteConnection) : Async<string list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT id FROM files"
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<string>()
        while reader.Read() do
            results.Add(reader.GetString(0))
        return results |> Seq.toList
    }

/// Returns all file paths currently in the DB for a given root folder (for reconciliation)
let getIndexedPathsInFolder (c: SqliteConnection) (rootFolder: string) : Async<Map<string, AssetFile>> =
    async {
        let rootFolder = normalizePath rootFolder
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at, created_at_unix
            FROM files
            WHERE folder = @folder OR folder LIKE @prefix
        """
        cmd.Parameters.AddWithValue("@folder", rootFolder) |> ignore
        cmd.Parameters.AddWithValue("@prefix", descendantPrefix rootFolder) |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.Dictionary<string, AssetFile>()
        while reader.Read() do
            let f = readAssetFile reader
            results[f.Path] <- f
        return results |> Seq.map (fun kv -> kv.Key, kv.Value) |> Map.ofSeq
    }

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

let private jsonList (xs: string list) = JsonSerializer.Serialize(xs)

/// Upsert XMP/Apple metadata for a file. Updates the FTS tags column with
/// subjects + apple_tags merged with user-defined tags (deduped).
let upsertMetadata (c: SqliteConnection) (fileId: FileId) (meta: EmbeddedMetadata) : Async<unit> =
    async {
        let xmpCore = meta.Xmp |> Option.map (fun x -> x.Core)
        let xmpDc   = meta.Xmp |> Option.map (fun x -> x.DublinCore)
        let apple   = meta.AppleMetadata

        let subjects  = xmpDc |> Option.map (fun x -> x.Subject) |> Option.defaultValue []
        let creator   = xmpDc |> Option.map (fun x -> x.Creator) |> Option.defaultValue []
        let appleTags = apple  |> Option.map (fun a -> a.UserTags |> List.map (fun t -> t.Text)) |> Option.defaultValue []

        use tx = c.BeginTransaction()

        use cmd = c.CreateCommand()
        cmd.Transaction <- tx
        cmd.CommandText <- """
            INSERT OR REPLACE INTO metadata (file_id, rating, label, title, description, creator, subjects, apple_tags)
            VALUES (@fileId, @rating, @label, @title, @description, @creator, @subjects, @appleTags)
        """
        let optBox (v: 'a option) = match v with Some x -> box x | None -> box DBNull.Value
        cmd.Parameters.AddWithValue("@fileId",      fileId) |> ignore
        cmd.Parameters.AddWithValue("@rating",      xmpCore |> Option.bind (fun x -> x.Rating)      |> optBox) |> ignore
        cmd.Parameters.AddWithValue("@label",       xmpCore |> Option.bind (fun x -> x.Label)       |> optBox) |> ignore
        cmd.Parameters.AddWithValue("@title",       xmpDc   |> Option.bind (fun x -> x.Title)       |> optBox) |> ignore
        cmd.Parameters.AddWithValue("@description", xmpDc   |> Option.bind (fun x -> x.Description) |> optBox) |> ignore
        cmd.Parameters.AddWithValue("@creator",     jsonList creator)   |> ignore
        cmd.Parameters.AddWithValue("@subjects",    jsonList subjects)  |> ignore
        cmd.Parameters.AddWithValue("@appleTags",   jsonList appleTags) |> ignore
        cmd.ExecuteNonQuery() |> ignore

        tx.Commit()

        // Merge metadata text with user-defined tags for FTS search
        let! userTags = getTagsForFile c fileId
        let ftsTokens =
            [ yield! userTags
              yield! subjects
              yield! appleTags
              yield! (xmpDc |> Option.bind (fun x -> x.Title)       |> Option.toList)
              yield! (xmpDc |> Option.bind (fun x -> x.Description) |> Option.toList)
              yield! creator ]
            |> List.distinct
        do! FTS.updateFileIndexTags c fileId ftsTokens
    }

// ---------------------------------------------------------------------------
// Albums
// ---------------------------------------------------------------------------

let private sortFieldToStr = function
    | Name -> "Name" | Date -> "Date" | Size -> "Size" | Ext -> "Ext"

let private strToSortField = function
    | "Name" -> Name | "Date" -> Date | "Size" -> Size | _ -> Ext

let serializeSearchQuery (q: SearchQuery) : string =
    JsonSerializer.Serialize {|
        text            = q.Text       |> Option.defaultValue null
        folderPath      = q.FolderPath |> Option.defaultValue null
        folderRecursive = q.FolderRecursive
        tags            = q.Tags       |> Array.ofList
        extensions      = q.Extensions |> Array.ofList
        dateFrom        = q.DateRange  |> Option.map (fun (f, _) -> f.ToString("O")) |> Option.defaultValue null
        dateTo          = q.DateRange  |> Option.map (fun (_, t) -> t.ToString("O")) |> Option.defaultValue null
        sortBy          = sortFieldToStr q.SortBy
        sortAsc         = q.SortAsc
    |}

let deserializeSearchQuery (json: string) : SearchQuery =
    use doc = JsonDocument.Parse(json)
    let root = doc.RootElement
    let strOpt (name: string) =
        match root.TryGetProperty(name) with
        | true, el when el.ValueKind <> JsonValueKind.Null -> Some(el.GetString())
        | _ -> None
    let strVal (name: string) =
        match root.TryGetProperty(name) with
        | true, el when el.ValueKind <> JsonValueKind.Null -> el.GetString()
        | _ -> null
    let arrVal (name: string) =
        match root.TryGetProperty(name) with
        | true, el when el.ValueKind = JsonValueKind.Array ->
            [ for e in el.EnumerateArray() do yield e.GetString() ]
        | _ -> []
    let boolVal (name: string) =
        match root.TryGetProperty(name) with
        | true, el -> el.GetBoolean()
        | _ -> false
    let dateRange =
        match strOpt "dateFrom", strOpt "dateTo" with
        | Some f, Some t ->
            try Some(DateTime.Parse(f), DateTime.Parse(t))
            with _ -> None
        | _ -> None
    {
        Text            = strOpt "text"
        FolderPath      = strOpt "folderPath"
        FolderRecursive = boolVal "folderRecursive"
        Tags            = arrVal "tags"
        Extensions      = arrVal "extensions"
        DateRange       = dateRange
        SortBy          = strToSortField (strVal "sortBy")
        SortAsc         = boolVal "sortAsc"
    }

let private readAlbum (reader: SqliteDataReader) : Album =
    let id        = reader.GetString(0)
    let name      = reader.GetString(1)
    let kind      = reader.GetString(2)
    let queryJson = if reader.IsDBNull(3) then None else Some(reader.GetString(3))
    let sortOrder = reader.GetInt32(4)
    let albumKind =
        match kind with
        | "smart" ->
            queryJson
            |> Option.map (fun json -> Smart(deserializeSearchQuery json))
            |> Option.defaultValue Manual
        | _ -> Manual
    { Id = id; Name = name; Kind = albumKind; SortOrder = sortOrder }

let createAlbum (c: SqliteConnection) (album: Album) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            INSERT INTO albums (id, name, kind, query_json, sort_order)
            VALUES (@id, @name, @kind, @queryJson, @sortOrder)
        """
        let kind, queryJson =
            match album.Kind with
            | Smart q -> "smart", box (serializeSearchQuery q)
            | Manual  -> "manual", box DBNull.Value
        cmd.Parameters.AddWithValue("@id",        album.Id)        |> ignore
        cmd.Parameters.AddWithValue("@name",      album.Name)      |> ignore
        cmd.Parameters.AddWithValue("@kind",      kind)            |> ignore
        cmd.Parameters.AddWithValue("@queryJson", queryJson)       |> ignore
        cmd.Parameters.AddWithValue("@sortOrder", album.SortOrder) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let getAllAlbums (c: SqliteConnection) : Async<Album list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT id, name, kind, query_json, sort_order FROM albums ORDER BY sort_order, name"
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<Album>()
        while reader.Read() do
            results.Add(readAlbum reader)
        return results |> Seq.toList
    }

let renameAlbum (c: SqliteConnection) (albumId: AlbumId) (newName: string) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "UPDATE albums SET name = @name WHERE id = @id"
        cmd.Parameters.AddWithValue("@name", newName)  |> ignore
        cmd.Parameters.AddWithValue("@id",   albumId)  |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let deleteAlbum (c: SqliteConnection) (albumId: AlbumId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "DELETE FROM albums WHERE id = @id"
        cmd.Parameters.AddWithValue("@id", albumId) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let updateSmartAlbumQuery (c: SqliteConnection) (albumId: AlbumId) (query: SearchQuery) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "UPDATE albums SET query_json = @json WHERE id = @id AND kind = 'smart'"
        cmd.Parameters.AddWithValue("@json", serializeSearchQuery query) |> ignore
        cmd.Parameters.AddWithValue("@id",   albumId)                   |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let addFileToAlbum (c: SqliteConnection) (albumId: AlbumId) (fileId: FileId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            INSERT OR IGNORE INTO album_files (album_id, file_id, added_at)
            VALUES (@albumId, @fileId, @now)
        """
        cmd.Parameters.AddWithValue("@albumId", albumId)                                   |> ignore
        cmd.Parameters.AddWithValue("@fileId",  fileId)                                    |> ignore
        cmd.Parameters.AddWithValue("@now",     DateTimeOffset.UtcNow.ToUnixTimeSeconds()) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let removeFileFromAlbum (c: SqliteConnection) (albumId: AlbumId) (fileId: FileId) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "DELETE FROM album_files WHERE album_id = @albumId AND file_id = @fileId"
        cmd.Parameters.AddWithValue("@albumId", albumId) |> ignore
        cmd.Parameters.AddWithValue("@fileId",  fileId)  |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }

let countAlbumFiles (c: SqliteConnection) (albumId: AlbumId) : Async<int> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- "SELECT COUNT(*) FROM album_files WHERE album_id = @albumId"
        cmd.Parameters.AddWithValue("@albumId", albumId) |> ignore
        return cmd.ExecuteScalar() :?> int64 |> int
    }

let getMetadata (c: SqliteConnection) (fileId: FileId) : Async<EmbeddedMetadata option> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT rating, label, title, description, creator, subjects, apple_tags
            FROM metadata WHERE file_id = @fileId
        """
        cmd.Parameters.AddWithValue("@fileId", fileId) |> ignore
        use reader = cmd.ExecuteReader()
        if not (reader.Read()) then return None
        else
            let strOpt i = if reader.IsDBNull(i) then None else Some (reader.GetString(i))
            let intOpt i = if reader.IsDBNull(i) then None else Some (reader.GetInt32(i))
            let listOf i =
                if reader.IsDBNull(i) then []
                else
                    try JsonSerializer.Deserialize<string list>(reader.GetString(i))
                    with _ -> []

            let rating      = intOpt 0
            let label       = strOpt 1
            let title       = strOpt 2
            let description = strOpt 3
            let creator     = listOf 4
            let subjects    = listOf 5
            let appleTags   = listOf 6 |> List.map (fun t -> { Text = t; ColorIdx = 0 })

            let hasXmp =
                Option.isSome rating || Option.isSome label ||
                Option.isSome title  || Option.isSome description ||
                not creator.IsEmpty  || not subjects.IsEmpty

            let xmp =
                if hasXmp then
                    Some {
                        Core = { XmpCore.empty with Rating = rating; Label = label }
                        DublinCore = { DublinCore.empty with Title = title; Description = description; Creator = creator; Subject = subjects }
                    }
                else None

            let apple =
                if appleTags.IsEmpty then None
                else Some { UserTags = appleTags }

            return Some { Xmp = xmp; AppleMetadata = apple }
    }
