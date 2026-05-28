using System.Text.Json;

namespace IsomFolio.Addons.Sdk;

public interface IAddonLogger
{
    ValueTask LogAsync(LogLevel level, string message);
}

public interface IMessageWriter
{
    ValueTask SendHandshakeResponseAsync(ulong id, string addonVersion, AddonCapability[] capabilities);
    ValueTask SendPingResponseAsync(ulong id);
    ValueTask SendReadyAsync();
    ValueTask SendFatalAsync(bool repairable, string message);
    ValueTask SendProgressAsync(ulong id, int percent);
    ValueTask SendClassifyResponseAsync(ulong id, ClassifyResult result);
    ValueTask SendClusterResponseAsync(ulong id, ClusterResult result);
    ValueTask SendErrorResponseAsync(ulong id, string error);
}

public class MessageWriter(TextWriter output) : IAddonLogger, IMessageWriter, IDisposable
{
    private readonly SemaphoreSlim _writeLock = new(1, 1);

    public ValueTask LogAsync(LogLevel level, string message) => SendAsync(new LogMessage(level, message));
    public ValueTask SendHandshakeResponseAsync(ulong id, string addonVersion, AddonCapability[] capabilities) => SendAsync(new OkResponse<HandshakeResult>(id, new HandshakeResult(1, addonVersion, capabilities)));
    public ValueTask SendPingResponseAsync(ulong id) => SendAsync(new OkResponse<PingResult>(id, new PingResult()));
    public ValueTask SendReadyAsync() => SendAsync(new ReadyMessage());
    public ValueTask SendFatalAsync(bool repairable, string message) => SendAsync(new FatalMessage(repairable, message));
    public ValueTask SendProgressAsync(ulong id, int percent) => SendAsync(new ProgressMessage(id, percent));
    public ValueTask SendClassifyResponseAsync(ulong id, ClassifyResult result) => SendAsync(new OkResponse<ClassifyResult>(id, result));
    public ValueTask SendClusterResponseAsync(ulong id, ClusterResult result) => SendAsync(new OkResponse<ClusterResult>(id, result));
    public ValueTask SendErrorResponseAsync(ulong id, string error) => SendAsync(new ErrorResponse(id, error));

    public void Dispose()
    {
        _writeLock.Dispose();
        GC.SuppressFinalize(this);
    }

    private async ValueTask SendAsync(OutboundMessage msg)
    {
        var json = msg switch
        {
            OutboundEvent evt                => JsonSerializer.Serialize(evt, SdkJsonContext.Default.OutboundEvent),
            OkResponse<HandshakeResult> r   => JsonSerializer.Serialize(r, SdkJsonContext.Default.OkResponseHandshakeResult),
            OkResponse<PingResult> r        => JsonSerializer.Serialize(r, SdkJsonContext.Default.OkResponsePingResult),
            OkResponse<ClassifyResult> r    => JsonSerializer.Serialize(r, SdkJsonContext.Default.OkResponseClassifyResult),
            OkResponse<ClusterResult> r     => JsonSerializer.Serialize(r, SdkJsonContext.Default.OkResponseClusterResult),
            ErrorResponse e                 => JsonSerializer.Serialize(e, SdkJsonContext.Default.ErrorResponse),
            _ => throw new ArgumentOutOfRangeException(nameof(msg), msg.GetType().Name, "unregistered OutboundMessage type")
        };
        await _writeLock.WaitAsync();
        try
        {
            await output.WriteLineAsync(json);
            await output.FlushAsync();
        }
        finally { _writeLock.Release(); }
    }
}
