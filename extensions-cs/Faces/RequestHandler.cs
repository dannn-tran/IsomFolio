using System.Security.Cryptography;
using System.Text;
using IsomFolio.Extensions.Sdk;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Extensions.Faces;

public class RequestHandler(DbscanConfig dbscanConfig, IMessageWriter writer,
    EmbeddingCache cache, FaceDetector detector, FaceRecognizer recognizer) : IRequestHandler
{
    private readonly FacesLogger _log = new("inference");

    public async Task<ClusterResult> HandleAsync(ClusterFacesRequest request, CancellationToken ct = default)
    {
        var files = request.Params.Files;
        var total = files.Length;

        if (total == 0)
            return new ClusterResult([], []);

        await _log.LogAsync(LogLevel.Info, $"processing {total} files…");

        var logEvery = Math.Max(1, total / 20);
        var lastPercent = -1;

        for (var i = 0; i < total; i++)
        {
            var file = files[i];

            var percent = (int)Math.Ceiling((double)(i + 1) / total * 80);
            if (percent != lastPercent)
            {
                await writer.SendProgressAsync(request.Id, percent);
                lastPercent = percent;
            }
            if ((i + 1) % logEvery == 0 || i + 1 == total)
                await _log.LogAsync(LogLevel.Info, $"processed {i + 1}/{total}");

            if (cache.IsCached(file.FileId, file.FileMtime)) continue;
            if (string.IsNullOrEmpty(file.ImagePath) || !File.Exists(file.ImagePath)) continue;

            // Written before native inference so that on a native ORT crash (SIGSEGV bypasses
            // AppDomain.UnhandledException) the last flushed line identifies the failing file.
            _log.Log(LogLevel.Info, $"{i}/{total} {file.ImagePath}");

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
            catch (UnauthorizedAccessException)
            {
                await _log.LogAsync(LogLevel.Warning,
                    $"skipped {file.ImagePath}: permission denied (iCloud placeholder or app lacks TCC access?)");
            }
            catch (Exception ex)
            {
                await _log.LogAsync(LogLevel.Warning, $"failed for {file.FileId}: {ex.Message}");
            }

            // Photos can be 60–100 MB as Rgb24; they land on the LOH and won't be collected
            // without an explicit gen-2 sweep. Optimized mode can skip the sweep if it deems
            // it not cost-effective; Forced ensures the LOH is always reclaimed after each image.
            GC.Collect(2, GCCollectionMode.Forced);
        }

        await writer.SendProgressAsync(request.Id, 82);
        await _log.LogAsync(LogLevel.Info, "clustering…");

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
            await _log.LogAsync(LogLevel.Info, $"incremental assignment against {centroids.Count} centroids…");
            labels = Clustering.AssignToCentroids(embeddings, centroids.Values.ToArray(), dbscanConfig.Eps);
        }
        else
        {
            await _log.LogAsync(LogLevel.Info, "full DBSCAN clustering…");
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
        await _log.LogAsync(LogLevel.Info, $"found {resultClusters.Count} people, {noiseMembers.Count} unclustered faces");

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
