using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public sealed class FacesWorker(IMessageWriter writer, Task<IRequestHandler> handlerTask) : IAsyncDisposable
{
    private readonly SemaphoreSlim _gate = new(1, 1);
    private readonly List<Task> _pendingTasks = [];

    public void Enqueue(ClusterFacesRequest req, CancellationToken ct) =>
        _pendingTasks.Add(ProcessClusterAsync(req, ct));

    public async ValueTask DisposeAsync()
    {
        await Task.WhenAll(_pendingTasks);
        _gate.Dispose();
        if (handlerTask.IsCompletedSuccessfully)
            handlerTask.Result.Dispose();
    }

    private async Task ProcessClusterAsync(ClusterFacesRequest req, CancellationToken ct)
    {
        try
        {
            await _gate.WaitAsync(ct);
            try
            {
                IRequestHandler handler;
                try { handler = await handlerTask; }
                catch (Exception ex) { await writer.SendErrorResponseAsync(req.Id, $"model init failed: {ex.Message}"); return; }

                var result = await handler.HandleAsync(req, ct);
                await writer.SendClusterResponseAsync(req.Id, result);
            }
            finally { _gate.Release(); }
        }
        catch (OperationCanceledException) { }
        catch (Exception ex) { await writer.SendErrorResponseAsync(req.Id, ex.Message); }
    }
}
