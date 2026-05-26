using IsomFolio.Addons.Faces;

namespace Faces.Tests;

public class ModelFixture : IAsyncLifetime
{
    private static readonly string ModelsDir =
        Environment.GetEnvironmentVariable("ISOMFOLIO_MODELS_DIR")
        ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "IsomFolio", "models");

    public string DetPath { get; private set; } = "";
    public string RecPath { get; private set; } = "";

    public ValueTask InitializeAsync()
    {
        var (det, rec) = ModelDownloader.EnsureModels(ModelsDir, Console.Out);
        DetPath = det;
        RecPath = rec;
        return ValueTask.CompletedTask;
    }

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
}
