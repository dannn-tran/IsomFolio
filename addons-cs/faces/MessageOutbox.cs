using System.Text.Json;

namespace IsomFolio.Addons.Faces;

public interface IMessageOutbox
{
    void SendHello(string[] capabilities);
    void SendResponse(ulong id, ClusterResult result);
    void SendError(ulong id, string error);
    void SendLog(LogLevel level, string message);
    void SendProgress(ulong id, int percent);
}

internal class MessageOutbox(TextWriter output) : IMessageOutbox
{
    public void SendHello(string[] capabilities)
    {
        var msg = new HelloMessage(1, 1, capabilities);
        Send(JsonSerializer.Serialize(msg, AppJsonContext.Default.HelloMessage));
    }

    public void SendResponse(ulong id, ClusterResult result)
    {
        var msg = new ResponseMessage(id, result);
        Send(JsonSerializer.Serialize(msg, AppJsonContext.Default.ResponseMessage));
    }

    public void SendError(ulong id, string error)
    {
        var msg = new ErrorMessage(id, error);
        Send(JsonSerializer.Serialize(msg, AppJsonContext.Default.ErrorMessage));
    }

    public void SendLog(LogLevel level, string message)
    {
        var msg = new LogMessage(level, message);
        Send(JsonSerializer.Serialize(msg, AppJsonContext.Default.LogMessage));
    }

    public void SendProgress(ulong id, int percent)
    {
        var msg = new ProgressMessage(id, percent);
        Send(JsonSerializer.Serialize(msg, AppJsonContext.Default.ProgressMessage));
    }

    private void Send(string serializedMessage)
    {
        output.WriteLine(serializedMessage);
        output.Flush();
    }
}
