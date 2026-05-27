using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public interface IRequestHandler : IDisposable
{
    Task<ClusterResult> HandleAsync(ClusterFacesRequest request, CancellationToken ct = default);
}
