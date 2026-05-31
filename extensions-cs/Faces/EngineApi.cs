using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Faces;

// Wire contract for the inference engine. The host owns persistence and
// clustering; the engine only turns image paths into face embeddings.
// See dev-docs/face-inference-engine.md.

public record EmbedRequest(EmbedFile[] Files);

public record EmbedFile(string FileId, string Path, long Mtime);

public record EmbedResponse(FileResult[] Results);

public record FileResult(string FileId, FaceResult[] Faces);

public record FaceResult(BboxResult Bbox, float[] Vec);

/// Normalised 0–1 bounding box (fraction of image width/height).
public record BboxResult(double X, double Y, double W, double H);

/// Persisted extension config (written by the host's config UI). Only
/// `model_variant` is engine-relevant; clustering params live host-side.
public record EngineConfig(string? ModelVariant = null);

[JsonSerializable(typeof(EmbedRequest))]
[JsonSerializable(typeof(EmbedResponse))]
[JsonSerializable(typeof(EngineConfig))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
public partial class EngineJsonContext : JsonSerializerContext;
