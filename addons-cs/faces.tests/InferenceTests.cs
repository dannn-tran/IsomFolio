using IsomFolio.Addons.Faces;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace Faces.Tests;

[Trait("Category", "Integration")]
public class InferenceTests : IClassFixture<ModelFixture>
{
    private readonly ModelFixture _models;

    public InferenceTests(ModelFixture models)
    {
        _models = models;
    }

    [Fact]
    public void Detector_LoadsWithoutError()
    {
        using var detector = new FaceDetector(_models.DetPath);
    }

    [Fact]
    public void Recognizer_LoadsWithoutError()
    {
        using var recognizer = new FaceRecognizer(_models.RecPath);
    }

    [Fact]
    public void Detector_DoesNotCrashOnSyntheticImage()
    {
        using var detector = new FaceDetector(_models.DetPath);
        using var img = new Image<Rgb24>(640, 480);
        var faces = detector.Detect(img);
        Assert.NotNull(faces);
    }

    [Theory]
    [InlineData(100, 100)]
    [InlineData(640, 480)]
    [InlineData(1920, 1080)]
    [InlineData(50, 2000)]
    public void Detector_HandlesVariousImageSizes(int width, int height)
    {
        using var detector = new FaceDetector(_models.DetPath);
        using var img = new Image<Rgb24>(width, height);
        var faces = detector.Detect(img);
        Assert.NotNull(faces);
    }
}

[Trait("Category", "RealImage")]
public class RealImageTests : IClassFixture<ModelFixture>
{
    private readonly ModelFixture _models;
    private readonly string _imagePath;

    public RealImageTests(ModelFixture models)
    {
        _models = models;
        _imagePath = Environment.GetEnvironmentVariable("TEST_FACE_IMAGE") ?? "";
    }

    [Fact]
    public void Recognizer_ProducesNormalizedEmbedding()
    {
        Assert.True(File.Exists(_imagePath), $"TEST_FACE_IMAGE not set or file missing: {_imagePath}");

        using var detector = new FaceDetector(_models.DetPath);
        using var recognizer = new FaceRecognizer(_models.RecPath);
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

        using var detector = new FaceDetector(_models.DetPath);
        using var recognizer = new FaceRecognizer(_models.RecPath);
        using var img = Image.Load<Rgb24>(_imagePath);

        var faces = detector.Detect(img);
        Assert.NotEmpty(faces);

        var embeddings = faces.Select(f => recognizer.Embed(img, f)).ToArray();
        Assert.All(embeddings, e => Assert.Equal(512, e.Length));

        var labels = Clustering.Dbscan(embeddings, 0.4f, 1);
        Assert.Equal(faces.Count, labels.Length);
    }
}
