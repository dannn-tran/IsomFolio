using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public interface IRequestHandler : IDisposable
{
    Task<ClusterResult> HandleAsync(ClusterFacesRequest request, CancellationToken ct = default);
}
