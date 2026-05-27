using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public static class Program
{
    public static async Task Main()
    {
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };

        using var writer = new MessageWriter(Console.Out);

        var loadingTask = LoadAsync(writer, cts.Token);
        _ = loadingTask.ContinueWith(_ => cts.Cancel(), TaskContinuationOptions.OnlyOnFaulted);

        await using var worker = new FacesWorker(writer, loadingTask);

        try
        {
            await foreach (var msg in MessageReader.ReadAllAsync(Console.In, writer, cts.Token))
            {
                switch (msg)
                {
                    case HandshakeRequest req:
                        await writer.SendHandshakeResponseAsync(req.Id, AddonInfo.Version, [AddonCapability.ClusterFaces]);
                        break;
                    case PingRequest req:
                        await writer.SendPingResponseAsync(req.Id);
                        break;
                    case ClusterFacesRequest req:
                        worker.Enqueue(req, cts.Token);
                        break;
                    default:
                        await writer.SendErrorResponseAsync(msg.Id, $"unknown request: {msg.GetType().Name}");
                        break;
                }
            }
        }
        catch (OperationCanceledException) { }
    }

    private static async Task<IRequestHandler> LoadAsync(MessageWriter writer, CancellationToken ct)
    {
        await writer.LogAsync(LogLevel.Info, "loading face models…");
        var handler = await new RequestHandlerFactory(writer, writer).CreateAsync(ct);
        await writer.SendReadyAsync();
        return handler;
    }
}
