module IsomFolio.Search.QueryEngine

open System
open System.Text
open Microsoft.Data.Sqlite
open IsomFolio.Models
open IsomFolio.Storage

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let private sortColumn (f: SortField) =
    match f with
    | Name -> "filename"
    | Date -> "modified_time"
    | Size -> "size"
    | Ext  -> "extension"

let private readAssetFile (r: SqliteDataReader) : AssetFile =
    {
        Id         = r.GetString(0)
        Path       = r.GetString(1)
        Name       = r.GetString(2)
        Folder     = r.GetString(3)
        Ext        = r.GetString(4)
        SizeBytes  = r.GetInt64(5)
        MTimeUnix  = r.GetInt64(6)
        IsOrphaned = r.GetInt32(7) = 1
        OrphanedAt = if r.IsDBNull(8) then None else Some(r.GetInt64(8))
    }

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Execute a SearchQuery — combines FTS5 candidate selection with SQL filtering.
let executeSearch (query: SearchQuery) : Async<AssetFile list> =
    async {
        let c = Db.connection ()
        use cmd = c.CreateCommand()

        let sql = StringBuilder()
        sql.Append("""
            SELECT f.id, f.path, f.filename, f.folder, f.extension,
                   f.size, f.modified_time, f.is_orphaned, f.orphaned_at
            FROM files f
        """) |> ignore

        // Tag filter via JOIN — each required tag is an INTERSECT-style inner join
        let mutable tagIdx = 0
        for tag in query.Tags do
            sql.Append($" JOIN tags t{tagIdx} ON f.id = t{tagIdx}.file_id AND t{tagIdx}.tag = @tag{tagIdx}") |> ignore
            cmd.Parameters.AddWithValue($"@tag{tagIdx}", tag) |> ignore
            tagIdx <- tagIdx + 1

        sql.Append(" WHERE f.is_orphaned = 0") |> ignore

        // FTS candidate set (if text query present)
        let! ftsEmpty =
            async {
                match query.Text with
                | Some txt when txt.Trim() <> "" ->
                    let! candidateIds = FTS.searchFts5 txt
                    if candidateIds.IsEmpty then
                        return true
                    else
                        let placeholders =
                            candidateIds
                            |> List.mapi (fun i id ->
                                let pname = $"@fts{i}"
                                cmd.Parameters.AddWithValue(pname, id) |> ignore
                                pname)
                            |> String.concat ","
                        sql.Append($" AND f.id IN ({placeholders})") |> ignore
                        return false
                | _ -> return false
            }

        if ftsEmpty then return []
        else

        // Extension filter
        if not query.Extensions.IsEmpty then
            let placeholders =
                query.Extensions
                |> List.mapi (fun i ext ->
                    let pname = $"@ext{i}"
                    cmd.Parameters.AddWithValue(pname, ext.TrimStart('.').ToLowerInvariant()) |> ignore
                    pname)
                |> String.concat ","
            sql.Append($" AND f.extension IN ({placeholders})") |> ignore

        // Date range filter
        match query.DateRange with
        | Some(fromDt, toDt) ->
            let fromUnix = DateTimeOffset(fromDt, TimeSpan.Zero).ToUnixTimeSeconds()
            let toUnix   = DateTimeOffset(toDt,   TimeSpan.Zero).ToUnixTimeSeconds()
            cmd.Parameters.AddWithValue("@fromDt", fromUnix) |> ignore
            cmd.Parameters.AddWithValue("@toDt",   toUnix)   |> ignore
            sql.Append(" AND f.modified_time BETWEEN @fromDt AND @toDt") |> ignore
        | None -> ()

        let dir = if query.SortAsc then "ASC" else "DESC"
        sql.Append($" ORDER BY f.{sortColumn query.SortBy} {dir}") |> ignore

        cmd.CommandText <- sql.ToString()
        use reader = cmd.ExecuteReader()
        let results = System.Collections.Generic.List<AssetFile>()
        while reader.Read() do
            results.Add(readAssetFile reader)
        return results |> Seq.toList
    }
