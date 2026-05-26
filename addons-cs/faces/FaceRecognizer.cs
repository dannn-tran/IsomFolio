using Microsoft.ML.OnnxRuntime;
using Microsoft.ML.OnnxRuntime.Tensors;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;

namespace IsomFolio.Addons.Faces;

public class FaceRecognizer : IDisposable
{
    private const int InputSize = 112;

    private static readonly float[][] ArcfaceDst =
    [
        [38.2946f, 51.6963f],
        [73.5318f, 51.5014f],
        [56.0252f, 71.7366f],
        [41.5493f, 92.3655f],
        [70.7299f, 92.2041f]
    ];

    private readonly InferenceSession _session;

    public FaceRecognizer(string modelPath)
    {
        var opts = new SessionOptions { InterOpNumThreads = 1, IntraOpNumThreads = 4 };
        opts.GraphOptimizationLevel = GraphOptimizationLevel.ORT_ENABLE_ALL;
        _session = new InferenceSession(modelPath, opts);
    }

    public float[] Embed(Image<Rgb24> image, DetectedFace face)
    {
        using var aligned = AlignFace(image, face.Kps);
        var input = Preprocess(aligned);

        using var results = _session.Run([NamedOnnxValue.CreateFromTensor(_session.InputNames[0], input)]);
        var output = results[0].AsEnumerable<float>().ToArray();
        return L2Normalize(output);
    }

    private static DenseTensor<float> Preprocess(Image<Rgb24> img)
    {
        var tensor = new DenseTensor<float>([1, 3, InputSize, InputSize]);
        img.ProcessPixelRows(accessor =>
        {
            for (var y = 0; y < InputSize; y++)
            {
                var row = accessor.GetRowSpan(y);
                for (var x = 0; x < InputSize; x++)
                {
                    var p = row[x];
                    tensor[0, 0, y, x] = (p.R - 127.5f) / 127.5f;
                    tensor[0, 1, y, x] = (p.G - 127.5f) / 127.5f;
                    tensor[0, 2, y, x] = (p.B - 127.5f) / 127.5f;
                }
            }
        });
        return tensor;
    }

    private static Image<Rgb24> AlignFace(Image<Rgb24> img, float[][] kps)
    {
        var (a, b, tx, ty) = EstimateSimilarityTransform(kps, ArcfaceDst);

        var aligned = new Image<Rgb24>(InputSize, InputSize);
        for (var oy = 0; oy < InputSize; oy++)
        {
            for (var ox = 0; ox < InputSize; ox++)
            {
                var sx = (int)Math.Round(a * ox - b * oy + tx);
                var sy = (int)Math.Round(b * ox + a * oy + ty);
                if (sx >= 0 && sy >= 0 && sx < img.Width && sy < img.Height)
                    aligned[ox, oy] = img[sx, sy];
            }
        }
        return aligned;
    }

    private static (double a, double b, double tx, double ty) EstimateSimilarityTransform(
        float[][] src, float[][] dst)
    {
        var n = (double)src.Length;
        double sx = 0, sy = 0, dx = 0, dy = 0;
        for (var i = 0; i < src.Length; i++)
        {
            sx += src[i][0]; sy += src[i][1];
            dx += dst[i][0]; dy += dst[i][1];
        }
        sx /= n; sy /= n; dx /= n; dy /= n;

        double numA = 0, numB = 0, denom = 0;
        for (var i = 0; i < src.Length; i++)
        {
            var (cx, cy) = (src[i][0] - sx, src[i][1] - sy);
            var (txx, tyy) = (dst[i][0] - dx, dst[i][1] - dy);
            numA += cx * txx + cy * tyy;
            numB += cx * tyy - cy * txx;
            denom += cx * cx + cy * cy;
        }

        if (Math.Abs(denom) < 1e-12)
            return (1.0, 0.0, dx - sx, dy - sy);

        var aFwd = numA / denom;
        var bFwd = numB / denom;
        var txInv = dx - (aFwd * sx - bFwd * sy);
        var tyInv = dy - (bFwd * sx + aFwd * sy);

        var det = aFwd * aFwd + bFwd * bFwd;
        var aInv = aFwd / det;
        var bInv = -bFwd / det;
        var txFwd = -(aInv * txInv - bInv * tyInv);
        var tyFwd = -(bInv * txInv + aInv * tyInv);

        return (aInv, bInv, txFwd, tyFwd);
    }

    private static float[] L2Normalize(float[] v)
    {
        var norm = MathF.Sqrt(v.Sum(x => x * x));
        if (norm <= 0) return v;
        
        for (var i = 0; i < v.Length; i++)
            v[i] /= norm;

        return v;

    }

    public void Dispose()
    {
        GC.SuppressFinalize(this);
        _session.Dispose();
    }
}
