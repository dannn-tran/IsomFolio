using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces.Tests.Integration;

[Trait("Category", "Integration")]
public class DetectorTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    [Fact]
    public void LoadsWithoutError()
    {
        using var detector = new FaceDetector(models.DetPath);
    }

    [Fact]
    public async Task DoesNotCrashOnSyntheticImage()
    {
        using var detector = new FaceDetector(models.DetPath);
        using var img = new Image<Rgb24>(640, 480);
        var faces = await detector.DetectAsync(img, TestContext.Current.CancellationToken);
        Assert.NotNull(faces);
    }

    [Theory]
    [InlineData(100, 100)]
    [InlineData(640, 480)]
    [InlineData(1920, 1080)]
    [InlineData(50, 2000)]
    public async Task HandlesVariousImageSizes(int width, int height)
    {
        using var detector = new FaceDetector(models.DetPath);
        using var img = new Image<Rgb24>(width, height);
        var faces = await detector.DetectAsync(img, TestContext.Current.CancellationToken);
        Assert.NotNull(faces);
    }
}
