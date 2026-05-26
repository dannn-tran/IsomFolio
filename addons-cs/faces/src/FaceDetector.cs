using Microsoft.ML.OnnxRuntime;
using Microsoft.ML.OnnxRuntime.Tensors;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.PixelFormats;
using SixLabors.ImageSharp.Processing;

namespace IsomFolio.Addons.Faces;

public record DetectedFace(float BboxX, float BboxY, float BboxW, float BboxH, float[][] Kps);

public class FaceDetector : IDisposable
{
    const int InputSize = 640;
    const float ScoreThresh = 0.5f;
    const float NmsThresh = 0.4f;
    static readonly int[] Strides = [8, 16, 32];
    const int NumAnchors = 2;

    private readonly InferenceSession _session;

    public FaceDetector(string modelPath)
    {
        var opts = new SessionOptions { InterOpNumThreads = 1, IntraOpNumThreads = 4 };
        opts.GraphOptimizationLevel = GraphOptimizationLevel.ORT_ENABLE_ALL;
        _session = new InferenceSession(modelPath, opts);
    }

    public List<DetectedFace> Detect(Image<Rgb24> image)
    {
        var origW = image.Width;
        var origH = image.Height;
        var scaleX = (float)origW / InputSize;
        var scaleY = (float)origH / InputSize;

        using var resized = image.Clone(ctx => ctx.Resize(InputSize, InputSize));
        var input = Preprocess(resized);

        using var results = _session.Run(new[] { NamedOnnxValue.CreateFromTensor(_session.InputNames[0], input) });
        var outputs = results.ToList();

        if (outputs.Count < 9)
            throw new InvalidOperationException($"Expected 9 SCRFD outputs, got {outputs.Count}");

        return DecodeScrfd(outputs, scaleX, scaleY);
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
                    tensor[0, 0, y, x] = (p.R - 127.5f) / 128f;
                    tensor[0, 1, y, x] = (p.G - 127.5f) / 128f;
                    tensor[0, 2, y, x] = (p.B - 127.5f) / 128f;
                }
            }
        });
        return tensor;
    }

    private static List<DetectedFace> DecodeScrfd(List<DisposableNamedOnnxValue> outputs, float scaleX, float scaleY)
    {
        var faces = new List<(float score, float[] bbox, float[][] kps)>();

        for (var si = 0; si < Strides.Length; si++)
        {
            var stride = Strides[si];
            var h = InputSize / stride;
            var w = InputSize / stride;
            var n = h * w * NumAnchors;

            var scores = outputs[si].AsEnumerable<float>().ToArray();
            var bboxes = outputs[si + 3].AsEnumerable<float>().ToArray();
            var kpsData = outputs[si + 6].AsEnumerable<float>().ToArray();

            if (scores.Length < n) continue;

            var anchors = GenerateAnchors(h, w, stride);
            var s = (float)stride;

            for (var i = 0; i < n; i++)
            {
                var score = Sigmoid(scores[i]);
                if (score < ScoreThresh) continue;

                var cx = anchors[i].x;
                var cy = anchors[i].y;

                var bi = i * 4;
                var ki = i * 10;
                if (bi + 3 >= bboxes.Length || ki + 9 >= kpsData.Length) continue;

                var x1 = (cx - bboxes[bi] * s) * scaleX;
                var y1 = (cy - bboxes[bi + 1] * s) * scaleY;
                var x2 = (cx + bboxes[bi + 2] * s) * scaleX;
                var y2 = (cy + bboxes[bi + 3] * s) * scaleY;

                var kps = new float[5][];
                for (var k = 0; k < 5; k++)
                {
                    kps[k] =
                    [
                        (cx + kpsData[ki + k * 2] * s) * scaleX,
                        (cy + kpsData[ki + k * 2 + 1] * s) * scaleY
                    ];
                }

                faces.Add((score, [x1, y1, x2, y2], kps));
            }
        }

        faces.Sort((a, b) => b.score.CompareTo(a.score));
        return Nms(faces, NmsThresh);
    }

    private static (float x, float y)[] GenerateAnchors(int h, int w, int stride)
    {
        var s = (float)stride;
        var anchors = new (float x, float y)[h * w * NumAnchors];
        var idx = 0;
        for (var row = 0; row < h; row++)
            for (var col = 0; col < w; col++)
                for (var a = 0; a < NumAnchors; a++)
                    anchors[idx++] = (col * s, row * s);
        return anchors;
    }

    private static List<DetectedFace> Nms(List<(float score, float[] bbox, float[][] kps)> faces, float thresh)
    {
        var suppressed = new bool[faces.Count];
        var kept = new List<DetectedFace>();

        for (var i = 0; i < faces.Count; i++)
        {
            if (suppressed[i]) continue;
            var (_, bbox, kps) = faces[i];
            kept.Add(new DetectedFace(bbox[0], bbox[1], bbox[2] - bbox[0], bbox[3] - bbox[1], kps));

            for (var j = i + 1; j < faces.Count; j++)
            {
                if (!suppressed[j] && Iou(bbox, faces[j].bbox) > thresh)
                    suppressed[j] = true;
            }
        }
        return kept;
    }

    private static float Iou(float[] a, float[] b)
    {
        var x1 = Math.Max(a[0], b[0]);
        var y1 = Math.Max(a[1], b[1]);
        var x2 = Math.Min(a[2], b[2]);
        var y2 = Math.Min(a[3], b[3]);
        var inter = Math.Max(0, x2 - x1) * Math.Max(0, y2 - y1);
        var areaA = (a[2] - a[0]) * (a[3] - a[1]);
        var areaB = (b[2] - b[0]) * (b[3] - b[1]);
        return inter / (areaA + areaB - inter + 1e-6f);
    }

    private static float Sigmoid(float x) => 1f / (1f + MathF.Exp(-x));

    public void Dispose() => _session.Dispose();
}
