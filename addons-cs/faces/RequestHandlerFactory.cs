using System.Text.Json;
using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public class RequestHandlerFactory(IAddonLogger logger, IMessageWriter writer)
{
    public async Task<RequestHandler> CreateAsync(CancellationToken ct = default)
    {
        var modelsDir = Environment.GetEnvironmentVariable("ISOMFOLIO_MODELS_DIR") ?? ".";

        var (detPath, recPath) = await new ModelDownloader(logger).EnsureModelsDownloadedAsync(modelsDir, ct);

        // ONNX session construction is CPU-bound with no async API
        var (detector, recognizer) = await Task.Run(
            () => (new FaceDetector(detPath), new FaceRecognizer(recPath)), ct);

        var stateDbPath = Path.Combine(modelsDir, "faces", "state.db");
        Directory.CreateDirectory(Path.GetDirectoryName(stateDbPath)!);
        var cache = new EmbeddingCache(stateDbPath);

        return new RequestHandler(await GetConfigAsync(ct), logger, writer, cache, detector, recognizer);
    }

    private async Task<DbscanConfig> GetConfigAsync(CancellationToken ct)
    {
        var path = Environment.GetEnvironmentVariable("ISOMFOLIO_ADDON_CONFIG") ?? "";
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
            return new DbscanConfig();

        try
        {
            var json = await File.ReadAllTextAsync(path, ct);
            return JsonSerializer.Deserialize(json, AppJsonContext.Default.DbscanConfig) ?? new DbscanConfig();
        }
        catch (Exception ex)
        {
            await logger.LogAsync(LogLevel.Warning, $"config parse error: {ex.Message}, using defaults");
            return new DbscanConfig();
        }
    }
}
