module IsomFolio.Core.Search.FTS

open System
open Microsoft.Data.Sqlite
open IsomFolio.Core.Models

// ---------------------------------------------------------------------------
// Query sanitisation
// ---------------------------------------------------------------------------

let private fts5Specials = Set.ofList ['"';'*';'^';'(';')';'[';']';'{';'}';'|';'&';'~';'+';':']

/// Replace FTS5 special chars with spaces (preserves word boundaries), collapse runs,
/// append * for prefix match unless query ends with space
let sanitizeFtsQuery (raw: string) : string =
    let replaced =
        raw.ToCharArray()
        |> Array.map (fun c -> if fts5Specials.Contains(c) then ' ' else c)
        |> String
    let trimmed = replaced.Trim()
    if trimmed = "" then ""
    elif raw.EndsWith(" ") then trimmed   // trailing space = exact token, no prefix
    else trimmed + "*"

// ---------------------------------------------------------------------------
// FTS5 search
// ---------------------------------------------------------------------------

/// Returns FileIds matching the FTS5 query (filename + tags + folder columns)
let searchFts5 (c: SqliteConnection) (rawQuery: string) : Async<string list> =
    async {
        let q = sanitizeFtsQuery rawQuery
        if q = "" then return []
        else
            use cmd = c.CreateCommand()
            cmd.CommandText <- """
                SELECT files.id
                FROM file_index
                JOIN files ON file_index.rowid = files.rowid
                WHERE file_index MATCH @q
                  AND files.is_orphaned = 0
                ORDER BY rank
            """
            cmd.Parameters.AddWithValue("@q", q) |> ignore
            use reader = cmd.ExecuteReader()
            let ids = System.Collections.Generic.List<string>()
            while reader.Read() do
                ids.Add(reader.GetString(0))
            return ids |> Seq.toList
    }

// ---------------------------------------------------------------------------
// FTS index tag sync
// ---------------------------------------------------------------------------

/// Update the tags column in file_index for a file after its tags change.
/// Space-separated tag string per FTS5 convention.
let updateFileIndexTags (c: SqliteConnection) (fileId: FileId) (tags: string list) : Async<unit> =
    async {
        let tagStr = tags |> String.concat " "
        // FTS5 content table update: delete old row then insert new
        use cmd = c.CreateCommand()
        cmd.CommandText <- """
            UPDATE file_index
            SET tags = @tags
            WHERE rowid = (SELECT rowid FROM files WHERE id = @fileId)
        """
        cmd.Parameters.AddWithValue("@tags",   tagStr) |> ignore
        cmd.Parameters.AddWithValue("@fileId", fileId) |> ignore
        cmd.ExecuteNonQuery() |> ignore
    }
