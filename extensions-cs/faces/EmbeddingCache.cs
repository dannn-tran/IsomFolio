using Microsoft.Data.Sqlite;

namespace IsomFolio.Extensions.Faces;

public class EmbeddingCache : IDisposable
{
    private const string ModelVersion = "scrfd-10g+arcface-w600k-r50-v2";
    private readonly SqliteConnection _conn;

    public EmbeddingCache(string dbPath)
    {
        _conn = new SqliteConnection($"Data Source={dbPath}");
        _conn.Open();

        using var cmd = _conn.CreateCommand();
        cmd.CommandText = """
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS face_embeddings (
                id           INTEGER PRIMARY KEY,
                file_id      TEXT NOT NULL,
                file_mtime   INTEGER NOT NULL,
                model_version TEXT NOT NULL,
                bbox_x       REAL NOT NULL,
                bbox_y       REAL NOT NULL,
                bbox_w       REAL NOT NULL,
                bbox_h       REAL NOT NULL,
                vec          BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_fe_key
                ON face_embeddings (file_id, file_mtime, model_version);
            CREATE TABLE IF NOT EXISTS cluster_centroids (
                cluster_id   TEXT PRIMARY KEY,
                centroid     BLOB NOT NULL
            );
            """;
        cmd.ExecuteNonQuery();
    }

    public bool IsCached(string fileId, long fileMtime)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "SELECT 1 FROM face_embeddings WHERE file_id = @fid AND file_mtime = @mt AND model_version = @mv LIMIT 1";
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", fileMtime);
        cmd.Parameters.AddWithValue("@mv", ModelVersion);
        return cmd.ExecuteScalar() != null;
    }

    public void DeleteStale(string fileId, long fileMtime)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = "DELETE FROM face_embeddings WHERE file_id = @fid AND (file_mtime != @mt OR model_version != @mv)";
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", fileMtime);
        cmd.Parameters.AddWithValue("@mv", ModelVersion);
        cmd.ExecuteNonQuery();
    }

    public void InsertEmbedding(string fileId, long fileMtime, float bboxX, float bboxY, float bboxW, float bboxH, float[] vec)
    {
        using var cmd = _conn.CreateCommand();
        cmd.CommandText = """
            INSERT INTO face_embeddings (file_id, file_mtime, model_version, bbox_x, bbox_y, bbox_w, bbox_h, vec)
            VALUES (@fid, @mt, @mv, @bx, @by, @bw, @bh, @vec)
            """;
        cmd.Parameters.AddWithValue("@fid", fileId);
        cmd.Parameters.AddWithValue("@mt", fileMtime);
        cmd.Parameters.AddWithValue("@mv", ModelVersion);
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
        cmd.CommandText = "SELECT file_id, bbox_x, bbox_y, bbox_w, bbox_h, vec FROM face_embeddings WHERE model_version = @mv";
        cmd.Parameters.AddWithValue("@mv", ModelVersion);

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
        cmd.CommandText = "SELECT cluster_id, centroid FROM cluster_centroids";
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
            del.CommandText = "DELETE FROM cluster_centroids";
            del.ExecuteNonQuery();
        }
        foreach (var (id, centroid) in centroids)
        {
            using var cmd = _conn.CreateCommand();
            cmd.CommandText = "INSERT INTO cluster_centroids (cluster_id, centroid) VALUES (@id, @c)";
            cmd.Parameters.AddWithValue("@id", id);
            cmd.Parameters.AddWithValue("@c", FloatsToBytes(centroid));
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
