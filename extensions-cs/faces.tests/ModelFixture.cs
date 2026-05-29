using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces.Tests;

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
        var variant = ModelVariant.Current();
        DetPath = Path.Combine(ModelsDir, variant.Name, variant.DetectionFile);
        RecPath = Path.Combine(ModelsDir, variant.Name, variant.RecognitionFile);
    }

    public ValueTask DisposeAsync()
    {
        GC.SuppressFinalize(this);
        return ValueTask.CompletedTask;
    }
}
