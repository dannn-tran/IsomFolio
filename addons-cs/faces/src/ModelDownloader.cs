using System.IO.Compression;

namespace IsomFolio.Addons.Faces;

public static class ModelDownloader
{
    const string BuffaloZipUrl = "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip";
    const string DetFilename = "det_10g.onnx";
    const string RecFilename = "w600k_r50.onnx";

    public static (string detPath, string recPath) EnsureModels(string modelsDir, TextWriter log)
    {
        var dir = Path.Combine(modelsDir, "buffalo_l");
        Directory.CreateDirectory(dir);

        var detPath = Path.Combine(dir, DetFilename);
        var recPath = Path.Combine(dir, RecFilename);

        if (!File.Exists(detPath) || !File.Exists(recPath))
            DownloadAndExtract(dir, log);

        if (!File.Exists(detPath))
            throw new FileNotFoundException($"{DetFilename} not found in {dir}");
        if (!File.Exists(recPath))
            throw new FileNotFoundException($"{RecFilename} not found in {dir}");

        return (detPath, recPath);
    }

    private static void DownloadAndExtract(string dir, TextWriter log)
    {
        Protocol.EmitLog(log, "info", "downloading face models from GitHub…");

        using var client = new HttpClient();
        var zipBytes = client.GetByteArrayAsync(BuffaloZipUrl).GetAwaiter().GetResult();

        Protocol.EmitLog(log, "info", "extracting models…");

        using var archive = new ZipArchive(new MemoryStream(zipBytes), ZipArchiveMode.Read);
        string[] needed = [DetFilename, RecFilename];

        foreach (var name in needed)
        {
            var entry = archive.GetEntry(name)
                ?? throw new InvalidOperationException($"{name} not found in archive");
            using var entryStream = entry.Open();
            using var outFile = File.Create(Path.Combine(dir, name));
            entryStream.CopyTo(outFile);
            Protocol.EmitLog(log, "info", $"{name} ready");
        }
    }
}
