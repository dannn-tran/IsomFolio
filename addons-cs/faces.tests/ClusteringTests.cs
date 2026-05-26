namespace IsomFolio.Addons.Faces.Tests;

public class ClusteringTests
{
    private static float[] Normalize(float[] v)
    {
        var norm = MathF.Sqrt(v.Sum(x => x * x));
        return v.Select(x => x / norm).ToArray();
    }

    public class DbscanTests : ClusteringTests
    {
        [Fact]
        public void TwoClusters_SeparatedCorrectly()
        {
            var embeddings = new[]
            {
                Normalize([1, 0, 0]),
                Normalize([0.95f, 0.05f, 0]),
                Normalize([0, 0, 1]),
                Normalize([0.05f, 0, 0.95f]),
            };
            var labels = Clustering.Dbscan(embeddings, 0.3f, 1);

            Assert.Equal(labels[0], labels[1]);
            Assert.Equal(labels[2], labels[3]);
            Assert.NotEqual(labels[0], labels[2]);
        }

        [Fact]
        public void SinglePoint_IsNoise()
        {
            var embeddings = new[]
            {
                Normalize([1, 0, 0]),
                Normalize([0, 1, 0]),
                Normalize([0, 0, 1]),
            };
            var labels = Clustering.Dbscan(embeddings, 0.3f, 2);

            Assert.All(labels, l => Assert.Equal(-1, l));
        }

        [Fact]
        public void AllSimilar_OneCluster()
        {
            var embeddings = new[]
            {
                Normalize([1, 0, 0]),
                Normalize([0.99f, 0.01f, 0]),
                Normalize([0.98f, 0.02f, 0]),
            };
            var labels = Clustering.Dbscan(embeddings, 0.3f, 1);

            Assert.True(labels.All(l => l == labels[0] && l >= 0));
        }
        
        [Fact]
        public void EmptyInput_ReturnsEmpty()
        {
            var labels = Clustering.Dbscan([], 0.3f, 2);
            Assert.Empty(labels);
        }
    }

    public class AssignToCentroidsTests
    {
        [Fact]
        public void MatchesNearest()
        {
            var centroids = new[]
            {
                Normalize([1, 0, 0]),
                Normalize([0, 0, 1]),
            };
            var embeddings = new[]
            {
                Normalize([0.9f, 0.1f, 0]),
                Normalize([0.1f, 0, 0.9f]),
            };
            var labels = Clustering.AssignToCentroids(embeddings, centroids, 0.4f);

            Assert.Equal(0, labels[0]);
            Assert.Equal(1, labels[1]);
        }

        [Fact]
        public void FarFromAll_IsNoise()
        {
            var centroids = new[] { Normalize([1, 0, 0]) };
            var embeddings = new[] { Normalize([0, 1, 0]) };
            var labels = Clustering.AssignToCentroids(embeddings, centroids, 0.3f);

            Assert.Equal(-1, labels[0]);
        }
    }

    public class ComputeCentroidTests
    {
        [Fact]
        public void IsNormalized()
        {
            var embeddings = new[]
            {
                Normalize([1, 0, 0]),
                Normalize([0, 1, 0]),
            };
            var centroid = Clustering.ComputeCentroid(embeddings);

            var norm = MathF.Sqrt(centroid.Sum(x => x * x));
            Assert.InRange(norm, 0.99f, 1.01f);
        }
    }
}
