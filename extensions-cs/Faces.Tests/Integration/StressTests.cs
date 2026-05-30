using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces.Tests.Integration;

/// <summary>
/// Stress test for the ORT memory arena fix (EnableCpuMemArena=false, EnableMemoryPattern=false).
/// Previously crashed at ~image 16 due to native heap exhaustion.
///
/// Set ISOMFOLIO_STRESS_TEST_DIR to a folder of JPEGs to run.
/// Set ISOMFOLIO_STRESS_TEST_MAX to limit image count (default: 50).
/// </summary>
[Trait("Category", "Stress")]
public class StressTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    [Fact]
    public async Task ProcessLargeBatch_DoesNotCrash()
    {
        var dir = Environment.GetEnvironmentVariable("ISOMFOLIO_STRESS_TEST_DIR");
        if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir))
            Assert.Skip("ISOMFOLIO_STRESS_TEST_DIR not set or does not exist");

        var maxRaw = Environment.GetEnvironmentVariable("ISOMFOLIO_STRESS_TEST_MAX");
        var max = int.TryParse(maxRaw, out var n) ? n : 50;

        var files = Directory.GetFiles(dir, "*.jpg", SearchOption.TopDirectoryOnly)
            .Concat(Directory.GetFiles(dir, "*.jpeg", SearchOption.TopDirectoryOnly))
            .OrderBy(p => p)
            .Take(max)
            .Select((p, i) =>
            {
                var mtime = new FileInfo(p).LastWriteTimeUtc.Ticks;
                return new ImageInfo($"stress-{i}", p, mtime);
            })
            .ToArray();

        Assert.True(files.Length >= 20,
            $"Need at least 20 images to stress test (found {files.Length} in {dir})");

        Console.Error.WriteLine($"[stress] processing {files.Length} images from {dir}");

        var dbPath = Path.Combine(Path.GetTempPath(), $"stress_test_{Guid.NewGuid():N}.db");
        using var nullWriter = new MessageWriter(TextWriter.Null);
        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        var cache = new EmbeddingCache(dbPath);

        try
        {
            using var handler = new RequestHandler(new DbscanConfig(), nullWriter, cache, detector, recognizer);
            var req = new ClusterFacesRequest(1, new ClusterFacesRequestParams(files, ForceFull: true));

            var sw = System.Diagnostics.Stopwatch.StartNew();
            var result = await handler.HandleAsync(req, TestContext.Current.CancellationToken);
            sw.Stop();

            Console.Error.WriteLine(
                $"[stress] done in {sw.Elapsed.TotalSeconds:F1}s — " +
                $"{result.Clusters.Count} clusters, {result.Noise.Count} noise faces");

            // If we reach here without a native crash, the fix works.
            Assert.True(result.Clusters.Count + result.Noise.Count >= 0);
        }
        finally
        {
            cache.Dispose();
            File.Delete(dbPath);
        }
    }
}
