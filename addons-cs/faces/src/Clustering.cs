namespace IsomFolio.Addons.Faces;

public static class Clustering
{
    public static int[] Dbscan(float[][] embeddings, float eps, int minPts)
    {
        var n = embeddings.Length;
        var labels = new int[n];
        Array.Fill(labels, -1);

        var neighbors = PrecomputeNeighbors(embeddings, eps);

        var clusterId = 0;
        for (var i = 0; i < n; i++)
        {
            if (labels[i] != -1) continue;
            if (neighbors[i].Count < minPts) continue;

            labels[i] = clusterId;
            var queue = new Queue<int>(neighbors[i]);

            while (queue.Count > 0)
            {
                var j = queue.Dequeue();
                if (labels[j] != -1) continue;
                labels[j] = clusterId;

                if (neighbors[j].Count >= minPts)
                    foreach (var k in neighbors[j])
                        if (labels[k] == -1)
                            queue.Enqueue(k);
            }
            clusterId++;
        }
        return labels;
    }

    public static int[] AssignToCentroids(float[][] embeddings, float[][] centroids, float eps)
    {
        var labels = new int[embeddings.Length];
        Array.Fill(labels, -1);

        for (var i = 0; i < embeddings.Length; i++)
        {
            var bestSim = 0f;
            var bestLabel = -1;
            for (var ci = 0; ci < centroids.Length; ci++)
            {
                var sim = CosineSim(embeddings[i], centroids[ci]);
                if (sim > bestSim)
                {
                    bestSim = sim;
                    bestLabel = ci;
                }
            }
            if (bestSim >= 1f - eps)
                labels[i] = bestLabel;
        }
        return labels;
    }

    public static float[] ComputeCentroid(float[][] embeddings)
    {
        if (embeddings.Length == 0) return [];
        var dim = embeddings[0].Length;
        var centroid = new float[dim];
        foreach (var emb in embeddings)
            for (var j = 0; j < dim; j++)
                centroid[j] += emb[j];

        var n = (float)embeddings.Length;
        var norm = 0f;
        for (var j = 0; j < dim; j++)
        {
            centroid[j] /= n;
            norm += centroid[j] * centroid[j];
        }
        norm = MathF.Sqrt(norm);
        if (norm > 0)
            for (var j = 0; j < dim; j++)
                centroid[j] /= norm;

        return centroid;
    }

    private static List<int>[] PrecomputeNeighbors(float[][] embeddings, float eps)
    {
        var n = embeddings.Length;
        var threshold = 1f - eps;
        var neighbors = new List<int>[n];
        for (var i = 0; i < n; i++)
            neighbors[i] = [];

        for (var i = 0; i < n; i++)
        {
            for (var j = i + 1; j < n; j++)
            {
                if (CosineSim(embeddings[i], embeddings[j]) >= threshold)
                {
                    neighbors[i].Add(j);
                    neighbors[j].Add(i);
                }
            }
        }
        return neighbors;
    }

    private static float CosineSim(float[] a, float[] b)
    {
        float dot = 0, na = 0, nb = 0;
        for (var i = 0; i < a.Length; i++)
        {
            dot += a[i] * b[i];
            na += a[i] * a[i];
            nb += b[i] * b[i];
        }
        na = MathF.Sqrt(na);
        nb = MathF.Sqrt(nb);
        return na == 0 || nb == 0 ? 0 : dot / (na * nb);
    }
}
