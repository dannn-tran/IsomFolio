using System.Text.Json;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

public static class Program
{
    public static async Task Main(string[] args)
    {
        var crashLog = new FacesLogger("fatal");
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
            crashLog.Log(LogLevel.Error, $"unhandled: {e.ExceptionObject}");
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            crashLog.Log(LogLevel.Error, $"unobserved task: {e.Exception}");
            e.SetObserved();
        };

        var dataDir = SdkArgs.DataDir(args);

        if (SdkArgs.IsSetupMode(args))
            await RunSetupAsync(dataDir);
        else
            await RunAsync(dataDir);
    }

    private static async Task RunAsync(string dataDir)
    {
        var log = new FacesLogger("startup");
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };
        using var writer = new MessageWriter(Console.Out);

        try
        {
            // Phase 1: respond to handshake immediately, before any heavy work
            await foreach (var msg in MessageReader.ReadAllAsync(Console.In, cts.Token))
            {
                if (msg is not HandshakeRequest req) continue;
                await writer.SendHandshakeResponseAsync(req.Id, ExtensionInfo.Version, [ExtensionCapability.ClusterFaces]);
                break;
            }

            // Phase 2: load models from disk (may download if variant changed)
            IRequestHandler handler;
            try
            {
                await log.LogAsync(LogLevel.Info, "loading face models…");
                handler = await new RequestHandlerFactory(writer).CreateAsync(AppContext.BaseDirectory, dataDir, cts.Token);
                await log.LogAsync(LogLevel.Info, "models ready");
            }
            catch (Exception ex)
            {
                var detail = ex.InnerException is null
                    ? ex.Message
                    : $"{ex.Message} — caused by: {ex.InnerException.GetType().Name}: {ex.InnerException.Message}";
                await log.LogAsync(LogLevel.Error, $"startup failed: {ex}");
                await writer.SendFatalAsync(repairable: ex is FileNotFoundException, detail);
                return;
            }

            await writer.SendReadyAsync();

            // Phase 3: request loop
            using (handler)
            {
                await foreach (var msg in MessageReader.ReadAllAsync(Console.In, cts.Token))
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
            log.Log(LogLevel.Error, $"unhandled in RunAsync: {ex.GetType().Name}: {ex.Message}");
            await writer.SendFatalAsync(repairable: false, $"unhandled: {ex.GetType().Name}: {ex.Message}");
            Environment.Exit(1);
        }
    }

    private static async Task RunSetupAsync(string dataDir)
    {
        var log = new FacesLogger("setup");
        try
        {
            var configPath = Path.Combine(AppContext.BaseDirectory, "config.json");
            DbscanConfig config = new();
            if (File.Exists(configPath))
            {
                try
                {
                    var json = await File.ReadAllTextAsync(configPath);
                    config = JsonSerializer.Deserialize(json, AppJsonContext.Default.DbscanConfig) ?? config;
                }
                catch { }
            }
            await new ModelDownloader().EnsureModelsDownloadedAsync(dataDir, ModelVariant.Current(config.ModelVariant));
            await log.LogAsync(LogLevel.Info, "complete");
        }
        catch (Exception ex)
        {
            await log.LogAsync(LogLevel.Error, $"failed: {ex.Message}");
            Environment.Exit(1);
        }
    }
}
