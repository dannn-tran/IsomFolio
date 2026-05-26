using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces.Tests.Integration;


[Trait("Category", "Integration")]
public class PipelineTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    private readonly string _imagePath = Environment.GetEnvironmentVariable("TEST_FACE_IMAGE") ?? "";

    [Fact]
    public void Recognizer_ProducesNormalizedEmbedding()
    {
        Assert.True(File.Exists(_imagePath), $"TEST_FACE_IMAGE not set or file missing: {_imagePath}");

        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        using var img = Image.Load<Rgb24>(_imagePath);

        var faces = detector.Detect(img);
        Assert.NotEmpty(faces);

        var embedding = recognizer.Embed(img, faces[0]);
        Assert.Equal(512, embedding.Length);

        var norm = MathF.Sqrt(embedding.Sum(x => x * x));
        Assert.InRange(norm, 0.99f, 1.01f);
    }

    [Fact]
    public void FullPipeline_DetectEmbedCluster()
    {
        Assert.True(File.Exists(_imagePath), $"TEST_FACE_IMAGE not set or file missing: {_imagePath}");

        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        using var img = Image.Load<Rgb24>(_imagePath);

        var faces = detector.Detect(img);
        Assert.NotEmpty(faces);

        var embeddings = faces.Select(f => recognizer.Embed(img, f)).ToArray();
        Assert.All(embeddings, e => Assert.Equal(512, e.Length));

        var labels = Clustering.Dbscan(embeddings, 0.4f, 1);
        Assert.Equal(faces.Count, labels.Length);
    }
}