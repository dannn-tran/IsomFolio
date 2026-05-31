using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

/// Loads the ORT models once, then turns image paths into face embeddings.
/// Stateless beyond the loaded models — no DB, no cache. The host owns
/// persistence and clustering.
public sealed class Engine : IDisposable
{
    private readonly FacesLogger _log = new("engine");
    private readonly SemaphoreSlim _gate = new(1, 1);
    private FaceDetector? _detector;
    private FaceRecognizer? _recognizer;

    /// True once models are loaded and /embed can be served.
    public volatile bool Ready;
    /// Set if model loading failed; surfaced via /health so the host fails fast.
    public volatile string? FatalError;

    public async Task LoadAsync(string dataDir, ModelVariant variant, CancellationToken ct = default)
    {
        try
        {
            var modelDir = Path.Combine(dataDir, variant.Name);
            var detPath = Path.Combine(modelDir, variant.DetectionFile);
            var recPath = Path.Combine(modelDir, variant.RecognitionFile);

            if (!File.Exists(detPath) || !File.Exists(recPath))
            {
                await _log.LogAsync(LogLevel.Info, $"{variant.Name} models not found, downloading…");
                await new ModelDownloader().EnsureModelsDownloadedAsync(dataDir, variant, ct);
            }

            (_detector, _recognizer) = await Task.Run(
                () => (new FaceDetector(detPath), new FaceRecognizer(recPath)), ct);
            Ready = true;
            await _log.LogAsync(LogLevel.Info, "models ready");
        }
        catch (Exception ex)
        {
            FatalError = ex.Message;
            await _log.LogAsync(LogLevel.Error, $"model load failed: {ex}");
        }
    }

    public async Task<EmbedResponse> EmbedAsync(EmbedRequest request, CancellationToken ct = default)
    {
        if (_detector is null || _recognizer is null)
            throw new InvalidOperationException("engine not ready");

        // One ORT batch at a time — the host sends batches sequentially anyway.
        await _gate.WaitAsync(ct);
        try
        {
            var results = new List<FileResult>(request.Files.Length);
            foreach (var file in request.Files)
            {
                results.Add(await EmbedFileAsync(file, ct));
                // Photos can be 60–100 MB as Rgb24 and land on the LOH; force a
                // gen-2 sweep so memory doesn't balloon across the batch.
                GC.Collect(2, GCCollectionMode.Forced);
            }
            return new EmbedResponse(results.ToArray());
        }
        finally { _gate.Release(); }
    }

    private async Task<FileResult> EmbedFileAsync(EmbedFile file, CancellationToken ct)
    {
        if (string.IsNullOrEmpty(file.Path) || !File.Exists(file.Path))
            return new FileResult(file.FileId, []);

        try
        {
            using var img = Image.Load<Rgb24>(file.Path);
            float w = img.Width;
            float h = img.Height;
            var faces = await _detector!.DetectAsync(img, ct);

            var dtos = new List<FaceResult>(faces.Count);
            foreach (var face in faces)
            {
                var vec = await _recognizer!.EmbedAsync(img, face, ct);
                dtos.Add(new FaceResult(
                    new BboxResult(face.BboxX / w, face.BboxY / h, face.BboxW / w, face.BboxH / h),
                    vec));
            }
            return new FileResult(file.FileId, dtos.ToArray());
        }
        catch (OperationCanceledException) { throw; }
        catch (UnauthorizedAccessException)
        {
            await _log.LogAsync(LogLevel.Warning,
                $"skipped {file.Path}: permission denied (iCloud placeholder or missing TCC access?)");
            return new FileResult(file.FileId, []);
        }
        catch (Exception ex)
        {
            await _log.LogAsync(LogLevel.Warning, $"failed for {file.FileId}: {ex.Message}");
            return new FileResult(file.FileId, []);
        }
    }

    public void Dispose()
    {
        _detector?.Dispose();
        _recognizer?.Dispose();
        _gate.Dispose();
    }
}
