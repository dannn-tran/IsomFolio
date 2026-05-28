using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces;

public static class Program
{
    public static async Task Main(string[] args)
    {
        var dataDir = SdkArgs.DataDir(args);
        if (dataDir is null)
        {
            await Console.Error.WriteLineAsync("usage: faces [install] --data-dir <path>");
            Environment.Exit(1);
            return;
        }

        if (SdkArgs.IsInstallMode(args))
            await RunInstallAsync(dataDir);
        else
            await RunRuntimeAsync(dataDir);
    }

    private static async Task RunRuntimeAsync(string dataDir)
    {
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };
        using var writer = new MessageWriter(Console.Out);

        try
        {
            // Phase 1: respond to handshake immediately, before any heavy work
            await foreach (var msg in MessageReader.ReadAllAsync(Console.In, writer, cts.Token))
            {
                if (msg is HandshakeRequest req)
                {
                    await writer.SendHandshakeResponseAsync(req.Id, AddonInfo.Version, [AddonCapability.ClusterFaces]);
                    break;
                }
            }

            // Phase 2: load models from disk
            IRequestHandler handler;
            try
            {
                await writer.LogAsync(LogLevel.Info, "loading face models…");
                handler = await new RequestHandlerFactory(writer, writer).CreateAsync(dataDir, cts.Token);
            }
            catch (Exception ex)
            {
                await writer.SendFatalAsync(repairable: ex is FileNotFoundException, ex.Message);
                return;
            }

            await writer.SendReadyAsync();

            // Phase 3: request loop
            using (handler)
            {
                await foreach (var msg in MessageReader.ReadAllAsync(Console.In, writer, cts.Token))
                {
                    switch (msg)
                    {
                        case PingRequest req:
                            await writer.SendPingResponseAsync(req.Id);
                            break;
                        case ClusterFacesRequest req:
                            try
                            {
                                var result = await handler.HandleAsync(req, cts.Token);
                                await writer.SendClusterResponseAsync(req.Id, result);
                            }
                            catch (OperationCanceledException) { throw; }
                            catch (Exception ex) { await writer.SendErrorResponseAsync(req.Id, ex.Message); }
                            break;
                        default:
                            await writer.SendErrorResponseAsync(msg.Id, $"unknown request: {msg.GetType().Name}");
                            break;
                    }
                }
            }
        }
        catch (OperationCanceledException) { }
    }

    private static async Task RunInstallAsync(string dataDir)
    {
        using var writer = new MessageWriter(Console.Out);
        try
        {
            await new ModelDownloader(writer).EnsureModelsDownloadedAsync(dataDir);
            await writer.LogAsync(LogLevel.Info, "installation complete");
        }
        catch (Exception ex)
        {
            await writer.LogAsync(LogLevel.Error, $"installation failed: {ex.Message}");
            Environment.Exit(1);
        }
    }

}
