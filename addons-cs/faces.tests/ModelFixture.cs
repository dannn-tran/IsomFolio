using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces.Tests;

public class ModelFixture : IAsyncLifetime
{
    private static readonly string ModelsDir =
        Environment.GetEnvironmentVariable("ISOMFOLIO_MODELS_DIR")
        ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "IsomFolio", "models");

    public string DetPath { get; private set; } = "";
    public string RecPath { get; private set; } = "";

    public async ValueTask InitializeAsync()
    {
        var downloader = new ModelDownloader(new MessageWriter(TextWriter.Null));
        var (det, rec) = await downloader.EnsureModelsDownloadedAsync(ModelsDir);
        DetPath = det;
        RecPath = rec;
    }

    public ValueTask DisposeAsync()
    {
        GC.SuppressFinalize(this);
        return ValueTask.CompletedTask;
    }
}
