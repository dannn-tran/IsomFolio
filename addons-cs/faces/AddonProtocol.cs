using System.Text.Json;

namespace IsomFolio.Addons.Faces;

public static class Protocol
{
    public static void SendHello(TextWriter output, string[] capabilities)
    {
        var msg = new HelloMessage("hello", 1, 1, capabilities);
        output.WriteLine(JsonSerializer.Serialize(msg, AppJsonContext.Default.HelloMessage));
        output.Flush();
    }

    public static void SendResponse(TextWriter output, ulong id, ClusterResult result)
    {
        var msg = new ResponseMessage(id, result);
        output.WriteLine(JsonSerializer.Serialize(msg, AppJsonContext.Default.ResponseMessage));
        output.Flush();
    }

    public static void SendError(TextWriter output, ulong id, string error)
    {
        var msg = new ErrorMessage(id, error);
        output.WriteLine(JsonSerializer.Serialize(msg, AppJsonContext.Default.ErrorMessage));
        output.Flush();
    }

    public static void EmitLog(TextWriter output, string level, string message)
    {
        var msg = new LogMessage("log", level, message);
        output.WriteLine(JsonSerializer.Serialize(msg, AppJsonContext.Default.LogMessage));
        output.Flush();
    }

    public static void EmitProgress(TextWriter output, ulong id, int percent)
    {
        var msg = new ProgressMessage("progress", id, percent);
        output.WriteLine(JsonSerializer.Serialize(msg, AppJsonContext.Default.ProgressMessage));
        output.Flush();
    }

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
