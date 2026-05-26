using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces;

public static class Program
{
    private record Config(float Eps = 0.4f, int MinPts = 2);

    public static void Main()
    {
        var output = Console.Out;

        Protocol.SendHello(output, ["cluster_faces"]);

        var config = LoadConfig(output);
        var modelsDir = Environment.GetEnvironmentVariable("ISOMFOLIO_MODELS_DIR") ?? ".";

        Protocol.EmitLog(output, "info", "loading face models…");

        string detPath, recPath;
        try
        {
            (detPath, recPath) = ModelDownloader.EnsureModels(modelsDir, output);
        }
        catch (Exception ex)
        {
            Protocol.EmitLog(output, "error", $"model init failed: {ex.Message}");
            return;
        }

        using var detector = new FaceDetector(detPath);
        using var recognizer = new FaceRecognizer(recPath);

        var stateDbPath = Path.Combine(modelsDir, "faces", "state.db");
        Directory.CreateDirectory(Path.GetDirectoryName(stateDbPath)!);
        using var cache = new EmbeddingCache(stateDbPath);

        Protocol.EmitLog(output, "info", "ready");

        while (Console.ReadLine() is { } line)
        {
            line = line.Trim();
            if (string.IsNullOrEmpty(line)) continue;

            var req = Protocol.ParseRequest(line);
            if (req == null)
            {
                Console.Error.WriteLine($"[faces] parse error: {line}");
                continue;
            }

            switch (req.Method)
            {
                case "cluster_faces":
                    try
                    {
                        var result = HandleClusterFaces(detector, recognizer, cache, config, req.Params, req.Id, output);
                        Protocol.SendResponse(output, req.Id, result);
                    }
                    catch (Exception ex)
                    {
                        Protocol.SendError(output, req.Id, ex.Message);
                    }
                    break;
                default:
                    Protocol.SendError(output, req.Id, $"unknown method: {req.Method}");
                    break;
            }
        }
    }

    private static ClusterResult HandleClusterFaces(
        FaceDetector detector, FaceRecognizer recognizer, EmbeddingCache cache,
        Config config, JsonElement parms, ulong reqId, TextWriter output)
    {
        var files = parms.GetProperty("files").EnumerateArray().ToList();
        var forceFull = parms.TryGetProperty("force_full", out var ff) && ff.GetBoolean();
        var total = files.Count;

        if (total == 0)
            return new ClusterResult([], []);

        Protocol.EmitLog(output, "info", $"processing {total} files…");

        for (var i = 0; i < total; i++)
        {
            var file = files[i];
            var fileId = file.GetProperty("file_id").GetString()!;
            var imagePath = file.GetProperty("image_path").GetString()!;
            var fileMtime = file.GetProperty("file_mtime").GetInt64();

            Protocol.EmitProgress(output, reqId, (int)((float)i / total * 80));

            if (cache.IsCached(fileId, fileMtime)) continue;
            if (string.IsNullOrEmpty(imagePath) || !File.Exists(imagePath)) continue;

            try
            {
                using var img = Image.Load<Rgb24>(imagePath);
                var faces = detector.Detect(img);
                cache.DeleteStale(fileId, fileMtime);

                foreach (var face in faces)
                {
                    var embedding = recognizer.Embed(img, face);
                    cache.InsertEmbedding(fileId, fileMtime, face.BboxX, face.BboxY, face.BboxW, face.BboxH, embedding);
                }
            }
            catch (Exception ex)
            {
                Protocol.EmitLog(output, "warn", $"failed for {fileId}: {ex.Message}");
            }
        }

        Protocol.EmitProgress(output, reqId, 82);
        Protocol.EmitLog(output, "info", "clustering…");

        var rows = cache.LoadAll();
        if (rows.Count == 0)
        {
            Protocol.EmitProgress(output, reqId, 100);
            return new ClusterResult([], []);
        }

        var embeddings = rows.Select(r => r.Vec).ToArray();
        var centroids = cache.LoadCentroids();

        int[] labels;
        if (!forceFull && centroids.Count > 0)
        {
            Protocol.EmitLog(output, "info", $"incremental assignment against {centroids.Count} centroids…");
            labels = Clustering.AssignToCentroids(embeddings, centroids.Values.ToArray(), config.Eps);
        }
        else
        {
            Protocol.EmitLog(output, "info", "full DBSCAN clustering…");
            labels = Clustering.Dbscan(embeddings, config.Eps, config.MinPts);
        }

        Protocol.EmitProgress(output, reqId, 95);

        var maxLabel = labels.Max();
        var clusterMembers = new List<(int idx, FaceMember member)>[Math.Max(0, maxLabel + 1)];
        for (var i = 0; i < clusterMembers.Length; i++)
            clusterMembers[i] = [];

        var noiseMembers = new List<FaceMember>();

        for (var i = 0; i < labels.Length; i++)
        {
            var r = rows[i];
            var member = new FaceMember(r.FileId, new BboxData(r.BboxX, r.BboxY, r.BboxW, r.BboxH));
            if (labels[i] < 0)
                noiseMembers.Add(member);
            else
                clusterMembers[labels[i]].Add((i, member));
        }

        var sorted = clusterMembers
            .Where(c => c.Count > 0)
            .OrderByDescending(c => c.Count)
            .ToList();

        if (!forceFull || centroids.Count == 0)
        {
            var newCentroids = new Dictionary<string, float[]>();
            foreach (var cluster in sorted)
            {
                var clusterEmbeddings = cluster.Select(m => embeddings[m.idx]).ToArray();
                var members = cluster.Select(m => m.member).ToList();
                newCentroids[StableClusterId(members)] = Clustering.ComputeCentroid(clusterEmbeddings);
            }
            cache.SaveCentroids(newCentroids);
        }

        var resultClusters = sorted.Select(cluster =>
        {
            var members = cluster.Select(m => m.member).ToList();
            return new ClusterEntry(StableClusterId(members), members);
        }).ToList();

        Protocol.EmitProgress(output, reqId, 100);
        Protocol.EmitLog(output, "info", $"found {resultClusters.Count} people, {noiseMembers.Count} unclustered faces");

        return new ClusterResult(resultClusters, noiseMembers);
    }

    private static string StableClusterId(List<FaceMember> members)
    {
        var keys = members
            .Select(m => $"{m.FileId}:{m.Bbox.X:F1}:{m.Bbox.Y:F1}")
            .OrderBy(k => k)
            .ToList();

        var combined = string.Join("\n", keys);
        var hash = SHA256.HashData(Encoding.UTF8.GetBytes(combined));
        return $"face-{Convert.ToHexString(hash)[..16].ToLowerInvariant()}";
    }

    private static Config LoadConfig(TextWriter output)
    {
        var path = Environment.GetEnvironmentVariable("ISOMFOLIO_ADDON_CONFIG") ?? "";
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
            return new Config();

        try
        {
            var json = File.ReadAllText(path);
            var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;

            var eps = root.TryGetProperty("eps", out var epsEl) ? epsEl.GetSingle() : 0.4f;
            var minPts = root.TryGetProperty("min_pts", out var mpEl) ? mpEl.GetInt32() : 2;
            return new Config(eps, minPts);
        }
        catch (Exception ex)
        {
            Protocol.EmitLog(output, "warn", $"config parse error: {ex.Message}, using defaults");
            return new Config();
        }
    }
}
