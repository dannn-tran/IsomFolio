namespace IsomFolio.Extensions.Faces;

/// <summary>
/// InsightFace model bundle. `buffalo_l` is the default (high accuracy, ~280 MB);
/// `buffalo_s` is a small variant (~25 MB) useful for testing or low-resource
/// environments. Selected via the `ISFX_FACES_VARIANT` environment variable.
/// </summary>
public record ModelVariant(string Name, string DetectionFile, string RecognitionFile)
{
    public static readonly ModelVariant BuffaloL = new("buffalo_l", "det_10g.onnx", "w600k_r50.onnx");
    public static readonly ModelVariant BuffaloS = new("buffalo_s", "det_500m.onnx", "w600k_mbf.onnx");

    public string DownloadUrl =>
        $"https://github.com/deepinsight/insightface/releases/download/v0.7/{Name}.zip";

    public static ModelVariant Current()
    {
        var name = Environment.GetEnvironmentVariable("ISFX_FACES_VARIANT");
        return name switch
        {
            "buffalo_s" => BuffaloS,
            _ => BuffaloL,
        };
    }
}
