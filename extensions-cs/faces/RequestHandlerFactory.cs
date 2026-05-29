using System.Text.Json;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public class RequestHandlerFactory(IExtensionLogger logger, IMessageWriter writer)
{
    public async Task<RequestHandler> CreateAsync(string extDir, CancellationToken ct = default)
    {
        var variant = ModelVariant.Current();
        var modelDir = Path.Combine(extDir, "models", variant.Name);
        var detPath = Path.Combine(modelDir, variant.DetectionFile);
        var recPath = Path.Combine(modelDir, variant.RecognitionFile);

        if (!File.Exists(detPath))
            throw new FileNotFoundException($"{variant.DetectionFile} not found — run setup to repair", detPath);
        if (!File.Exists(recPath))
            throw new FileNotFoundException($"{variant.RecognitionFile} not found — run setup to repair", recPath);

        var (detector, recognizer) = await Task.Run(
            () => (new FaceDetector(detPath), new FaceRecognizer(recPath)), ct);

        var stateDbPath = Path.Combine(extDir, "state.db");
        Directory.CreateDirectory(Path.GetDirectoryName(stateDbPath)!);
        var cache = new EmbeddingCache(stateDbPath);

        return new RequestHandler(await GetConfigAsync(ct), logger, writer, cache, detector, recognizer);
    }

    private async Task<DbscanConfig> GetConfigAsync(CancellationToken ct)
    {
        var path = Path.Combine(AppContext.BaseDirectory, "config.json");
        if (!File.Exists(path))
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
