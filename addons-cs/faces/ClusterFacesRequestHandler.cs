using System.Security.Cryptography;
using System.Text;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces;

public interface IClusterFacesRequestHandler
{
    ClusterResult Handle(ClusterFacesRequest request);
}

internal class ClusterFacesRequestHandler(IMessageOutbox outbox, EmbeddingCache cache, FaceDetector detector,
    FaceRecognizer recognizer) : IClusterFacesRequestHandler
{
    public ClusterResult Handle(ClusterFacesRequest request)
    {
    // private static ClusterResult Handle(
    //     FaceDetector detector, FaceRecognizer recognizer, EmbeddingCache cache,
    //     Config config, JsonElement parms, ulong reqId, TextWriter output)
    // {
        var total = request.Params.Files.Count;

        if (total == 0)
            return new ClusterResult([], []);

        outbox.SendLog(LogLevel.Info, $"processing {total} files…");

        for (var i = 0; i < total; i++)
        {
            var file = files[i];
            var fileId = file.GetProperty("file_id").GetString()!;
            var imagePath = file.GetProperty("image_path").GetString()!;
            var fileMtime = file.GetProperty("file_mtime").GetInt64();

            outbox.SendProgress(request.Id, (int)((float)i / total * 80));

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
                outbox.SendLog(LogLevel.Warning, $"failed for {fileId}: {ex.Message}");
            }
        }

        outbox.SendProgress(request.Id, 82);
        outbox.SendLog(LogLevel.Info, "clustering…");

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
}