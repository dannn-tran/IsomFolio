using System.Text.Json;

namespace IsomFolio.Addons.Faces;

public interface IMessageSender
{
    void SendHello(string[] capabilities);
    void SendResponse(ulong id, ClusterResult result);
    void SendError(ulong id, string error);
    void SendInfo(string level, string message);
    void SendProgress(ulong id, int percent);
}

internal class MessageSender(IMessageSink sink) : IMessageSender
{
    public void SendHello(string[] capabilities)
    {
        var msg = new HelloMessage("hello", 1, 1, capabilities);
        sink.Receive(JsonSerializer.Serialize(msg, AppJsonContext.Default.HelloMessage));
    }

    public void SendResponse(ulong id, ClusterResult result)
    {
        var msg = new ResponseMessage(id, result);
        sink.Receive(JsonSerializer.Serialize(msg, AppJsonContext.Default.ResponseMessage));
    }

    public void SendError(ulong id, string error)
    {
        var msg = new ErrorMessage(id, error);
        sink.Receive(JsonSerializer.Serialize(msg, AppJsonContext.Default.ErrorMessage));
    }

    public void SendInfo(string level, string message)
    {
        var msg = new LogMessage("log", level, message);
        sink.Receive(JsonSerializer.Serialize(msg, AppJsonContext.Default.LogMessage));
    }

    public void SendProgress(ulong id, int percent)
    {
        var msg = new ProgressMessage("progress", id, percent);
        sink.Receive(JsonSerializer.Serialize(msg, AppJsonContext.Default.ProgressMessage));
    }
}

public static class Protocol
{
    public static AddonRequest? ParseRequest(string line)
    {
        try
        {
            return JsonSerializer.Deserialize(line, AppJsonContext.Default.AddonRequest);
        }
        catch
        {
            return null;
        }
    }
}
