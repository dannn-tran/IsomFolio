using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces.Tests;

public class ModelFixture : IAsyncLifetime
{
    private static readonly string ModelsDir =
        Environment.GetEnvironmentVariable("ISOMFOLIO_TEST_MODELS_DIR")
        ?? Path.Combine(Path.GetTempPath(), "isomfolio-test-models");

    public string DetPath { get; private set; } = "";
    public string RecPath { get; private set; } = "";

    public async ValueTask InitializeAsync()
    {
        await new ModelDownloader(new MessageWriter(TextWriter.Null)).EnsureModelsDownloadedAsync(ModelsDir);
        DetPath = Path.Combine(ModelsDir, "buffalo_l", "det_10g.onnx");
        RecPath = Path.Combine(ModelsDir, "buffalo_l", "w600k_r50.onnx");
    }

    public ValueTask DisposeAsync()
    {
        GC.SuppressFinalize(this);
        return ValueTask.CompletedTask;
    }
}
