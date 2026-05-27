using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces.Tests.Integration;

[Trait("Category", "Integration")]
public class RecognizerTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    private static readonly string TestFaceImage = Path.Combine(
        AppContext.BaseDirectory, "Assets", "test_face.jpg");

    [Fact]
    public void LoadsWithoutError()
    {
        using var recognizer = new FaceRecognizer(models.RecPath);
    }

    [Fact]
    public async Task ProducesNormalizedEmbedding()
    {
        using var detector = new FaceDetector(models.DetPath);
        using var recognizer = new FaceRecognizer(models.RecPath);
        using var img = Image.Load<Rgb24>(TestFaceImage);

        var ct = TestContext.Current.CancellationToken;
        var faces = await detector.DetectAsync(img, ct);
        Assert.NotEmpty(faces);

        var embedding = await recognizer.EmbedAsync(img, faces[0], ct);
        Assert.Equal(512, embedding.Length);

        var norm = MathF.Sqrt(embedding.Sum(x => x * x));
        Assert.InRange(norm, 0.99f, 1.01f);
    }
}
