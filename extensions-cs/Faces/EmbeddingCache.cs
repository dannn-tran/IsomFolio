using Microsoft.Data.Sqlite;

namespace IsomFolio.Extensions.Faces;

/// Reads and writes face embeddings in the catalog DB (face_embeddings / face_centroids tables).
/// Schema is owned by the Rust host — this class does not CREATE TABLE.
public class EmbeddingCache : IDisposable
{
    private readonly SqliteConnection _conn;

    public EmbeddingCache(string catalogDbPath)
    {
        _conn = new SqliteConnection($"Data Source={catalogDbPath}");
        _conn.Open();

        using var cmd = _conn.CreateCommand();
        // Pragmas are idempotent; tables use IF NOT EXISTS so this is safe when
        // the Rust host has already created them and safe in tests where it hasn't.
        cmd.CommandText = """
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS face_embeddings (
                file_id TEXT NOT NULL,
                mtime   INTEGER NOT NULL,
                bbox_x  REAL NOT NULL,
                bbox_y  REAL NOT NULL,
                bbox_w  REAL NOT NULL,
                bbox_h  REAL NOT NULL,
                vec     BLOB NOT NULL,
                PRIMARY KEY (file_id, bbox_x, bbox_y)
            );
            CREATE INDEX IF NOT EXISTS idx_fe_file ON face_embeddings(file_id, mtime);
            CREATE TABLE IF NOT EXISTS face_centroids (
                cluster_id TEXT PRIMARY KEY,
                vec        BLOB NOT NULL
            );
            """;
        cmd.ExecuteNonQuery();
    }

    public bool IsCached(string fileId, long mtime)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText =
            "SELECT 1 FROM face_embeddings WHERE file_id = @fid AND mtime = @mt LIMIT 1";
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", mtime);
        return cmd.ExecuteScalar() != null;
    }

    public void DeleteStale(string fileId, long mtime)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText =
            "DELETE FROM face_embeddings WHERE file_id = @fid AND mtime != @mt";
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", mtime);
        cmd.ExecuteNonQuery();
    }

    public void InsertEmbedding(string fileId, long mtime, float bboxX, float bboxY, float bboxW, float bboxH, float[] vec)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = """
            INSERT OR REPLACE INTO face_embeddings (file_id, mtime, bbox_x, bbox_y, bbox_w, bbox_h, vec)
            VALUES (@fid, @mt, @bx, @by, @bw, @bh, @vec)
            """;
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", mtime);
        cmd.Parameters.AddWithValue("@bx", bboxX);
        cmd.Parameters.AddWithValue("@by", bboxY);
        cmd.Parameters.AddWithValue("@bw", bboxW);
        cmd.Parameters.AddWithValue("@bh", bboxH);
        cmd.Parameters.AddWithValue("@vec", FloatsToBytes(vec));
        cmd.ExecuteNonQuery();
    }

    public record EmbeddingRow(string FileId, float BboxX, float BboxY, float BboxW, float BboxH, float[] Vec);

    public List<EmbeddingRow> LoadAll()
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText =
            "SELECT file_id, bbox_x, bbox_y, bbox_w, bbox_h, vec FROM face_embeddings";

        var rows = new List<EmbeddingRow>();
        using var reader = cmd.ExecuteReader();
        while (reader.Read())
        {
            rows.Add(new EmbeddingRow(
                reader.GetString(0),
                reader.GetFloat(1),
                reader.GetFloat(2),
                reader.GetFloat(3),
                reader.GetFloat(4),
                BytesToFloats((byte[])reader.GetValue(5))
            ));
        }
        return rows;
    }

    public Dictionary<string, float[]> LoadCentroids()
    {
        var centroids = new Dictionary<string, float[]>();
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "SELECT cluster_id, vec FROM face_centroids";
        using var reader = cmd.ExecuteReader();
        while (reader.Read())
            centroids[reader.GetString(0)] = BytesToFloats((byte[])reader.GetValue(1));
        return centroids;
    }

    public void SaveCentroids(Dictionary<string, float[]> centroids)
    {
        using var tx = _conn.BeginTransaction();
        using (var del = _conn.CreateCommand())
        {
            del.CommandText = "DELETE FROM face_centroids";
            del.ExecuteNonQuery();
        }
        foreach (var (id, centroid) in centroids)
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText =
                "INSERT INTO face_centroids (cluster_id, vec) VALUES (@id, @v)";
            cmd.Parameters.AddWithValue("@id", id);
            cmd.Parameters.AddWithValue("@v", FloatsToBytes(centroid));
            cmd.ExecuteNonQuery();
        }
        tx.Commit();
    }

    private static byte[] FloatsToBytes(float[] v)
    {
        var bytes = new byte[v.Length * 4];
        Buffer.BlockCopy(v, 0, bytes, 0, bytes.Length);
        return bytes;
    }

    private static float[] BytesToFloats(byte[] b)
    {
        var floats = new float[b.Length / 4];
        Buffer.BlockCopy(b, 0, floats, 0, b.Length);
        return floats;
    }

    public void Dispose()
    {
        _conn.Dispose();
        GC.SuppressFinalize(this);
    }
}
