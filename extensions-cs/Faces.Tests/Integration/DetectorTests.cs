using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Extensions.Faces.Tests.Integration;

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

    /// <summary>
    /// Regression test for two related bugs:
    /// 1. Detector used `AsEnumerable&lt;float&gt;()` to extract tensor data — reflection-heavy,
    ///    silently produced incorrect values under PublishAot, leading to thousands of false
    ///    positive detections.
    /// 2. Detector applied `Sigmoid()` on top of model outputs that are already probabilities
    ///    (SCRFD trained with sigmoid head), so non-face anchor scores compressed to ~0.5,
    ///    above the 0.5 threshold — every anchor passed.
    /// A solid synthetic image has no face and must produce zero detections.
    /// </summary>
    [Fact]
    public async Task SolidColourImageProducesNoFalsePositives()
    {
        using var detector = new FaceDetector(models.DetPath);
        using var img = new Image<Rgb24>(640, 640, new Rgb24(128, 128, 128));
        var faces = await detector.DetectAsync(img, TestContext.Current.CancellationToken);
        Assert.Empty(faces);
    }

    [Fact]
    public async Task DetectsAtLeastOneFaceInRealPortrait()
    {
        var testImagePath = Path.Combine(
            AppContext.BaseDirectory, "..", "..", "..", "Assets", "test_face.jpg");
        if (!File.Exists(testImagePath))
            return; // fixture missing — skip silently

        using var detector = new FaceDetector(models.DetPath);
        using var img = Image.Load<Rgb24>(testImagePath);
        var faces = await detector.DetectAsync(img, TestContext.Current.CancellationToken);
        Assert.NotEmpty(faces);
        // The portrait has a single subject — we should not detect dozens of false positives.
        Assert.True(faces.Count < 10,
            $"Expected fewer than 10 faces in test_face.jpg (single portrait); got {faces.Count}. " +
            "If this regresses, suspect a tensor-extraction or score-postprocessing change.");
    }
}
