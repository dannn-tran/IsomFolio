module IsomFolio.Storage.Db

open System
open System.IO
open Microsoft.Data.Sqlite
open IsomFolio.Models

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
        Id         = reader.GetString(0)
        Path       = reader.GetString(1)
        Name       = reader.GetString(2)
        Folder     = reader.GetString(3)
        Ext        = reader.GetString(4)
        SizeBytes  = reader.GetInt64(5)
        MTimeUnix  = reader.GetInt64(6)
        IsOrphaned = reader.GetInt32(7) = 1
        OrphanedAt = if reader.IsDBNull(8) then None else Some(reader.GetInt64(8))
    }

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
                        (id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at)
                    VALUES
                        (@id, @path, @filename, @folder, @ext, @size, @mtime, @orphaned, @orphanedAt)
                """
                cmd.Parameters.AddWithValue("@id",        f.Id)        |> ignore
                cmd.Parameters.AddWithValue("@path",      f.Path)      |> ignore
                cmd.Parameters.AddWithValue("@filename",  f.Name)      |> ignore
                cmd.Parameters.AddWithValue("@folder",    f.Folder)    |> ignore
                cmd.Parameters.AddWithValue("@ext",       f.Ext)       |> ignore
                cmd.Parameters.AddWithValue("@size",      f.SizeBytes) |> ignore
                cmd.Parameters.AddWithValue("@mtime",     f.MTimeUnix) |> ignore
                cmd.Parameters.AddWithValue("@orphaned",  if f.IsOrphaned then 1 else 0) |> ignore
                cmd.Parameters.AddWithValue("@orphanedAt",
                    match f.OrphanedAt with Some v -> box v | None -> box DBNull.Value) |> ignore
                total <- total + cmd.ExecuteNonQuery()
            tx.Commit()
        return total
    }

let getFilesByFolder (c: SqliteConnection) (folder: string) : Async<AssetFile list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at
            FROM files
            WHERE folder = @folder AND is_orphaned = 0
            ORDER BY filename
        """
        cmd.Parameters.AddWithValue("@folder", folder) |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<AssetFile>()
        while reader.Read() do
            results.Add(readAssetFile reader)
        return results |> Seq.toList
    }

let getFilesByFolderRecursive (c: SqliteConnection) (rootFolder: string) : Async<AssetFile list> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at
            FROM files
            WHERE (folder = @folder OR folder LIKE @prefix) AND is_orphaned = 0
            ORDER BY filename
        """
        cmd.Parameters.AddWithValue("@folder", rootFolder) |> ignore
        cmd.Parameters.AddWithValue("@prefix", rootFolder.TrimEnd('/','\\') + "%") |> ignore
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
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at
            FROM files WHERE id = @id
        """
        cmd.Parameters.AddWithValue("@id", fileId) |> ignore
        use reader = cmd.ExecuteReader()
        if reader.Read() then return Some(readAssetFile reader)
        else return None
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
                extension = @ext, size = @size, modified_time = @mtime
            WHERE path = @oldPath
        """
        cmd.Parameters.AddWithValue("@newId",    newFile.Id)        |> ignore
        cmd.Parameters.AddWithValue("@newPath",  newFile.Path)      |> ignore
        cmd.Parameters.AddWithValue("@filename", newFile.Name)      |> ignore
        cmd.Parameters.AddWithValue("@folder",   newFile.Folder)    |> ignore
        cmd.Parameters.AddWithValue("@ext",      newFile.Ext)       |> ignore
        cmd.Parameters.AddWithValue("@size",     newFile.SizeBytes) |> ignore
        cmd.Parameters.AddWithValue("@mtime",    newFile.MTimeUnix) |> ignore
        cmd.Parameters.AddWithValue("@oldPath",  oldPath)           |> ignore
        cmd.ExecuteNonQuery() |> ignore
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

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

let executeRaw (c: SqliteConnection) (sql: string) : Async<unit> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- sql
        cmd.ExecuteNonQuery() |> ignore
    }

/// Returns all file paths currently in the DB for a given root folder (for reconciliation)
let getIndexedPathsInFolder (c: SqliteConnection) (rootFolder: string) : Async<Map<string, AssetFile>> =
    async {
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            SELECT id, path, filename, folder, extension, size, modified_time, is_orphaned, orphaned_at
            FROM files
            WHERE folder = @folder OR folder LIKE @prefix
        """
        cmd.Parameters.AddWithValue("@folder", rootFolder) |> ignore
        cmd.Parameters.AddWithValue("@prefix", rootFolder.TrimEnd('/','\\') + "%") |> ignore
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.Dictionary<string, AssetFile>()
        while reader.Read() do
            let f = readAssetFile reader
            results[f.Path] <- f
        return results |> Seq.map (fun kv -> kv.Key, kv.Value) |> Map.ofSeq
    }
