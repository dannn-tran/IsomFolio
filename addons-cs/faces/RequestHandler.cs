using System.Security.Cryptography;
using System.Text;
using IsomFolio.Addons.Sdk;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces;

public class RequestHandler(DbscanConfig dbscanConfig, IAddonLogger logger, IMessageWriter writer,
    EmbeddingCache cache, FaceDetector detector, FaceRecognizer recognizer) : IRequestHandler
{
    public async Task<ClusterResult> HandleAsync(ClusterFacesRequest request, CancellationToken ct = default)
    {
        var files = request.Params.Files;
        var total = files.Length;

        if (total == 0)
            return new ClusterResult([], []);

        await logger.LogAsync(LogLevel.Info, $"processing {total} files…");

        for (var i = 0; i < total; i++)
        {
            var file = files[i];

            await writer.SendProgressAsync(request.Id, (int)((float)i / total * 80));

            if (cache.IsCached(file.FileId, file.FileMtime)) continue;
            if (string.IsNullOrEmpty(file.ImagePath) || !File.Exists(file.ImagePath)) continue;

            try
            {
                using var img = Image.Load<Rgb24>(file.ImagePath);
                var faces = await detector.DetectAsync(img, ct);
                cache.DeleteStale(file.FileId, file.FileMtime);

                foreach (var face in faces)
                {
                    var embedding = await recognizer.EmbedAsync(img, face, ct);
                    cache.InsertEmbedding(file.FileId, file.FileMtime, face.BboxX, face.BboxY, face.BboxW, face.BboxH, embedding);
                }
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex)
            {
                await logger.LogAsync(LogLevel.Warning, $"failed for {file.FileId}: {ex.Message}");
            }
        }

        await writer.SendProgressAsync(request.Id, 82);
        await logger.LogAsync(LogLevel.Info, "clustering…");

        var rows = cache.LoadAll();
        if (rows.Count == 0)
        {
            await writer.SendProgressAsync(request.Id, 100);
            return new ClusterResult([], []);
        }

        var embeddings = rows.Select(r => r.Vec).ToArray();
        var centroids = cache.LoadCentroids();

        int[] labels;
        if (!request.Params.ForceFull && centroids.Count > 0)
        {
            await logger.LogAsync(LogLevel.Info, $"incremental assignment against {centroids.Count} centroids…");
            labels = Clustering.AssignToCentroids(embeddings, centroids.Values.ToArray(), dbscanConfig.Eps);
        }
        else
        {
            await logger.LogAsync(LogLevel.Info, "full DBSCAN clustering…");
            labels = Clustering.Dbscan(embeddings, dbscanConfig.Eps, dbscanConfig.MinPts);
        }

        await writer.SendProgressAsync(request.Id, 95);

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

        if (!request.Params.ForceFull || centroids.Count == 0)
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

        await writer.SendProgressAsync(request.Id, 100);
        await logger.LogAsync(LogLevel.Info, $"found {resultClusters.Count} people, {noiseMembers.Count} unclustered faces");

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

    public void Dispose()
    {
        cache.Dispose();
        detector.Dispose();
        recognizer.Dispose();
        GC.SuppressFinalize(this);
    }
}
