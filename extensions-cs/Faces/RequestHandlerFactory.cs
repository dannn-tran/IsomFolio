using System.Text.Json;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public class RequestHandlerFactory(IMessageWriter writer)
{
    private readonly FacesLogger _log = new("startup");

    /// <param name="extDir">Install directory — holds <c>config.json</c>.</param>
    /// <param name="dataDir">Persistent data directory — survives reinstalls. Holds models.</param>
    /// <param name="catalogDbPath">Catalog DB path passed by the host via <c>--catalog-db</c>.</param>
    public async Task<RequestHandler> CreateAsync(
        string extDir,
        string dataDir,
        string catalogDbPath,
        CancellationToken ct = default)
    {
        var config = await GetConfigAsync(extDir, ct);
        var variant = ModelVariant.Current(config.ModelVariant);
        var modelDir = Path.Combine(dataDir, variant.Name);
        var detPath = Path.Combine(modelDir, variant.DetectionFile);
        var recPath = Path.Combine(modelDir, variant.RecognitionFile);

        if (!File.Exists(detPath) || !File.Exists(recPath))
        {
            await _log.LogAsync(LogLevel.Info, $"{variant.Name} models not found, downloading…");
            await new ModelDownloader().EnsureModelsDownloadedAsync(dataDir, variant, ct);
        }

        var (detector, recognizer) = await Task.Run(
            () => (new FaceDetector(detPath), new FaceRecognizer(recPath)), ct);

        var cache = new EmbeddingCache(catalogDbPath);
        return new RequestHandler(config, writer, cache, detector, recognizer);
    }

    private async Task<DbscanConfig> GetConfigAsync(string extDir, CancellationToken ct)
    {
        var path = Path.Combine(extDir, "config.json");
        if (!File.Exists(path))
            return new DbscanConfig();

        try
        {
            var json = await File.ReadAllTextAsync(path, ct);
            return JsonSerializer.Deserialize(json, AppJsonContext.Default.DbscanConfig) ?? new DbscanConfig();
        }
        catch (Exception ex)
        {
            await _log.LogAsync(LogLevel.Warning, $"config parse error: {ex.Message}, using defaults");
            return new DbscanConfig();
        }
    }
}
