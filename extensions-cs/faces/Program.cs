using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public static class Program
{
    public static async Task Main(string[] args)
    {
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            Console.Error.WriteLine($"UNHANDLED EXCEPTION: {e.ExceptionObject}");
            Console.Error.Flush();
        };
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            Console.Error.WriteLine($"UNOBSERVED TASK EXCEPTION: {e.Exception}");
            Console.Error.Flush();
            e.SetObserved();
        };

        if (SdkArgs.IsSetupMode(args))
            await RunSetupAsync();
        else
            await RunAsync();
    }

    private static async Task RunAsync()
    {
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };
        using var writer = new MessageWriter(Console.Out);

        try
        {
            // Phase 1: respond to handshake immediately, before any heavy work
            await foreach (var msg in MessageReader.ReadAllAsync(Console.In, writer, cts.Token))
            {
                if (msg is not HandshakeRequest req) continue;
                await writer.SendHandshakeResponseAsync(req.Id, ExtensionInfo.Version, [ExtensionCapability.ClusterFaces]);
                break;
            }

            // Phase 2: load models from disk
            IRequestHandler handler;
            try
            {
                await writer.LogAsync(LogLevel.Info, "loading face models…");
                handler = await new RequestHandlerFactory(writer, writer).CreateAsync(AppContext.BaseDirectory, cts.Token);
            }
            catch (Exception ex)
            {
                var detail = ex.InnerException is null
                    ? ex.Message
                    : $"{ex.Message} — caused by: {ex.InnerException.GetType().Name}: {ex.InnerException.Message}";
                await writer.LogAsync(LogLevel.Error, $"startup trace: {ex}");
                await writer.SendFatalAsync(repairable: ex is FileNotFoundException, detail);
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
        catch (Exception ex)
        {
            Console.Error.WriteLine($"FATAL in RunAsync: {ex}");
            Console.Error.Flush();
            await writer.SendFatalAsync(repairable: false, $"unhandled: {ex.GetType().Name}: {ex.Message}");
            Environment.Exit(1);
        }
    }

    private static async Task RunSetupAsync()
    {
        using var writer = new MessageWriter(Console.Out);
        try
        {
            await new ModelDownloader(writer).EnsureModelsDownloadedAsync(SdkArgs.ModelsDir());
            await writer.LogAsync(LogLevel.Info, "setup complete");
        }
        catch (Exception ex)
        {
            await writer.LogAsync(LogLevel.Error, $"setup failed: {ex.Message}");
            Environment.Exit(1);
        }
    }
}
