using System.IO.Compression;

namespace IsomFolio.Addons.Faces;

public interface IModelDownloader
{
    public (string DetPath, string RecPath) EnsureModelsDownloaded(string modelsDir);
}

public class ModelDownloader(IMessageOutbox messageOutbox) : IModelDownloader
{
    private static readonly HttpClient Client = new();
    private const string BuffaloZipUrl = "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip";
    private const string DetFilename = "det_10g.onnx";
    private const string RecFilename = "w600k_r50.onnx";

    public (string DetPath, string RecPath) EnsureModelsDownloaded(string modelsDir)
    {
        var dir = Path.Combine(modelsDir, "buffalo_l");
        Directory.CreateDirectory(dir);

        var detPath = Path.Combine(dir, DetFilename);
        var recPath = Path.Combine(dir, RecFilename);

        if (!File.Exists(detPath) || !File.Exists(recPath))
            DownloadAndExtract(dir);

        if (!File.Exists(detPath))
            throw new FileNotFoundException($"{DetFilename} not found in {dir}");
        if (!File.Exists(recPath))
            throw new FileNotFoundException($"{RecFilename} not found in {dir}");

        return (detPath, recPath);
    }

    private void DownloadAndExtract(string dir)
    {
        messageOutbox.SendLog(LogLevel.Info, "downloading face models from GitHub…");

        var zipBytes = Client.GetByteArrayAsync(BuffaloZipUrl).GetAwaiter().GetResult();

        messageOutbox.SendLog(LogLevel.Info, "extracting models…");

        using var archive = new ZipArchive(new MemoryStream(zipBytes), ZipArchiveMode.Read);
        string[] needed = [DetFilename, RecFilename];

        foreach (var name in needed)
        {
            var entry = archive.GetEntry(name)
                ?? throw new InvalidOperationException($"{name} not found in archive");
            using var entryStream = entry.Open();
            using var outFile = File.Create(Path.Combine(dir, name));
            entryStream.CopyTo(outFile);
            messageOutbox.SendLog(LogLevel.Info, $"{name} ready");
        }
    }
}
