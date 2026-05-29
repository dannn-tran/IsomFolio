using System.IO.Compression;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public class ModelDownloader(IExtensionLogger logger)
{
    private static readonly HttpClient Client = new();

    public async Task EnsureModelsDownloadedAsync(string dataDir, CancellationToken ct = default)
    {
        var variant = ModelVariant.Current();
        var dir = Path.Combine(dataDir, variant.Name);
        Directory.CreateDirectory(dir);

        if (!File.Exists(Path.Combine(dir, variant.DetectionFile))
            || !File.Exists(Path.Combine(dir, variant.RecognitionFile)))
            await DownloadAndExtractAsync(variant, dir, ct);

        if (!File.Exists(Path.Combine(dir, variant.DetectionFile)))
            throw new FileNotFoundException($"{variant.DetectionFile} not found in {dir}");
        if (!File.Exists(Path.Combine(dir, variant.RecognitionFile)))
            throw new FileNotFoundException($"{variant.RecognitionFile} not found in {dir}");
    }

    private async Task DownloadAndExtractAsync(ModelVariant variant, string dir, CancellationToken ct)
    {
        await logger.LogAsync(LogLevel.Info, $"downloading {variant.Name} face models from GitHub…");

        var tmpPath = Path.Combine(dir, $"{variant.Name}.zip.tmp");
        try
        {
            using var response = await Client.GetAsync(variant.DownloadUrl, HttpCompletionOption.ResponseHeadersRead, ct);
            response.EnsureSuccessStatusCode();

            var total = response.Content.Headers.ContentLength ?? 0;
            await using var stream = await response.Content.ReadAsStreamAsync(ct);
            await using var tmpFile = File.Create(tmpPath);

            var buffer = new byte[81920];
            long downloaded = 0;
            int lastReported = -1;
            int read;
            while ((read = await stream.ReadAsync(buffer, ct)) > 0)
            {
                await tmpFile.WriteAsync(buffer.AsMemory(0, read), ct);
                downloaded += read;
                if (total > 0)
                {
                    var percent = (int)(downloaded * 100 / total);
                    if (percent / 10 != lastReported / 10 && percent % 10 == 0)
                    {
                        lastReported = percent;
                        await logger.LogAsync(LogLevel.Info, $"downloading… {percent}%");
                    }
                }
            }
            await tmpFile.FlushAsync(ct);
        }
        catch
        {
            if (File.Exists(tmpPath)) File.Delete(tmpPath);
            throw;
        }

        await logger.LogAsync(LogLevel.Info, "extracting models…");
        await using (var archive = new ZipArchive(File.OpenRead(tmpPath), ZipArchiveMode.Read))
        {
            foreach (var name in new[] { variant.DetectionFile, variant.RecognitionFile })
            {
                var entry = archive.GetEntry(name)
                    ?? throw new InvalidOperationException($"{name} not found in archive");
                await using var entryStream = await entry.OpenAsync(ct);
                await using var outFile = File.Create(Path.Combine(dir, name));
                await entryStream.CopyToAsync(outFile, ct);
                await logger.LogAsync(LogLevel.Info, $"{name} ready");
            }
        }
        File.Delete(tmpPath);
    }
}
