using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces.Tests.Integration;

[Trait("Category", "Integration")]
public class RequestHandlerTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    private static readonly string TestFaceImage = Path.Combine(
        AppContext.BaseDirectory, "Assets", "test_face.jpg");

    private static (EmbeddingCache cache, string path) TempCache()
    {
        var path = Path.Combine(Path.GetTempPath(), $"faces_test_{Guid.NewGuid():N}.db");
        return (new EmbeddingCache(path), path);
    }

    [Fact]
    public async Task Handle_EmptyFiles_ReturnsEmpty()
    {
        var logger = new MessageWriter(TextWriter.Null);
        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        var (cache, dbPath) = TempCache();
        try
        {
            using var handler = new RequestHandler(new DbscanConfig(), logger, logger, cache, detector, recognizer);
            var req = new ClusterFacesRequest(1, new ClusterFacesRequestParams([], ForceFull: false));
            var result = await handler.HandleAsync(req, TestContext.Current.CancellationToken);

            Assert.Empty(result.Clusters);
            Assert.Empty(result.Noise);
        }
        finally
        {
            cache.Dispose();
            File.Delete(dbPath);
        }
    }

    [Fact]
    public async Task Handle_WithFaceImage_FindsFaces()
    {
        var logger = new MessageWriter(TextWriter.Null);
        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        var (cache, dbPath) = TempCache();
        try
        {
            using var handler = new RequestHandler(new DbscanConfig(), logger, logger, cache, detector, recognizer);
            var req = new ClusterFacesRequest(1, new ClusterFacesRequestParams(
                [new ImageInfo("file1", TestFaceImage, 0)], ForceFull: true));
            var result = await handler.HandleAsync(req, TestContext.Current.CancellationToken);

            Assert.True(result.Clusters.Count + result.Noise.Count > 0,
                "expected at least one face detected");
        }
        finally
        {
            cache.Dispose();
            File.Delete(dbPath);
        }
    }
}
