using System.IO.Compression;
using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public class ModelDownloader(IAddonLogger logger)
{
    private static readonly HttpClient Client = new();
    private const string BuffaloZipUrl = "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip";
    private const string DetFilename = "det_10g.onnx";
    private const string RecFilename = "w600k_r50.onnx";

    public async Task<(string DetPath, string RecPath)> EnsureModelsDownloadedAsync(
        string modelsDir, CancellationToken ct = default)
    {
        var dir = Path.Combine(modelsDir, "buffalo_l");
        Directory.CreateDirectory(dir);

        var detPath = Path.Combine(dir, DetFilename);
        var recPath = Path.Combine(dir, RecFilename);

        if (!File.Exists(detPath) || !File.Exists(recPath))
            await DownloadAndExtractAsync(dir, ct);

        if (!File.Exists(detPath))
            throw new FileNotFoundException($"{DetFilename} not found in {dir}");
        if (!File.Exists(recPath))
            throw new FileNotFoundException($"{RecFilename} not found in {dir}");

        return (detPath, recPath);
    }

    private async Task DownloadAndExtractAsync(string dir, CancellationToken ct)
    {
        await logger.LogAsync(LogLevel.Info, "downloading face models from GitHub…");
        var zipBytes = await Client.GetByteArrayAsync(BuffaloZipUrl, ct);

        await logger.LogAsync(LogLevel.Info, "extracting models…");
        await using var archive = new ZipArchive(new MemoryStream(zipBytes), ZipArchiveMode.Read);

        foreach (var name in new[] { DetFilename, RecFilename })
        {
            var entry = archive.GetEntry(name)
                ?? throw new InvalidOperationException($"{name} not found in archive");
            await using var entryStream = await entry.OpenAsync(ct);
            await using var outFile = File.Create(Path.Combine(dir, name));
            await entryStream.CopyToAsync(outFile, ct);
            await logger.LogAsync(LogLevel.Info, $"{name} ready");
        }
    }
}
