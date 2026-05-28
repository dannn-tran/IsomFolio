using System.Text.Json;
using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public class RequestHandlerFactory(IAddonLogger logger, IMessageWriter writer)
{
    private const string DetFilename = "det_10g.onnx";
    private const string RecFilename = "w600k_r50.onnx";

    public async Task<RequestHandler> CreateAsync(string dataDir, CancellationToken ct = default)
    {
        var modelDir = Path.Combine(dataDir, "buffalo_l");
        var detPath = Path.Combine(modelDir, DetFilename);
        var recPath = Path.Combine(modelDir, RecFilename);

        if (!File.Exists(detPath))
            throw new FileNotFoundException($"{DetFilename} not found — run installer to repair", detPath);
        if (!File.Exists(recPath))
            throw new FileNotFoundException($"{RecFilename} not found — run installer to repair", recPath);

        var (detector, recognizer) = await Task.Run(
            () => (new FaceDetector(detPath), new FaceRecognizer(recPath)), ct);

        var stateDbPath = Path.Combine(dataDir, "faces", "state.db");
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
