using System.IO.Compression;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public class ModelDownloader
{
    private static readonly HttpClient Client = new();
    private readonly FacesLogger _log = new("models");

    public async Task EnsureModelsDownloadedAsync(string dataDir, ModelVariant? variant = null, CancellationToken ct = default)
    {
        var v = variant ?? ModelVariant.Current();
        var dir = Path.Combine(dataDir, v.Name);
        Directory.CreateDirectory(dir);

        if (!File.Exists(Path.Combine(dir, v.DetectionFile))
            || !File.Exists(Path.Combine(dir, v.RecognitionFile)))
            await DownloadAndExtractAsync(v, dir, ct);

        if (!File.Exists(Path.Combine(dir, v.DetectionFile)))
            throw new FileNotFoundException($"{v.DetectionFile} not found in {dir}");
        if (!File.Exists(Path.Combine(dir, v.RecognitionFile)))
            throw new FileNotFoundException($"{v.RecognitionFile} not found in {dir}");
    }

    private async Task DownloadAndExtractAsync(ModelVariant variant, string dir, CancellationToken ct)
    {
        await _log.LogAsync(LogLevel.Info, $"downloading {variant.Name} face models from GitHub…");

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
                        await _log.LogAsync(LogLevel.Info, $"downloading… {percent}%");
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

        await _log.LogAsync(LogLevel.Info, "extracting models…");
        await using (var archive = new ZipArchive(File.OpenRead(tmpPath), ZipArchiveMode.Read))
        {
            foreach (var name in new[] { variant.DetectionFile, variant.RecognitionFile })
            {
                var entry = archive.GetEntry(name)
                    ?? throw new InvalidOperationException($"{name} not found in archive");
                await using var entryStream = await entry.OpenAsync(ct);
                await using var outFile = File.Create(Path.Combine(dir, name));
                await entryStream.CopyToAsync(outFile, ct);
                await _log.LogAsync(LogLevel.Info, $"{name} ready");
            }
        }
        File.Delete(tmpPath);
    }
}
